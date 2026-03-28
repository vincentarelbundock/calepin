mod assets;
pub(crate) mod contents;
mod context;
pub(crate) mod discover;
mod orchestrator;
mod render;
mod partials;
mod templating;

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use discover::{discover_listing_documents, discover_documents, DocumentInfo};

use crate::paths::resolve_project_root;

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

    // 1. Load config and convert to metadata
    let (meta, found_path) = discover::load_config(config_path, &cwd)?;

    let base_dir = resolve_project_root(&found_path, &cwd);
    if !quiet {
        eprintln!("  \x1b[36mconfig:\x1b[0m {}", found_path.display());
    }

    // 2. Resolve collection target (format and extension)
    //    CLI -f flag takes precedence over [site].target, which defaults to "html".
    let collection_target_name = cli_target.map(|s| s.to_string())
        .or_else(|| meta.target.clone())
        .unwrap_or_else(|| "html".to_string());
    let collection_target = crate::config::resolve_target(&collection_target_name, &meta.targets)?;
    let format = &collection_target.writer;
    let output_ext = collection_target.output_extension();

    // Set active target and project root for template/component resolution
    crate::paths::set_active_target(Some(&collection_target_name));
    crate::paths::set_project_root(Some(&base_dir));

    // Auto-detect orchestrator: check templates/{target}/orchestrator.{ext}
    // Falls back to built-in templates if not found on filesystem.
    let ext = crate::paths::resolve_extension(format);
    let orchestrator_filename = format!("orchestrator.{}", ext);
    let orchestrator = meta.orchestrator.clone()
        .or_else(|| {
            let p = crate::paths::partials_dir(&base_dir).join(&collection_target_name)
                .join(&orchestrator_filename);
            if p.exists() { return Some(p.display().to_string()); }
            // Check built-in templates
            let builtin_path = format!("{}/{}", collection_target_name, orchestrator_filename);
            if crate::render::elements::BUILTIN_PARTIALS.get_file(&builtin_path).is_some() {
                Some(format!("__builtin__:{}", builtin_path))
            } else {
                None
            }
        });

    // 3. Prepare output directory (relative to CWD, not project root)
    let output = if output.is_relative() {
        &cwd.join(output)
    } else {
        output
    };
    if clean && output.exists() {
        fs::remove_dir_all(output)
            .with_context(|| format!("Failed to clean output dir: {}", output.display()))?;
    }
    fs::create_dir_all(output)?;

    // 4. Discover all .qmd pages (auto-discovered, filtered by exclude)
    let mut pages = discover_documents(&meta, &base_dir, output_ext)?;

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

    // 6. Page-level cache: skip rendering for pages whose source + config
    //    haven't changed since the last build. Read config content once for hashing.
    //    Disabled for crossref builds (they need ref_data from every page) and
    //    when clean=true (the output directory was just wiped).
    let apply_page_template = orchestrator.is_some();
    let use_crossref = format == "html" && !apply_page_template && meta.global_crossref;
    let use_page_cache = !clean && !use_crossref;

    let mut config_bytes = fs::read(&found_path).unwrap_or_default();
    config_bytes.extend_from_slice(&crate::utils::cache::collect_auxiliary_bytes(&base_dir));
    let old_cache = if use_page_cache { crate::utils::cache::load(output) } else { HashMap::new() };
    let mut new_cache: HashMap<String, u64> = HashMap::new();

    // Build overrides once (same logic render.rs uses) for the hash
    let cache_overrides = render::build_overrides(&meta, Some(&collection_target));

    // Partition pages into stale (need rendering) and fresh (output already on disk)
    let (stale_pages, fresh_keys): (Vec<&DocumentInfo>, Vec<String>) = if use_page_cache {
        let mut stale = Vec::new();
        let mut fresh = Vec::new();
        for page in &pages {
            let key = page.source.display().to_string();
            let source_path = base_dir.join(&page.source);
            let source_bytes = fs::read(&source_path).unwrap_or_default();
            let hash = crate::utils::cache::page_hash(&source_bytes, &config_bytes, &collection_target_name, &cache_overrides);
            new_cache.insert(key.clone(), hash);

            let output_exists = output.join(&page.output).exists();
            if output_exists && old_cache.get(&key) == Some(&hash) {
                fresh.push(key);
            } else {
                stale.push(page);
            }
        }
        (stale, fresh)
    } else {
        (pages.iter().collect(), Vec::new())
    };

    let skipped = fresh_keys.len();

    // 7. Render pages
    let results = if use_crossref {
        render::render_documents_with_crossref(&pages, &meta, &base_dir, output, Some(&collection_target_name), Some(&collection_target), quiet)?
    } else {
        // Render only stale pages
        let stale_owned: Vec<DocumentInfo> = stale_pages.into_iter().cloned().collect();
        render::render_documents(&stale_owned, &meta, &base_dir, output, format, apply_page_template, Some(&collection_target_name), Some(&collection_target), quiet)?
    };

    if !quiet && skipped > 0 {
        eprintln!("  \x1b[36mcache:\x1b[0m skipped {} unchanged page(s)", skipped);
    }

    // 8. Write page output files
    //    For orchestrated builds, strip the output directory prefix from paths
    //    in the rendered bodies so they resolve correctly when the compile
    //    command runs from the output directory.
    let output_prefix = format!("{}/", output.display());
    for page in &pages {
        let source_key = page.source.display().to_string();
        // Don't overwrite fresh pages -- their output is already correct on disk
        if fresh_keys.contains(&source_key) {
            continue;
        }
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

    // 9. Site-specific wrapping (HTML) or orchestrator assembly
    if let Some(ref orchestrator_path) = orchestrator {
        orchestrator::render_orchestrator(&meta, &pages, &results, &base_dir, output, orchestrator_path, format, output_ext, &collection_target_name, quiet)?;
    } else {
        templating::apply_collection_partials(&meta, &pages, &results, &all_listing_documents, &base_dir, output, format, &collection_target_name)?;
    }

    // 10. Copy assets/ and static directories to output
    assets::copy_assets(&base_dir, output, &meta.static_dirs)?;

    // 11. Run user-configured post-processing commands
    run_post_commands(&meta, &collection_target_name, &base_dir, output, quiet)?;

    // 12. Save page cache (after successful build only)
    if use_page_cache {
        crate::utils::cache::save(output, &new_cache);
    }

    if !quiet {
        eprintln!("  \x1b[36mbuilt:\x1b[0m {}", output.display());
    }

    Ok(())
}

/// Run user-configured post-processing commands from `[[post]]` in config.toml.
///
/// Each command is executed from the project root. `{output}` and `{root}` in the
/// command string are replaced with the output directory and project root paths.
/// Commands with a `targets` restriction are skipped if the active target doesn't match.
fn run_post_commands(
    meta: &crate::config::Metadata,
    target: &str,
    project_root: &Path,
    output: &Path,
    quiet: bool,
) -> Result<()> {
    for post in &meta.post {
        if !post.targets.is_empty() && !post.targets.iter().any(|t| t == target) {
            continue;
        }

        let relative_output = output.strip_prefix(project_root)
            .unwrap_or(output);
        let cmd = post.command
            .replace("{output}", &relative_output.display().to_string())
            .replace("{root}", &project_root.display().to_string());

        if !quiet {
            eprintln!("  \x1b[36mpost:\x1b[0m {}", cmd);
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
