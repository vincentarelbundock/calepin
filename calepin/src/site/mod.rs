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

use context::{build_page_context, build_site_context, mark_active, ListingItem};
use discover::{discover_listing_pages, discover_pages, discover_standalone_pages, PageInfo};
use crate::project::{PageNode, expand_contents};

/// Build a static site from .qmd files.
/// `cli_target` overrides `[site].target` when provided via `-t` on the command line.
pub fn build_site(
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

    // 2. Resolve site target (format and extension)
    //    CLI -t flag takes precedence over [site].target, which defaults to "html".
    let site_target_name = cli_target.map(|s| s.to_string())
        .or_else(|| config.target.clone())
        .unwrap_or_else(|| "html".to_string());
    let site_target = crate::project::resolve_target(&site_target_name, Some(&config))?;
    let format = &site_target.base;
    let output_ext = site_target.output_extension();

    // Set active target and project root for template/component resolution
    crate::paths::set_active_target(Some(&site_target_name));
    crate::paths::set_project_root(Some(&base_dir));

    // Auto-detect orchestrator: check templates/{target}/orchestrator.{ext}
    // Falls back to built-in templates if not found on filesystem.
    let ext = crate::paths::base_to_ext(format);
    let orchestrator_filename = format!("orchestrator.{}", ext);
    let orchestrator = config.orchestrator.clone()
        .or_else(|| {
            let p = base_dir.join("_calepin").join("templates").join(&site_target_name)
                .join(&orchestrator_filename);
            if p.exists() { return Some(p.display().to_string()); }
            // Check built-in templates
            let builtin_path = format!("templates/{}/{}", site_target_name, orchestrator_filename);
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
    let mut pages = discover_pages(&config, &base_dir, output_ext)?;
    let standalone = discover_standalone_pages(&config, &base_dir, output_ext)?;
    pages.extend(standalone);
    if !quiet {
        eprintln!("Found {} pages", pages.len());
    }

    // 5. Discover listing pages and merge into the page list
    let mut all_listing_pages: HashMap<String, Vec<PageInfo>> = HashMap::new();
    for page in &pages {
        if let Some(ref listing) = page.meta.listing {
            let listing_pages = discover_listing_pages(listing, &base_dir, &pages, output_ext)?;
            all_listing_pages.insert(page.source.display().to_string(), listing_pages);
        }
    }
    let existing_sources: Vec<String> = pages.iter().map(|p| p.source.display().to_string()).collect();
    for listing_pages in all_listing_pages.values() {
        for lp in listing_pages {
            if !existing_sources.contains(&lp.source.display().to_string()) {
                pages.push(lp.clone());
            }
        }
    }

    // 6. Render all pages with calepin (in parallel)
    //    When an orchestrator is set, apply the page template to each fragment
    //    (the project's templates/latex/page.tex or similar).
    //    When no orchestrator (HTML sites), return raw bodies for site Jinja wrapping.
    let apply_page_template = orchestrator.is_some();
    let results = render::render_pages(&pages, &config, &base_dir, output, format, apply_page_template, Some(&site_target_name), Some(&site_target), quiet)?;

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
            if !quiet {
                eprintln!("  {} -> {}", source_key, output_path.display());
            }
        }
    }

    // 8. Site-specific wrapping (HTML) or orchestrator assembly
    if let Some(ref orchestrator_path) = orchestrator {
        // Render the orchestrator template with page tree
        render_orchestrator(&config, &pages, &results, &base_dir, output, orchestrator_path, format, output_ext, &site_target_name, quiet)?;
    } else {
        // HTML site path: re-wrap pages through Jinja site templates
        apply_site_templates(&config, &pages, &results, &all_listing_pages, &base_dir, output, format, &site_target_name)?;
    }

    // 9. Copy assets/ to output
    assets::copy_assets(&base_dir, output)?;

    // 10. Run user-configured post-processing commands
    run_post_commands(&config, &site_target_name, &base_dir, output, quiet)?;

    if !quiet {
        eprintln!("Site built: {}", output.display());
    }

    Ok(())
}

