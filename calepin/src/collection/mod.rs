mod assets;
mod config;
mod context;
mod discover;
mod icons;
mod render;
mod templates;

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use context::{build_document_context, build_collection_context, build_nav_tree_for_lang, mark_active, ListingItem};
use discover::{discover_listing_documents, discover_documents, discover_standalone_documents, DocumentInfo};
use crate::project::{DocumentNode, expand_contents};

/// Extract the first <img> src from rendered HTML.
fn extract_first_image(html: &str) -> Option<String> {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r#"<img[^>]+src="([^"]+)""#).unwrap()
    });
    RE.captures(html).map(|c| c[1].to_string())
}

/// Build a ListingItem from a page info and optional rendered result.
fn build_listing_item(
    lp: &DocumentInfo,
    results: &HashMap<String, render::CollectionRenderResult>,
) -> ListingItem {
    let image = lp.meta.image.clone().or_else(|| {
        let key = lp.source.display().to_string();
        results.get(&key).and_then(|r| extract_first_image(&r.body))
    });
    ListingItem {
        title: lp.meta.title.as_ref().map(|t| crate::render::convert::render_inline(t, "html")),
        date: lp.meta.date.as_ref().map(|d| crate::collection::context::format_date(d)),
        description: lp.meta.description.clone(),
        image,
        url: lp.url.clone(),
    }
}

/// Build a collection from .qmd files.
/// `cli_target` overrides `[site].target` when provided via `-t` on the command line.
pub fn build_collection(
    config_path: Option<&Path>,
    output: &Path,
    clean: bool,
    quiet: bool,
    cli_target: Option<&str>,
) -> Result<()> {
    let cwd = std::env::current_dir()?;

    // 1. Load config
    let (config, found_path) = config::load_config(config_path, &cwd)?;

    let base_dir = found_path.parent().unwrap_or(&cwd).to_path_buf();
    if !quiet {
        eprintln!("Config: {}", found_path.display());
    }

    // 2. Resolve collection target (format and extension)
    //    CLI -f flag takes precedence over [site].target, which defaults to "html".
    let collection_target_name = cli_target.map(|s| s.to_string())
        .or_else(|| config.target.clone())
        .unwrap_or_else(|| "html".to_string());
    let collection_target = crate::project::resolve_target(&collection_target_name, Some(&config))?;
    let format = &collection_target.engine;
    let output_ext = collection_target.output_extension();

    // Set active target and project root for template/component resolution
    crate::paths::set_active_target(Some(&collection_target_name));
    crate::paths::set_project_root(Some(&base_dir));

    // Auto-detect orchestrator: check templates/{target}/orchestrator.{ext}
    // Falls back to built-in templates if not found on filesystem.
    let ext = crate::paths::engine_to_ext(format);
    let orchestrator_filename = format!("orchestrator.{}", ext);
    let orchestrator = config.orchestrator.clone()
        .or_else(|| {
            let p = base_dir.join("_calepin").join("templates").join(&collection_target_name)
                .join(&orchestrator_filename);
            if p.exists() { return Some(p.display().to_string()); }
            // Check built-in templates
            let builtin_path = format!("templates/{}/{}", collection_target_name, orchestrator_filename);
            if crate::render::elements::BUILTIN_PROJECT.get_file(&builtin_path).is_some() {
                Some(format!("__builtin__:{}", builtin_path))
            } else {
                None
            }
        });

    // 3. Prepare output directory
    let output = if output.is_relative() {
        &base_dir.join(output)
    } else {
        output
    };
    if clean && output.exists() {
        fs::remove_dir_all(output)
            .with_context(|| format!("Failed to clean output dir: {}", output.display()))?;
    }
    fs::create_dir_all(output)?;

    // 4. Discover pages (nav + standalone)
    let mut pages = discover_documents(&config, &base_dir, output_ext)?;
    let standalone = discover_standalone_documents(&config, &base_dir, output_ext)?;
    pages.extend(standalone);
    if !quiet {
        eprintln!("Found {} documents", pages.len());
    }

    // 5. Discover listing pages and merge into the page list
    let mut all_listing_documents: HashMap<String, Vec<DocumentInfo>> = HashMap::new();
    for page in &pages {
        if let Some(ref listing) = page.meta.listing {
            let listing_documents = discover_listing_documents(listing, &base_dir, &pages, output_ext)?;
            all_listing_documents.insert(page.source.display().to_string(), listing_documents);
        }
    }
    let mut existing_sources: std::collections::HashSet<String> = pages.iter().map(|p| p.source.display().to_string()).collect();
    for listing_documents in all_listing_documents.values() {
        for lp in listing_documents {
            let key = lp.source.display().to_string();
            if existing_sources.insert(key) {
                pages.push(lp.clone());
            }
        }
    }

    // 6. Render all pages with calepin (in parallel)
    //    For HTML sites without an orchestrator, use the two-pass cross-ref pipeline
    //    so that cross-file references resolve globally.
    //    For orchestrated builds (LaTeX/Typst), use single-pass with page template
    //    (the native toolchains handle global refs).
    let apply_page_template = orchestrator.is_some();
    let results = if format == "html" && !apply_page_template && config.global_crossref {
        render::render_documents_with_crossref(&pages, &config, &base_dir, output, Some(&collection_target_name), Some(&collection_target), quiet)?
    } else {
        render::render_documents(&pages, &config, &base_dir, output, format, apply_page_template, Some(&collection_target_name), Some(&collection_target), quiet)?
    };

    // 7. Write page output files
    //    For orchestrated builds, strip the output directory prefix from paths
    //    in the rendered bodies so they resolve correctly when the compile
    //    command runs from the output directory.
    let output_prefix = format!("{}/", output.display());
    for page in &pages {
        let source_key = page.source.display().to_string();
        if let Some(result) = results.get(&source_key) {
            let output_path = output.join(&page.output);
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let body = if orchestrator.is_some() {
                result.body.replace(&output_prefix, "")
            } else {
                result.body.clone()
            };
            fs::write(&output_path, &body)?;
        }
    }

    // 8. Site-specific wrapping (HTML) or orchestrator assembly
    if let Some(ref orchestrator_path) = orchestrator {
        // Render the orchestrator template with page tree
        render_orchestrator(&config, &pages, &results, &base_dir, output, orchestrator_path, format, output_ext, &collection_target_name, quiet)?;
    } else {
        // HTML site path: re-wrap pages through Jinja site templates
        apply_collection_templates(&config, &pages, &results, &all_listing_documents, &base_dir, output, format, &collection_target_name)?;
    }

    // 9. Copy assets/ and static directories to output
    assets::copy_assets(&base_dir, output, &config.static_dirs)?;

    // 10. Run user-configured post-processing commands
    run_post_commands(&config, &collection_target_name, &base_dir, output, quiet)?;

    if !quiet {
        eprintln!("Collection built: {}", output.display());
    }

    Ok(())
}

