use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};

use crate::config;
use super::discover::DocumentInfo;

/// Result of rendering a single document for the collection.
#[derive(Clone)]
pub struct CollectionRenderResult {
    pub body: String,
    pub toc: Option<String>,
    pub title: Option<String>,
    pub date: Option<String>,
    pub subtitle: Option<String>,
    pub abstract_text: Option<String>,
}

/// Render all documents by calling calepin's render_core() directly.
/// Returns a map from source path to rendered result.
///
/// When `apply_page_template` is true, each page's body is wrapped in the
/// project's page template (e.g., `templates/latex/page.tex`). This is
/// used for orchestrated builds where fragments need to be complete files.
/// When false (HTML sites), the body is returned raw for wrapping by site
/// Jinja templates.
pub fn render_documents(
    pages: &[DocumentInfo],
    meta: &crate::config::Metadata,
    base_dir: &Path,
    output_dir: &Path,
    format: &str,
    apply_page_template: bool,
    target_name: Option<&str>,
    target: Option<&config::Target>,
    quiet: bool,
) -> Result<HashMap<String, CollectionRenderResult>> {
    if pages.is_empty() {
        return Ok(HashMap::new());
    }

    let overrides = build_overrides(meta, target);

    let format_owned = format.to_string();
    let target_owned = target_name.map(|s| s.to_string());

    let results = render_parallel(pages, quiet, |page| {
        let key = page.source.display().to_string();
        crate::paths::set_active_target(target_owned.as_deref());
        crate::paths::set_project_root(Some(base_dir));
        let result = render_one_document(page, &overrides, base_dir, output_dir, &format_owned, apply_page_template, Some(meta));
        (key, result)
    });

    let mut map = HashMap::new();
    let mut failed: Vec<&DocumentInfo> = Vec::new();
    for (key, result) in &results {
        match result {
            Ok(render_result) => {
                map.insert(key.clone(), render_result.clone());
            }
            Err(e) => {
                eprintln!("Error rendering {}: {:#}", key, e);
                // Collect failed pages for retry
                if let Some(page) = pages.iter().find(|p| p.source.display().to_string() == *key) {
                    failed.push(page);
                }
            }
        }
    }

    // Retry failed pages sequentially (avoids concurrent cairo crashes)
    if !failed.is_empty() {
        if !quiet {
            eprintln!("Retrying {} failed document(s) sequentially...", failed.len());
        }
        for page in &failed {
            crate::paths::set_active_target(target_name.map(|s| s));
            crate::paths::set_project_root(Some(base_dir));
            let key = page.source.display().to_string();
            match render_one_document(page, &overrides, base_dir, output_dir, format, apply_page_template, Some(meta)) {
                Ok(render_result) => {
                    if !quiet {
                        eprintln!("  [ok] {}", key);
                    }
                    map.insert(key, render_result);
                }
                Err(e) => {
                    eprintln!("Error rendering {} (retry failed): {:#}", key, e);
                }
            }
        }
    }

    Ok(map)
}

fn render_one_document(
    page: &DocumentInfo,
    overrides: &[String],
    base_dir: &Path,
    output_dir: &Path,
    format: &str,
    apply_page_template: bool,
    project_metadata: Option<&crate::config::Metadata>,
) -> Result<CollectionRenderResult> {
    let input = base_dir.join(&page.source);
    let output_path = output_dir.join(&page.output);

    // Ensure the output parent directory exists before rendering,
    // so figure files can be written alongside the output.
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let result = crate::render::pipeline::render_core(&input, &output_path, Some(format), overrides, Some(base_dir), &crate::render::pipeline::RenderCoreOptions::default(), project_metadata, None)?;

    let body = if apply_page_template {
        // Apply the project's page template (e.g., book's minimal page.tex)
        let pipeline = crate::render::formats::FormatPipeline::from_writer(format)?;
        pipeline.assemble_page(&result.rendered, &result.metadata, &result.element_renderer)
            .unwrap_or(result.rendered)
    } else {
        // Site mode: run document transforms (footnotes, highlight CSS, etc.)
        let pipeline = crate::render::formats::FormatPipeline::from_writer(format)?;
        pipeline.transform_document(&result.rendered, &result.element_renderer)
    };

    // Build TOC from rendered headings (HTML only)
    let toc = if format == "html" && result.metadata.toc.as_ref().and_then(|t| t.enabled).unwrap_or(true) {
        let depth = result.metadata.toc.as_ref().and_then(|t| t.depth).unwrap_or(3) as u8;
        let title = result.metadata.toc.as_ref().and_then(|t| t.title.as_deref()).unwrap_or("Contents");
        let toc_html = crate::render::template::build_toc_html_from_body(&body, depth, title);
        if toc_html.is_empty() { None } else { Some(toc_html) }
    } else {
        None
    };

    Ok(CollectionRenderResult {
        body,
        toc,
        title: result.metadata.title.map(|t| crate::render::convert::render_inline(&t, format)),
        date: result.metadata.date,
        subtitle: result.metadata.subtitle.map(|t| crate::render::convert::render_inline(&t, format)),
        abstract_text: result.metadata.abstract_text,
    })
}