/// Rebuild only the specified pages within an already-built site.
/// Discovers all pages (for nav context) but only re-renders those whose
/// source paths are in `changed_sources`. Skips the clean step and assets copy.
pub fn rebuild_pages(
    config_path: Option<&Path>,
    cli_target: Option<&str>,
    changed_sources: &[std::path::PathBuf],
) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let (config, found_path) = config::load_config(config_path, &cwd)?;
    let base_dir = found_path.parent().unwrap_or(&cwd).to_path_buf();

    let site_target_name = cli_target.map(|s| s.to_string())
        .or_else(|| config.target.clone())
        .unwrap_or_else(|| "html".to_string());
    let site_target = crate::project::resolve_target(&site_target_name, Some(&config))?;
    let format = &site_target.base;
    let output_ext = site_target.output_extension();

    crate::paths::set_active_target(Some(&site_target_name));
    crate::paths::set_project_root(Some(&base_dir));

    let output_dir = base_dir.join("output");

    // Discover all pages (needed for nav context), including standalone
    let mut pages = discover_pages(&config, &base_dir, output_ext)?;
    let standalone = discover_standalone_pages(&config, &base_dir, output_ext)?;
    pages.extend(standalone);

    // Determine which pages to re-render by matching changed absolute paths
    // against the discovered page sources. Canonicalize both sides so that
    // symlinks and path normalization differences don't cause mismatches.
    let canon_changed: Vec<std::path::PathBuf> = changed_sources.iter()
        .filter_map(|c| c.canonicalize().ok())
        .collect();
    let changed_pages: Vec<&PageInfo> = pages.iter().filter(|p| {
        let abs = base_dir.join(&p.source);
        let canon = abs.canonicalize().unwrap_or(abs);
        canon_changed.iter().any(|c| c == &canon)
    }).collect();

    if changed_pages.is_empty() {
        return Ok(());
    }

    // Render only the changed pages
    let pages_to_render: Vec<PageInfo> = changed_pages.iter().map(|p| (*p).clone()).collect();
    let results = render::render_pages(
        &pages_to_render, &config, &base_dir, &output_dir, format,
        false, Some(&site_target_name), Some(&site_target), true,
    )?;

    // Write raw body files
    for page in &pages_to_render {
        let source_key = page.source.display().to_string();
        if let Some(result) = results.get(&source_key) {
            let output_path = output_dir.join(&page.output);
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&output_path, &result.body)?;
        }
    }

    // Apply site templates to the changed pages (with full nav context)
    if format == "html" {
        let env = templates::init_jinja(&base_dir, &site_target_name)?
            .ok_or_else(|| anyhow::anyhow!("No template files found"))?;

        let site_ctx = build_site_context(&config, &pages, &base_dir);
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
        let mut all_listing_pages: HashMap<String, Vec<PageInfo>> = HashMap::new();
        for page in &pages_to_render {
            if let Some(ref listing) = page.meta.listing {
                let listing_pages = discover_listing_pages(listing, &base_dir, &pages, output_ext)?;
                all_listing_pages.insert(page.source.display().to_string(), listing_pages);
            }
        }

        for page in &pages_to_render {
            let source_key = page.source.display().to_string();
            let result = results.get(&source_key);

            let listing_items = all_listing_pages.get(&source_key).map(|listing_pages| {
                listing_pages.iter().map(|lp| ListingItem {
                    title: lp.meta.title.clone(),
                    date: lp.meta.date.clone(),
                    description: lp.meta.description.clone(),
                    image: lp.meta.image.clone(),
                    url: lp.url.clone(),
                }).collect()
            });

            let page_ctx = build_page_context(page, result, &pages, listing_items);

            let mut nav_tree = site_ctx.pages.clone();
            mark_active(&mut nav_tree, &page.url);

            let site_with_active = minijinja::context! {
                site => context::SiteContext {
                    title: site_ctx.title.clone(),
                    subtitle: site_ctx.subtitle.clone(),
                    url: site_ctx.url.clone(),
                    favicon: site_ctx.favicon.clone(),
                    logo: site_ctx.logo.clone(),
                    logo_dark: site_ctx.logo_dark.clone(),
                    pages: nav_tree,
                    dark_mode: site_ctx.dark_mode,
                    math_block: site_ctx.math_block.clone(),
                },
                page => page_ctx,
                var => var_ctx.clone(),
            };

            let template_name = if page.meta.listing.is_some() {
                format!("listing.{}", tpl_ext)
            } else {
                format!("page.{}", tpl_ext)
            };

            let tpl = env.get_template(&template_name)
                .with_context(|| format!("Failed to get template {} for {}", template_name, source_key))?;
            let rendered = tpl.render(&site_with_active)
                .with_context(|| format!("Failed to render template for {}", source_key))?;

            let output_path = output_dir.join(&page.output);
            fs::write(&output_path, &rendered)?;
        }

        // Update _source/ copies for the changed pages
        for page in &pages_to_render {
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

/// HTML site path: wrap each page's body through Jinja site templates
/// (page.html, listing.html with extends/includes).
/// Overwrites the raw body files written in step 7 with fully templated HTML.
fn apply_site_templates(
    config: &crate::project::ProjectConfig,
    pages: &[PageInfo],
    results: &HashMap<String, render::SiteRenderResult>,
    all_listing_pages: &HashMap<String, Vec<PageInfo>>,
    base_dir: &Path,
    output: &Path,
    format: &str,
    target_name: &str,
) -> Result<()> {
    // Initialize MiniJinja from templates/{target}/
    let env = templates::init_jinja(base_dir, target_name)?
        .ok_or_else(|| anyhow::anyhow!(
            "No template files found in templates/{}/. \
             At least base and page templates are required for multi-file site mode.",
            target_name
        ))?;

    // Build site context
    let site_ctx = build_site_context(config, pages, base_dir);

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

        let listing_items = all_listing_pages.get(&source_key).map(|listing_pages| {
            listing_pages.iter().map(|lp| ListingItem {
                title: lp.meta.title.clone(),
                date: lp.meta.date.clone(),
                description: lp.meta.description.clone(),
                image: lp.meta.image.clone(),
                url: lp.url.clone(),
            }).collect()
        });

        let page_ctx = build_page_context(page, result, pages, listing_items);

        // Mark active page in nav tree
        let mut nav_tree = site_ctx.pages.clone();
        mark_active(&mut nav_tree, &page.url);

        let site_with_active = minijinja::context! {
            site => context::SiteContext {
                title: site_ctx.title.clone(),
                subtitle: site_ctx.subtitle.clone(),
                url: site_ctx.url.clone(),
                favicon: site_ctx.favicon.clone(),
                logo: site_ctx.logo.clone(),
                logo_dark: site_ctx.logo_dark.clone(),
                pages: nav_tree,
                dark_mode: site_ctx.dark_mode,
                math_block: site_ctx.math_block.clone(),
            },
            page => page_ctx,
            var => var_ctx.clone(),
        };

        let template_name = if page.meta.listing.is_some() {
            format!("listing.{}", tpl_ext)
        } else {
            format!("page.{}", tpl_ext)
        };

        let tpl = env.get_template(&template_name)
            .with_context(|| format!("Failed to get template {} for {}", template_name, source_key))?;
        let rendered = tpl.render(&site_with_active)
            .with_context(|| format!("Failed to render template for {}", source_key))?;

        // Overwrite the raw body file with fully templated output
        let output_path = output.join(&page.output);
        fs::write(&output_path, &rendered)?;
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

/// Render the orchestrator template with the page tree.
/// Fragment files are already written; this produces the master file
/// that references them via \include{} or equivalent.
fn render_orchestrator(
    config: &crate::project::ProjectConfig,
    pages: &[PageInfo],
    results: &HashMap<String, render::SiteRenderResult>,
    base_dir: &Path,
    output: &Path,
    orchestrator_path: &str,
    format: &str,
    output_ext: &str,
    target_name: &str,
    quiet: bool,
) -> Result<()> {
    // Build the page tree with titles and paths
    let page_tree = expand_contents(&config.contents, base_dir);

    let page_map: HashMap<String, &PageInfo> = pages.iter()
        .map(|p| (p.source.display().to_string(), p))
        .collect();

    let nav_nodes = build_orchestrator_tree(&page_tree, &page_map, results, format);

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

    let ctx = minijinja::context! {
        meta => meta_ctx,
        var => var_ctx,
        pages => nav_nodes,
        format => format,
        base => format,
    };

    // Build a Jinja environment with all templates from the target and common dirs.
    // This lets the orchestrator use {% include "preamble.jinja" %} etc.
    let mut env = minijinja::Environment::new();
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);

    // Load templates from templates/{target}/ and templates/common/
    let dirs = [
        base_dir.join("_calepin").join("templates").join(target_name),
        base_dir.join("_calepin").join("templates").join("common"),
    ];
    for dir in &dirs {
        if !dir.is_dir() { continue; }
        // Load all files (any extension) so .jinja, .tex, .typ all work
        let pattern = dir.join("**").join("*.*");
        let pattern_str = pattern.display().to_string();
        for entry in glob::glob(&pattern_str).unwrap_or_else(|_| glob::glob("").unwrap()) {
            if let Ok(path) = entry {
                if !path.is_file() { continue; }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let rel = path.strip_prefix(dir).unwrap_or(&path);
                    let name = rel.display().to_string();
                    let content: &'static str = Box::leak(content.into_boxed_str());
                    let name: &'static str = Box::leak(name.into_boxed_str());
                    // Don't overwrite target-specific with common (target loaded first)
                    if env.get_template(name).is_err() {
                        let _ = env.add_template(name, content);
                    }
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
                if !name.is_empty() && env.get_template(name).is_err() {
                    let content: &'static str = Box::leak(content.to_string().into_boxed_str());
                    let name: &'static str = Box::leak(name.to_string().into_boxed_str());
                    let _ = env.add_template(name, content);
                }
            }
        }
    }

    // Load the orchestrator template itself
    let tpl_source = if let Some(builtin_path) = orchestrator_path.strip_prefix("__builtin__:") {
        // Load from embedded built-in templates
        crate::render::elements::BUILTIN_PROJECT.get_file(builtin_path)
            .and_then(|f| f.contents_utf8())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("Built-in orchestrator template not found: {}", builtin_path))?
    } else {
        let tpl_path = base_dir.join(orchestrator_path);
        fs::read_to_string(&tpl_path)
            .with_context(|| format!("Failed to read orchestrator template: {}", tpl_path.display()))?
    };
    env.add_template("orchestrator", &tpl_source)
        .with_context(|| format!("Failed to parse orchestrator template: {}", orchestrator_path))?;

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
    /// Child nodes (pages within a section)
    children: Vec<OrchestratorNode>,
}

fn build_orchestrator_tree(
    nodes: &[PageNode],
    page_map: &HashMap<String, &PageInfo>,
    results: &HashMap<String, render::SiteRenderResult>,
    format: &str,
) -> Vec<OrchestratorNode> {
    nodes.iter().map(|node| match node {
        PageNode::Page { path: source, title: override_title } => {
            let info = page_map.get(source.as_str());
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
        PageNode::Section { title, pages, .. } => {
            OrchestratorNode {
                title: title.clone(),
                path: None,
                file: None,
                children: build_orchestrator_tree(pages, page_map, results, format),
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

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        let rel = url.split('?').next().unwrap_or(&url).trim_start_matches('/');

        let mut file_path = output.join(rel);
        if file_path.is_dir() {
            file_path = file_path.join("index.html");
        }

        if file_path.is_file() {
            match fs::read(&file_path) {
                Ok(data) => {
                    let mime = crate::preview::server::resolve_mime(&file_path);
                    let header = Header::from_bytes("Content-Type", mime).unwrap();
                    let _ = request.respond(Response::from_data(data).with_header(header));
                }
                Err(_) => {
                    let page_404 = output.join("404.html");
                    if let Ok(body) = fs::read(&page_404) {
                        let header = Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap();
                        let _ = request.respond(Response::from_data(body).with_header(header).with_status_code(StatusCode(404)));
                    } else {
                        let _ = request.respond(Response::from_string("Not found").with_status_code(StatusCode(404)));
                    }
                }
            }
        } else {
            let page_404 = output.join("404.html");
            if let Ok(body) = fs::read(&page_404) {
                let header = Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap();
                let _ = request.respond(Response::from_data(body).with_header(header).with_status_code(StatusCode(404)));
            } else {
                let _ = request.respond(Response::from_string("Not found").with_status_code(StatusCode(404)));
            }
        }
    }

    Ok(())
}