/// Rebuild only the specified documents within an already-built site.
/// Discovers all documents (for nav context) but only re-renders those whose
/// source paths are in `changed_sources`. Skips the clean step and assets copy.
pub fn rebuild_documents(
    config_path: Option<&Path>,
    cli_target: Option<&str>,
    changed_sources: &[std::path::PathBuf],
) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let (config, found_path) = config::load_config(config_path, &cwd)?;
    let base_dir = found_path.parent().unwrap_or(&cwd).to_path_buf();

    let collection_target_name = cli_target.map(|s| s.to_string())
        .or_else(|| config.target.clone())
        .unwrap_or_else(|| "html".to_string());
    let collection_target = crate::project::resolve_target(&collection_target_name, Some(&config))?;
    let format = &collection_target.engine;
    let output_ext = collection_target.output_extension();

    crate::paths::set_active_target(Some(&collection_target_name));
    crate::paths::set_project_root(Some(&base_dir));

    let output_dir = base_dir.join(config.output.as_deref().unwrap_or("output"));

    // Discover all pages (needed for nav context), including standalone
    let mut pages = discover_documents(&config, &base_dir, output_ext)?;
    let standalone = discover_standalone_documents(&config, &base_dir, output_ext)?;
    pages.extend(standalone);

    // Determine which pages to re-render by matching changed absolute paths
    // against the discovered page sources. Canonicalize both sides so that
    // symlinks and path normalization differences don't cause mismatches.
    let canon_changed: Vec<std::path::PathBuf> = changed_sources.iter()
        .filter_map(|c| c.canonicalize().ok())
        .collect();
    let changed_documents: Vec<&DocumentInfo> = pages.iter().filter(|p| {
        let abs = base_dir.join(&p.source);
        let canon = abs.canonicalize().unwrap_or(abs);
        canon_changed.iter().any(|c| c == &canon)
    }).collect();

    if changed_documents.is_empty() {
        return Ok(());
    }

    // Render only the changed pages
    let documents_to_render: Vec<DocumentInfo> = changed_documents.iter().map(|p| (*p).clone()).collect();
    let results = render::render_documents(
        &documents_to_render, &config, &base_dir, &output_dir, format,
        false, Some(&collection_target_name), Some(&collection_target), true,
    )?;

    // Write raw body files
    for page in &documents_to_render {
        let source_key = page.source.display().to_string();
        if let Some(result) = results.get(&source_key) {
            let output_path = output_dir.join(&page.output);
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&output_path, &result.body)?;
        }
    }

    // Apply collection templates to the changed pages (with full nav context)
    if format == "html" {
        let env = templates::init_jinja(&base_dir, &collection_target_name)?
            .ok_or_else(|| anyhow::anyhow!("No template files found"))?;

        let collection_ctx = build_collection_context(&config, &pages, &base_dir);
        let var_ctx = config.var.as_ref()
            .map(|v| crate::project::target_vars_to_jinja(Some(v)))
            .unwrap_or_else(|| minijinja::Value::from(()));

        let tpl_ext = match format.as_str() {
            "latex" => "tex",
            "typst" => "typ",
            "markdown" => "md",
            _ => format,
        };

        // Also discover listings that the changed pages might need
        let mut all_listing_documents: HashMap<String, Vec<DocumentInfo>> = HashMap::new();
        for page in &documents_to_render {
            if let Some(ref listing) = page.meta.listing {
                let listing_documents = discover_listing_documents(listing, &base_dir, &pages, output_ext)?;
                all_listing_documents.insert(page.source.display().to_string(), listing_documents);
            }
        }

        for page in &documents_to_render {
            let source_key = page.source.display().to_string();
            let result = results.get(&source_key);

            let listing_items = all_listing_documents.get(&source_key).map(|listing_documents| {
                listing_documents.iter().map(|lp| build_listing_item(lp, &results)).collect()
            });

            let doc_ctx = build_document_context(page, result, &pages, listing_items, &config.languages);

            let mut nav_tree = if !config.languages.is_empty() {
                if let Some(ref lang) = page.lang {
                    build_nav_tree_for_lang(&config, &pages, &base_dir, lang)
                } else {
                    collection_ctx.pages.clone()
                }
            } else {
                collection_ctx.pages.clone()
            };
            mark_active(&mut nav_tree, &page.url);

            let collection_with_active = minijinja::context! {
                collection => context::CollectionContext {
                    title: collection_ctx.title.clone(),
                    subtitle: collection_ctx.subtitle.clone(),
                    url: collection_ctx.url.clone(),
                    favicon: collection_ctx.favicon.clone(),
                    logo: collection_ctx.logo.clone(),
                    logo_dark: collection_ctx.logo_dark.clone(),
                    pages: nav_tree,
                    languages: collection_ctx.languages.clone(),
                    dark_mode: collection_ctx.dark_mode,
                    math: collection_ctx.math.clone(),
                },
                document => doc_ctx,
                var => var_ctx.clone(),
            };

            let template_name = if page.meta.listing.is_some() {
                format!("listing.{}", tpl_ext)
            } else {
                format!("page.{}", tpl_ext)
            };

            let tpl = env.get_template(&template_name)
                .with_context(|| format!("Failed to get template {} for {}", template_name, source_key))?;
            let rendered = tpl.render(&collection_with_active)
                .with_context(|| format!("Failed to render template for {}", source_key))?;

            let output_path = output_dir.join(&page.output);
            fs::write(&output_path, &rendered)?;
        }

        // Update _source/ copies for the changed pages
        for page in &documents_to_render {
            let source_dest = output_dir.join("_source").join(&page.source);
            if let Some(parent) = source_dest.parent() {
                fs::create_dir_all(parent)?;
            }
            let source_src = base_dir.join(&page.source);
            if source_src.exists() {
                fs::copy(&source_src, &source_dest)?;
            }
        }
    }

    Ok(())
}