/// Render documents with cross-file cross-reference resolution (HTML only).
///
/// Two-pass pipeline:
///   Pass 1: Render all pages in parallel with cross-ref resolution deferred.
///   Between: Build a global CrossRefRegistry with chapter-prefixed numbers.
///   Pass 2: Resolve cross-refs and renumber display numbers in parallel.
pub fn render_documents_with_crossref(
    pages: &[DocumentInfo],
    meta: &crate::config::Metadata,
    base_dir: &Path,
    output_dir: &Path,
    target_name: Option<&str>,
    target: Option<&config::Target>,
    quiet: bool,
) -> Result<HashMap<String, CollectionRenderResult>> {
    use crate::crossref::{CrossRefRegistry, resolve_html_global, renumber_display_html};

    if pages.is_empty() {
        return Ok(HashMap::new());
    }

    let overrides = build_overrides(meta, target);

    // Assign chapter numbers based on [[contents]] ordering
    let chapter_map = assign_chapter_numbers(meta);

    let target_owned = target_name.map(|s| s.to_string());

    // Pass 1: Render all pages in parallel with skip_crossref=true
    let pass1 = render_parallel(pages, quiet, |page| {
        let key = page.source.display().to_string();
        let chapter = chapter_map.get(&key).copied();
        crate::paths::set_active_target(target_owned.as_deref());
        crate::paths::set_project_root(Some(base_dir));
        let result = render_one_document_pass1(page, &overrides, base_dir, output_dir, chapter);
        (key, result)
    });

    // Collect pass 1 results
    let mut pass1_results: HashMap<String, Pass1Result> = HashMap::new();
    for (key, result) in pass1 {
        match result {
            Ok(r) => { pass1_results.insert(key, r); }
            Err(e) => { eprintln!("Error rendering {}: {:#}", key, e); }
        }
    }

    // Build per-language registries from all pages' ref data.
    // Multilingual sites can have duplicate IDs across languages (e.g., sec-code
    // in both English and French pages), so each language gets its own registry.
    let has_languages = !meta.languages.is_empty();
    let mut lang_registry_input: HashMap<Option<String>, Vec<(usize, String, crate::crossref::PageRefData)>> = HashMap::new();
    for page in pages {
        let key = page.source.display().to_string();
        if let Some(r) = pass1_results.get(&key) {
            if let Some(ref ref_data) = r.ref_data {
                let chapter = chapter_map.get(&key).copied().unwrap_or(0);
                let url = page.output.display().to_string();
                let lang_key = if has_languages { page.lang.clone() } else { None };
                lang_registry_input.entry(lang_key)
                    .or_default()
                    .push((chapter, url, ref_data.clone()));
            }
        }
    }

    let mut lang_registries: HashMap<Option<String>, CrossRefRegistry> = HashMap::new();
    for (lang, input) in &lang_registry_input {
        let registry = CrossRefRegistry::build(input);
        lang_registries.insert(lang.clone(), registry);
    }

    let total_ids: usize = lang_registries.values().map(|r| r.entries.len()).sum();
    if !quiet {
        eprintln!("Cross-ref pass 2: resolving {} IDs across {} pages...",
            total_ids, pass1_results.len());
    }

    // Pass 2: Resolve cross-refs and renumber (cheap string ops, sequential)
    let empty_registry = CrossRefRegistry::default();
    let mut map = HashMap::new();
    for page in pages {
        let key = page.source.display().to_string();
        if let Some(r) = pass1_results.remove(&key) {
            let lang_key = if has_languages { page.lang.clone() } else { None };
            let registry = lang_registries.get(&lang_key).unwrap_or(&empty_registry);
            let current_url = page.output.display().to_string();
            let body = resolve_html_global(&r.body, registry, &current_url);
            let body = renumber_display_html(&body, registry);

            map.insert(key, CollectionRenderResult {
                body,
                toc: r.toc,
                title: r.title,
                date: r.date,
                subtitle: r.subtitle,
                abstract_text: r.abstract_text,
            });
        }
    }

    Ok(map)
}

/// Intermediate result from pass 1 (before cross-ref resolution).
struct Pass1Result {
    body: String,
    toc: Option<String>,
    title: Option<String>,
    date: Option<String>,
    subtitle: Option<String>,
    abstract_text: Option<String>,
    ref_data: Option<crate::crossref::PageRefData>,
}

