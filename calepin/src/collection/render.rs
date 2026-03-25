use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;

use crate::project;
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
    meta: &crate::metadata::Metadata,
    base_dir: &Path,
    output_dir: &Path,
    format: &str,
    apply_page_template: bool,
    target_name: Option<&str>,
    target: Option<&project::Target>,
    quiet: bool,
) -> Result<HashMap<String, CollectionRenderResult>> {
    if pages.is_empty() {
        return Ok(HashMap::new());
    }

    // Resolve defaults from metadata, letting target override embed-resources
    let mut defaults = project::resolve_defaults(Some(&meta.defaults));
    if let Some(t) = target {
        if let Some(embed) = t.embed_resources {
            defaults.embed_resources = Some(embed);
        }
    }

    let overrides = build_overrides(&defaults);

    if !quiet {
        eprintln!("Rendering {} documents...", pages.len());
    }
    let format_owned = format.to_string();
    let target_owned = target_name.map(|s| s.to_string());
    let total = pages.len();
    let done = AtomicUsize::new(0);

    // Render all pages in parallel using thread::scope
    let results: Vec<(String, Result<CollectionRenderResult>)> = std::thread::scope(|s| {
        let handles: Vec<_> = pages
            .iter()
            .map(|page| {
                let overrides = &overrides;
                let base_dir = base_dir;
                let output_dir = output_dir;
                let format = &format_owned;
                let target = &target_owned;
                let project_meta = meta;
                let done = &done;
                let quiet = quiet;
                s.spawn(move || {
                    crate::paths::set_active_target(target.as_deref());
                    crate::paths::set_project_root(Some(base_dir));
                    let key = page.source.display().to_string();
                    let result = render_one_document(page, overrides, base_dir, output_dir, format, apply_page_template, Some(project_meta));
                    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if !quiet {
                        let out = output_dir.join(&page.output);
                        eprintln!("  [{}/{}] {}", n, total, out.display());
                    }
                    (key, result)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
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
    project_metadata: Option<&crate::metadata::Metadata>,
) -> Result<CollectionRenderResult> {
    let input = base_dir.join(&page.source);
    let output_path = output_dir.join(&page.output);

    // Ensure the output parent directory exists before rendering,
    // so figure files can be written alongside the output.
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let result = crate::pipeline::render_core(&input, &output_path, Some(format), overrides, Some(base_dir), &crate::pipeline::RenderCoreOptions::default(), project_metadata)?;

    let body = if apply_page_template {
        // Apply the project's page template (e.g., book's minimal page.tex)
        let renderer = crate::formats::create_renderer(format)?;
        renderer.assemble_page(&result.rendered, &result.metadata, &result.element_renderer)
            .unwrap_or(result.rendered)
    } else if format == "html" {
        // HTML site mode: prepend syntax highlighting CSS, append footnotes
        let syntax_css = result.element_renderer.syntax_css_with_scope(
            crate::render::highlighting::ColorScope::DataTheme,
        );
        let footnotes = result.element_renderer.render_footnote_section();
        let mut body = result.rendered;
        if !syntax_css.is_empty() {
            body = format!("<style>\n{}</style>\n{}", syntax_css, body);
        }
        if !footnotes.is_empty() {
            body.push_str(&footnotes);
        }
        body
    } else {
        result.rendered
    };

    // Build TOC from rendered headings (HTML only)
    let toc = if format == "html" && result.metadata.toc.unwrap_or(true) {
        let depth = if result.metadata.toc_depth == 0 { 3 } else { result.metadata.toc_depth };
        let title = result.metadata.toc_title.as_deref().unwrap_or("Contents");
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
    meta: &crate::metadata::Metadata,
    base_dir: &Path,
    output_dir: &Path,
    target_name: Option<&str>,
    target: Option<&project::Target>,
    quiet: bool,
) -> Result<HashMap<String, CollectionRenderResult>> {
    use crate::crossref::{CrossRefRegistry, resolve_html_global, renumber_display_html};

    if pages.is_empty() {
        return Ok(HashMap::new());
    }

    let mut defaults = project::resolve_defaults(Some(&meta.defaults));
    if let Some(t) = target {
        if let Some(embed) = t.embed_resources {
            defaults.embed_resources = Some(embed);
        }
    }

    let overrides = build_overrides(&defaults);

    if !quiet {
        eprintln!("Rendering {} documents (cross-ref pass 1)...", pages.len());
    }

    // Assign chapter numbers based on [[contents]] ordering
    let chapter_map = assign_chapter_numbers(meta);

    let target_owned = target_name.map(|s| s.to_string());
    let total = pages.len();
    let done = AtomicUsize::new(0);

    // Pass 1: Render all pages in parallel with skip_crossref=true
    let pass1: Vec<(String, Result<Pass1Result>)> = std::thread::scope(|s| {
        let handles: Vec<_> = pages
            .iter()
            .map(|page| {
                let overrides = &overrides;
                let base_dir = base_dir;
                let output_dir = output_dir;
                let target = &target_owned;
                let chapter = chapter_map.get(&page.source.display().to_string()).copied();
                let done = &done;
                let quiet = quiet;
                s.spawn(move || {
                    crate::paths::set_active_target(target.as_deref());
                    crate::paths::set_project_root(Some(base_dir));
                    let key = page.source.display().to_string();
                    let result = render_one_document_pass1(page, overrides, base_dir, output_dir, chapter);
                    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if !quiet {
                        let out = output_dir.join(&page.output);
                        eprintln!("  [{}/{}] {}", n, total, out.display());
                    }
                    (key, result)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
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

    let options = crate::pipeline::RenderCoreOptions {
        skip_crossref: true,
        chapter_number,
    };
    let result = crate::pipeline::render_core(
        &input, &output_path, Some("html"), overrides, Some(base_dir), &options, None,
    )?;

    // Collect cross-ref data for global resolution in pass 2 (before moving body)
    let html_renderer = crate::formats::create_renderer("html")?;
    let ref_data = html_renderer.collect_crossref_data(&result.rendered, &result.element_renderer);

    // HTML site mode: prepend syntax highlighting CSS, append footnotes
    let syntax_css = result.element_renderer.syntax_css_with_scope(
        crate::render::highlighting::ColorScope::DataTheme,
    );
    let footnotes = result.element_renderer.render_footnote_section();
    let mut body = result.rendered;
    if !syntax_css.is_empty() {
        body = format!("<style>\n{}</style>\n{}", syntax_css, body);
    }
    if !footnotes.is_empty() {
        body.push_str(&footnotes);
    }

    let toc = if result.metadata.toc.unwrap_or(true) {
        let depth = if result.metadata.toc_depth == 0 { 3 } else { result.metadata.toc_depth };
        let title = result.metadata.toc_title.as_deref().unwrap_or("Contents");
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
/// Each non-standalone page gets a sequential chapter number (1-based).
/// Returns a map from source path (string) to chapter number.
fn assign_chapter_numbers(meta: &crate::metadata::Metadata) -> HashMap<String, usize> {
    let mut chapter_map = HashMap::new();
    let mut chapter = 0usize;

    // Walk the contents sections in order -- this mirrors collect_document_paths ordering
    for section in &meta.contents {
        if section.standalone {
            continue;
        }

        // Section index page gets its own chapter number
        if let Some(ref idx) = section.index {
            if idx.ends_with(".qmd") {
                chapter += 1;
                chapter_map.insert(idx.clone(), chapter);
            }
        }

        for entry in &section.pages {
            for path in crate::project::expand_glob_pub(entry.path(), std::path::Path::new("")) {
                if path.ends_with(".qmd") {
                    // If no index page, each page in the section is a chapter
                    if section.index.is_none() {
                        chapter += 1;
                    }
                    chapter_map.insert(path, chapter);
                }
            }
        }
    }

    // Also handle pages not in contents (standalone pages) -- no chapter number
    // They won't be in chapter_map, which is fine (chapter_number = None).

    chapter_map
}

fn build_overrides(defaults: &project::Defaults) -> Vec<String> {
    let mut overrides = Vec::new();

    // Highlight style from defaults
    if let Some(ref hl) = defaults.highlight {
        if let Some(ref light) = hl.light {
            overrides.push(format!("highlight-style.light={}", light));
        }
        if let Some(ref dark) = hl.dark {
            overrides.push(format!("highlight-style.dark={}", dark));
        }
    }

    overrides
}