/// HTML collection path: wrap each document's body through Jinja site templates
/// (page.html, listing.html with extends/includes).
/// Overwrites the raw body files written in step 7 with fully templated HTML.
fn apply_collection_templates(
    config: &crate::project::ProjectConfig,
    pages: &[DocumentInfo],
    results: &HashMap<String, render::CollectionRenderResult>,
    all_listing_documents: &HashMap<String, Vec<DocumentInfo>>,
    base_dir: &Path,
    output: &Path,
    format: &str,
    target_name: &str,
) -> Result<()> {
    // Initialize MiniJinja from templates/{target}/
    let env = templates::init_jinja(base_dir, target_name)?
        .ok_or_else(|| anyhow::anyhow!(
            "No template files found in templates/{}/. \
             At least base and page templates are required for multi-file collection mode.",
            target_name
        ))?;

    // Build collection context
    let collection_ctx = build_collection_context(config, pages, base_dir);

    // Convert [var] to minijinja Value for template access
    let var_ctx = config.var.as_ref()
        .map(|v| crate::project::target_vars_to_jinja(Some(v)))
        .unwrap_or_else(|| minijinja::Value::from(()));

    // Determine template extension
    let tpl_ext = match format {
        "latex" => "tex",
        "typst" => "typ",
        "markdown" => "md",
        _ => format,
    };

    // Render each page through MiniJinja, overwriting the raw body files
    for page in pages {
        let source_key = page.source.display().to_string();
        let result = results.get(&source_key);

        let all_listing_items: Option<Vec<ListingItem>> = all_listing_documents.get(&source_key).map(|listing_documents| {
            listing_documents.iter().map(|lp| build_listing_item(lp, results)).collect()
        });

        // Determine pagination: split listing items into pages if page-size is set
        let page_size = page.meta.listing.as_ref().map(|l| l.page_size).unwrap_or(0);
        let paginated: Vec<(Vec<ListingItem>, Option<context::Pagination>)> = if page_size > 0 {
            if let Some(ref items) = all_listing_items {
                let chunks: Vec<Vec<ListingItem>> = items.chunks(page_size).map(|c| c.to_vec()).collect();
                let total = chunks.len();
                let base_url = page.url.trim_end_matches(".html");
                chunks.into_iter().enumerate().map(|(i, chunk)| {
                    let current = i + 1;
                    let prev_url = if current > 1 {
                        if current == 2 { Some(format!("{}.html", base_url)) }
                        else { Some(format!("{}/page/{}.html", base_url, current - 1)) }
                    } else { None };
                    let next_url = if current < total {
                        Some(format!("{}/page/{}.html", base_url, current + 1))
                    } else { None };
                    (chunk, Some(context::Pagination { current, total, prev_url, next_url }))
                }).collect()
            } else {
                vec![(vec![], None)]
            }
        } else {
            vec![(all_listing_items.unwrap_or_default(), None)]
        };

        // Build language-specific nav tree
        let mut nav_tree = if !config.languages.is_empty() {
            if let Some(ref lang) = page.lang {
                build_nav_tree_for_lang(config, pages, base_dir, lang)
            } else {
                collection_ctx.pages.clone()
            }
        } else {
            collection_ctx.pages.clone()
        };
        mark_active(&mut nav_tree, &page.url);

        let template_name = if page.meta.listing.is_some() {
            format!("listing.{}", tpl_ext)
        } else {
            format!("page.{}", tpl_ext)
        };
        let tpl = env.get_template(&template_name)
            .with_context(|| format!("Failed to get template {} for {}", template_name, source_key))?;

        for (page_idx, (items, pagination)) in paginated.iter().enumerate() {
            let listing = if items.is_empty() { None } else { Some(items.clone()) };
            let mut doc_ctx = build_document_context(page, result, pages, listing, &config.languages);
            doc_ctx.pagination = pagination.clone();

            let collection_with_active = minijinja::context! {
                collection => context::CollectionContext {
                    title: collection_ctx.title.clone(),
                    subtitle: collection_ctx.subtitle.clone(),
                    url: collection_ctx.url.clone(),
                    favicon: collection_ctx.favicon.clone(),
                    logo: collection_ctx.logo.clone(),
                    logo_dark: collection_ctx.logo_dark.clone(),
                    pages: nav_tree.clone(),
                    languages: collection_ctx.languages.clone(),
                    dark_mode: collection_ctx.dark_mode,
                    math: collection_ctx.math.clone(),
                },
                document => doc_ctx,
                var => var_ctx.clone(),
            };

            let rendered = tpl.render(&collection_with_active)
                .with_context(|| format!("Failed to render template for {}", source_key))?;

            let output_path = if page_idx == 0 {
                output.join(&page.output)
            } else {
                let base = page.output.with_extension("");
                let paginated_path = base.join("page").join(format!("{}.html", page_idx + 1));
                let full = output.join(&paginated_path);
                if let Some(parent) = full.parent() {
                    fs::create_dir_all(parent)?;
                }
                full
            };
            fs::write(&output_path, &rendered)?;
        }
    }

    if format == "html" {
        // Copy .qmd source files to _source/ for the source viewer
        for page in pages {
            let source_dest = output.join("_source").join(&page.source);
            if let Some(parent) = source_dest.parent() {
                fs::create_dir_all(parent)?;
            }
            let source_src = base_dir.join(&page.source);
            if source_src.exists() {
                fs::copy(&source_src, &source_dest)?;
            }
        }
    }

    Ok(())
}