/// Render a single document for pass 1: skip cross-ref resolution, collect ref data.
fn render_one_document_pass1(
    page: &DocumentInfo,
    overrides: &[String],
    base_dir: &Path,
    output_dir: &Path,
    chapter_number: Option<usize>,
) -> Result<Pass1Result> {
    let input = base_dir.join(&page.source);
    let output_path = output_dir.join(&page.output);

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let options = crate::render::pipeline::RenderCoreOptions {
        skip_crossref: true,
        chapter_number,
    };
    let result = crate::render::pipeline::render_core(
        &input, &output_path, Some("html"), overrides, Some(base_dir), &options, None, None,
    )?;

    // Collect cross-ref data for global resolution in pass 2 (before moving body)
    let pipeline = crate::render::formats::FormatPipeline::from_writer("html")?;
    let ref_data = pipeline.collect_crossref_data(&result.rendered, &result.element_renderer);

    // Run document transforms (footnotes, highlight CSS, etc.)
    let body = pipeline.transform_document(&result.rendered, &result.element_renderer);

    let toc = if result.metadata.toc.as_ref().and_then(|t| t.enabled).unwrap_or(true) {
        let depth = result.metadata.toc.as_ref().and_then(|t| t.depth).unwrap_or(3) as u8;
        let title = result.metadata.toc.as_ref().and_then(|t| t.title.as_deref()).unwrap_or("Contents");
        let toc_html = crate::render::template::build_toc_html_from_body(&body, depth, title);
        if toc_html.is_empty() { None } else { Some(toc_html) }
    } else {
        None
    };

    Ok(Pass1Result {
        body,
        toc,
        title: result.metadata.title.map(|t| crate::render::convert::render_inline(&t, "html")),
        date: result.metadata.date,
        subtitle: result.metadata.subtitle.map(|t| crate::render::convert::render_inline(&t, "html")),
        abstract_text: result.metadata.abstract_text,
        ref_data,
    })
}

/// Assign chapter numbers to pages based on their position in [[contents]].
/// Each page listed in [[contents]] gets a sequential chapter number (1-based).
/// Pages not in [[contents]] get no chapter number.
fn assign_chapter_numbers(meta: &crate::config::Metadata) -> HashMap<String, usize> {
    let mut chapter_map = HashMap::new();
    let mut chapter = 0usize;

    for section in &meta.contents {
        // Section index page gets its own chapter number
        if let Some(href) = section.display_href() {
            if href.ends_with(".qmd") {
                chapter += 1;
                chapter_map.insert(href.to_string(), chapter);
            }
        }

        for entry in &section.resolved_include() {
            let entry_path = match entry {
                crate::config::IncludeEntry::Path(p) => p.as_str(),
                crate::config::IncludeEntry::Item { href: Some(h), .. } => h.as_str(),
                _ => continue,
            };
            for path in super::contents::expand_glob(entry_path, std::path::Path::new("")) {
                if path.ends_with(".qmd") {
                    if section.display_href().is_none() {
                        chapter += 1;
                    }
                    chapter_map.insert(path, chapter);
                }
            }
        }
    }

    // Pages not in [[contents]] won't be in chapter_map (chapter_number = None).

    chapter_map
}

/// Create a progress bar for rendering a set of pages.
fn create_progress_bar(total: u64, quiet: bool) -> Arc<ProgressBar> {
    let pb = if quiet {
        ProgressBar::hidden()
    } else {
        let pb = ProgressBar::new(total);
        pb.set_style(ProgressStyle::default_bar()
            .template("  [{bar:30}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=> "));
        pb
    };
    Arc::new(pb)
}

/// Render pages in parallel using thread::scope, with a shared progress bar.
///
/// The closure `render_fn` receives a `&DocumentInfo` and returns `(key, Result<T>)`.
/// It is called inside a spawned thread for each page.
fn render_parallel<T, F>(
    pages: &[DocumentInfo],
    quiet: bool,
    render_fn: F,
) -> Vec<(String, Result<T>)>
where
    T: Send,
    F: Fn(&DocumentInfo) -> (String, Result<T>) + Sync,
{
    let pb = create_progress_bar(pages.len() as u64, quiet);
    let results = std::thread::scope(|s| {
        let handles: Vec<_> = pages
            .iter()
            .map(|page| {
                let pb = Arc::clone(&pb);
                let render_fn = &render_fn;
                s.spawn(move || {
                    let result = render_fn(page);
                    pb.set_message(page.output.display().to_string());
                    pb.inc(1);
                    result
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    pb.finish_and_clear();
    results
}

pub fn build_overrides(meta: &crate::config::Metadata, target: Option<&config::Target>) -> Vec<String> {
    let mut overrides = Vec::new();

    // embed-resources override from target
    if let Some(t) = target {
        if let Some(embed) = t.embed_resources {
            overrides.push(format!("embed-resources={}", embed));
        }
    }

    // Highlight style from metadata
    if let Some(ref hl) = meta.highlight {
        if let Some(ref light) = hl.light {
            overrides.push(format!("highlight-style.light={}", light));
        }
        if let Some(ref dark) = hl.dark {
            overrides.push(format!("highlight-style.dark={}", dark));
        }
    }

    overrides
}