/// Run user-configured post-processing commands from `[[post]]` in calepin.toml.
///
/// Each command is executed from the project root. `{output}` and `{root}` in the
/// command string are replaced with the output directory and project root paths.
/// Commands with a `targets` restriction are skipped if the active target doesn't match.
fn run_post_commands(
    config: &crate::project::ProjectConfig,
    target: &str,
    project_root: &Path,
    output: &Path,
    quiet: bool,
) -> Result<()> {
    for post in &config.post {
        if !post.targets.is_empty() && !post.targets.iter().any(|t| t == target) {
            continue;
        }

        let cmd = post.command
            .replace("{output}", &output.display().to_string())
            .replace("{root}", &project_root.display().to_string());

        if !quiet {
            eprintln!("  post: {}", cmd);
        }

        let result = std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .current_dir(project_root)
            .output();

        match result {
            Ok(out) => {
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    cwarn!("post command failed: {}", cmd);
                    if !stderr.trim().is_empty() {
                        eprintln!("  {}", stderr.trim());
                    }
                }
            }
            Err(e) => {
                cwarn!("failed to run post command: {}: {}", cmd, e);
            }
        }
    }
    Ok(())
}

/// Render the orchestrator template with the document tree.
/// Fragment files are already written; this produces the master file
/// that references them via \include{} or equivalent.
fn render_orchestrator(
    config: &crate::project::ProjectConfig,
    pages: &[DocumentInfo],
    results: &HashMap<String, render::CollectionRenderResult>,
    base_dir: &Path,
    output: &Path,
    orchestrator_path: &str,
    format: &str,
    output_ext: &str,
    target_name: &str,
    quiet: bool,
) -> Result<()> {
    // Build the document tree with titles and paths
    let document_tree = expand_contents(&config.contents, base_dir);

    let document_map: HashMap<String, &DocumentInfo> = pages.iter()
        .map(|p| (p.source.display().to_string(), p))
        .collect();

    let nav_nodes = build_orchestrator_tree(&document_tree, &document_map, results, format);

    // Build template context
    let meta_ctx = minijinja::context! {
        title => config.title.clone(),
        subtitle => config.subtitle.clone(),
        author => config.author.as_ref().map(|a| format_author(a)),
        url => config.url.clone(),
    };

    let var_ctx = config.var.as_ref()
        .map(|v| crate::project::target_vars_to_jinja(Some(v)))
        .unwrap_or_else(|| minijinja::Value::from(()));

    // Format-specific defaults for orchestrator templates
    let defs = crate::project::get_defaults();
    let latex_defs = defs.latex.as_ref();
    let typst_defs = defs.typst.as_ref();
    let label_defs = defs.labels.as_ref();

    let ctx = minijinja::context! {
        meta => meta_ctx,
        var => var_ctx,
        pages => nav_nodes,
        format => format,
        base => format,
        latex_documentclass => latex_defs.and_then(|l| l.documentclass.as_deref()).unwrap_or("article"),
        latex_fontsize => latex_defs.and_then(|l| l.fontsize.as_deref()).unwrap_or("11pt"),
        latex_linkcolor => latex_defs.and_then(|l| l.linkcolor.as_deref()).unwrap_or("black"),
        latex_urlcolor => latex_defs.and_then(|l| l.urlcolor.as_deref()).unwrap_or("blue!60!black"),
        latex_citecolor => latex_defs.and_then(|l| l.citecolor.as_deref()).unwrap_or("blue!60!black"),
        typst_fontsize => typst_defs.and_then(|t| t.fontsize.as_deref()).unwrap_or("11pt"),
        typst_leading => typst_defs.and_then(|t| t.leading.as_deref()).unwrap_or("0.65em"),
        typst_justify => typst_defs.and_then(|t| t.justify).unwrap_or(true),
        typst_heading_numbering => typst_defs.and_then(|t| t.heading_numbering.as_deref()).unwrap_or("1.1"),
        label_contents => label_defs.and_then(|l| l.contents.as_deref()).unwrap_or("Contents"),
    };

    // Collect all template sources into an owned map for the loader.
    let mut templates = std::collections::HashMap::new();

    // Load templates from templates/{target}/ and templates/common/
    let dirs = [
        base_dir.join("_calepin").join("templates").join(target_name),
        base_dir.join("_calepin").join("templates").join("common"),
    ];
    for dir in &dirs {
        if !dir.is_dir() { continue; }
        let pattern = dir.join("**").join("*.*");
        let pattern_str = pattern.display().to_string();
        for entry in glob::glob(&pattern_str).unwrap_or_else(|_| glob::glob("").unwrap()) {
            if let Ok(path) = entry {
                if !path.is_file() { continue; }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let rel = path.strip_prefix(dir).unwrap_or(&path);
                    let name = rel.display().to_string();
                    // Don't overwrite target-specific with common (target loaded first)
                    templates.entry(name).or_insert(content);
                }
            }
        }
    }

    // Also load built-in templates as fallback (target-specific + common)
    for builtin_dir_name in &[format!("templates/{}", target_name), "templates/common".to_string()] {
        for entry in crate::render::elements::BUILTIN_PROJECT.get_dir(builtin_dir_name.as_str()).into_iter().flat_map(|d| d.files()) {
            if let Some(content) = entry.contents_utf8() {
                let name = entry.path().file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                if !name.is_empty() {
                    templates.entry(name.to_string()).or_insert_with(|| content.to_string());
                }
            }
        }
    }

    // Load the orchestrator template itself
    let tpl_source = if let Some(builtin_path) = orchestrator_path.strip_prefix("__builtin__:") {
        crate::render::elements::BUILTIN_PROJECT.get_file(builtin_path)
            .and_then(|f| f.contents_utf8())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Built-in orchestrator template not found: {}", builtin_path))?
    } else {
        let tpl_path = base_dir.join(orchestrator_path);
        fs::read_to_string(&tpl_path)
            .with_context(|| format!("Failed to read orchestrator template: {}", tpl_path.display()))?
    };
    templates.insert("orchestrator".to_string(), tpl_source);

    let mut env = minijinja::Environment::new();
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);
    let sources = std::sync::Arc::new(templates);
    env.set_loader(move |name: &str| {
        Ok(sources.get(name).cloned())
    });

    let tpl = env.get_template("orchestrator")?;
    let rendered = tpl.render(&ctx)
        .with_context(|| format!("Failed to render orchestrator template: {}", orchestrator_path))?;

    // Write the master file
    let master_name = format!("book.{}", output_ext);
    let master_path = output.join(&master_name);
    fs::write(&master_path, &rendered)?;

    if !quiet {
        eprintln!("  Master: {}", master_path.display());
    }

    // Run compile command if configured
    let compile_target = config.targets.get(target_name)
        .and_then(|t| t.compile.as_ref());

    if let Some(compile) = compile_target {
        let compile_ext = compile.extension.as_deref().unwrap_or("pdf");
        let output_filename = format!("book.{}", compile_ext);

        if compile.command.is_none() && master_name.ends_with(".typ") {
            // Native Typst compilation
            let input_path = output.join(&master_name);
            let output_file = output.join(&output_filename);
            if !quiet {
                eprintln!("  Compiling: {} → {}", input_path.display(), output_file.display());
            }
            crate::typst_compile::compile_typst_to_pdf(&input_path, &output_file)?;
            if !quiet {
                eprintln!("  Output: {}", output_file.display());
            }
        } else if let Some(ref cmd) = compile.command {
            let expanded = cmd
                .replace("{input}", &master_name)
                .replace("{output}", &output_filename);

            if !quiet {
                eprintln!("  Compiling: {}", expanded);
            }

            // Run from the output directory (so \include paths resolve),
            // but add both the output dir and the project root to TEXINPUTS
            // so image paths (relative to either) resolve correctly.
            let texinputs = format!(
                "{}:{}:",
                output.display(),
                base_dir.display(),
            );
            let status = std::process::Command::new("sh")
                .arg("-c")
                .arg(&expanded)
                .current_dir(output)
                .env("TEXINPUTS", &texinputs)
                .status()
                .with_context(|| format!("Failed to run compile command: {}", expanded))?;

            if !status.success() {
                anyhow::bail!("Compile command failed: {}", expanded);
            }

            if !quiet {
                eprintln!("  Output: {}", output.join(&output_filename).display());
            }
        }
    }

    Ok(())
}

/// A node in the orchestrator page tree, serializable to MiniJinja.
#[derive(Debug, Clone, serde::Serialize)]
struct OrchestratorNode {
    /// Display title (section title or page title from frontmatter)
    title: String,
    /// Relative path to the fragment file (without extension for LaTeX \include)
    path: Option<String>,
    /// Path with extension
    file: Option<String>,
    /// Child nodes (documents within a section)
    children: Vec<OrchestratorNode>,
}

fn build_orchestrator_tree(
    nodes: &[DocumentNode],
    document_map: &HashMap<String, &DocumentInfo>,
    results: &HashMap<String, render::CollectionRenderResult>,
    format: &str,
) -> Vec<OrchestratorNode> {
    nodes.iter().map(|node| match node {
        DocumentNode::Document { path: source, title: override_title } => {
            let info = document_map.get(source.as_str());
            let title = override_title.clone()
                .or_else(|| results.get(source.as_str()).and_then(|r| r.title.clone()))
                .or_else(|| info.and_then(|p| p.meta.title.clone()))
                .unwrap_or_else(|| source.clone());
            let file = info.map(|p| p.output.display().to_string());
            // For LaTeX \include, strip the extension
            let path = file.as_ref().map(|f| {
                if format == "latex" {
                    f.strip_suffix(".tex").unwrap_or(f).to_string()
                } else {
                    f.clone()
                }
            });
            OrchestratorNode { title, path, file, children: vec![] }
        }
        DocumentNode::Section { title, documents, .. } => {
            OrchestratorNode {
                title: title.clone(),
                path: None,
                file: None,
                children: build_orchestrator_tree(documents, document_map, results, format),
            }
        }
    }).collect()
}

/// Format author from TOML value (string or array of strings).
fn format_author(val: &toml::Value) -> String {
    match val {
        toml::Value::String(s) => s.clone(),
        toml::Value::Array(arr) => arr.iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}


/// Serve a built site directory using the built-in HTTP server.
pub fn serve(output: &std::path::Path, port: u16) -> anyhow::Result<()> {
    use tiny_http::{Header, Response, StatusCode};

    let output = output.canonicalize()
        .with_context(|| format!("Site directory not found: {}", output.display()))?;

    let (server, actual_port) = crate::preview::server::try_bind(port)?;

    eprintln!("Serving at http://localhost:{}", actual_port);
    let _ = open::that(format!("http://localhost:{}", actual_port));

    let respond_404 = |request: tiny_http::Request, output: &std::path::Path| {
        let page_404 = output.join("404.html");
        if let Ok(body) = fs::read(&page_404) {
            let header = Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap();
            let _ = request.respond(Response::from_data(body).with_header(header).with_status_code(StatusCode(404)));
        } else {
            let _ = request.respond(Response::from_string("Not found").with_status_code(StatusCode(404)));
        }
    };

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        let rel = url.split('?').next().unwrap_or(&url).trim_start_matches('/');

        let mut file_path = output.join(rel);
        if file_path.is_dir() {
            file_path = file_path.join("index.html");
        }

        // Prevent path traversal
        if !file_path.starts_with(&output) {
            let _ = request.respond(Response::from_string("Forbidden").with_status_code(StatusCode(403)));
            continue;
        }

        if file_path.is_file() {
            match fs::read(&file_path) {
                Ok(data) => {
                    let mime = crate::preview::server::resolve_mime(&file_path);
                    let header = Header::from_bytes("Content-Type", mime).unwrap();
                    let _ = request.respond(Response::from_data(data).with_header(header));
                }
                Err(_) => respond_404(request, &output),
            }
        } else {
            respond_404(request, &output);
        }
    }

    Ok(())
}
