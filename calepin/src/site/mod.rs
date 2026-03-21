mod assets;
mod config;
mod context;
mod discover;
mod icons;
mod render;
mod search;
mod templates;

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use config::SiteConfig;
use context::{build_page_context, build_site_context, mark_active, ListingItem};
use discover::{discover_listing_pages, discover_pages, PageInfo};

/// Build a static site from .qmd files.
pub fn build_site(
    config_path: Option<&Path>,
    output: &Path,
    clean: bool,
    quiet: bool,
) -> Result<()> {
    let base_dir = std::env::current_dir()?;

    // 1. Load config
    let (config, found_path) = SiteConfig::load(config_path, &base_dir)?;
    if !quiet {
        eprintln!("Config: {}", found_path.display());
    }

    // 2. Prepare output directory
    if clean && output.exists() {
        fs::remove_dir_all(output)
            .with_context(|| format!("Failed to clean output dir: {}", output.display()))?;
    }
    fs::create_dir_all(output)?;

    // 3. Discover pages
    let mut pages = discover_pages(&config, &base_dir)?;
    if !quiet {
        eprintln!("Found {} pages", pages.len());
    }

    // 4. Discover listing pages and merge into the page list
    let mut all_listing_pages: HashMap<String, Vec<PageInfo>> = HashMap::new();
    for page in &pages {
        if let Some(ref listing) = page.meta.listing {
            let listing_pages = discover_listing_pages(listing, &base_dir, &pages)?;
            all_listing_pages.insert(page.source.display().to_string(), listing_pages);
        }
    }

    // Add listing-discovered pages that aren't already in the main list
    let existing_sources: Vec<String> = pages.iter().map(|p| p.source.display().to_string()).collect();
    for listing_pages in all_listing_pages.values() {
        for lp in listing_pages {
            if !existing_sources.contains(&lp.source.display().to_string()) {
                pages.push(lp.clone());
            }
        }
    }

    // 5. Render all pages with calepin (in parallel)
    let results = render::render_pages(&pages, &config, &base_dir, quiet)?;

    // 6. Initialize MiniJinja
    let env = templates::init_jinja(&base_dir)?;

    // 7. Build site context
    let site_ctx = build_site_context(&config, &pages);

    // 8. Render each page through MiniJinja
    for page in &pages {
        let source_key = page.source.display().to_string();
        let result = results.get(&source_key);

        // Build listing items if this page has a listing
        let listing_items = all_listing_pages.get(&source_key).map(|listing_pages| {
            listing_pages
                .iter()
                .map(|lp| ListingItem {
                    title: lp.meta.title.clone(),
                    date: lp.meta.date.clone(),
                    description: lp.meta.description.clone(),
                    image: lp.meta.image.clone(),
                    url: lp.url.clone(),
                })
                .collect()
        });

        let page_ctx = build_page_context(page, result, &pages, listing_items);

        // Mark active page in nav tree
        let mut nav_tree = site_ctx.pages.clone();
        mark_active(&mut nav_tree, &page.url);

        // Build MiniJinja context with active-marked nav tree
        let site_with_active = minijinja::context! {
            site => context::SiteContext {
                title: site_ctx.title.clone(),
                subtitle: site_ctx.subtitle.clone(),
                url: site_ctx.url.clone(),
                favicon: site_ctx.favicon.clone(),
                navbar: context::NavbarContext {
                    logo: site_ctx.navbar.logo.clone(),
                    logo_dark: site_ctx.navbar.logo_dark.clone(),
                    logo_alt: site_ctx.navbar.logo_alt.clone(),
                    background: site_ctx.navbar.background.clone(),
                    left: site_ctx.navbar.left.clone(),
                    right: site_ctx.navbar.right.clone(),
                    search: site_ctx.navbar.search,
                },
                sidebar: context::SidebarContext {
                    collapse_level: site_ctx.sidebar.collapse_level,
                },
                pages: nav_tree,
                dark_mode: site_ctx.dark_mode,
                math_block: site_ctx.math_block.clone(),
            },
            page => page_ctx,
        };

        // Choose template
        let template_name = if page.meta.listing.is_some() {
            "listing.html"
        } else {
            "page.html"
        };

        let tpl = env.get_template(template_name)
            .with_context(|| format!("Failed to get template {} for {}", template_name, source_key))?;
        let rendered = tpl.render(&site_with_active)
            .with_context(|| format!("Failed to render template for {}", source_key))?;

        // Write output
        let output_path = output.join(&page.output);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&output_path, &rendered)?;

        if !quiet {
            eprintln!("  {} → {}", source_key, output_path.display());
        }
    }

    // 9. Generate search index
    if config.website.navbar.search {
        search::generate_search_index(&pages, &results, output)?;
        if !quiet {
            eprintln!("  Generated search-index.json");
        }
    }

    // 10. Copy .qmd source files to _source/ for the source viewer
    for page in &pages {
        let source_dest = output.join("_source").join(&page.source);
        if let Some(parent) = source_dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let source_src = base_dir.join(&page.source);
        if source_src.exists() {
            fs::copy(&source_src, &source_dest)?;
        }
    }

    // 11. Write built-in assets
    assets::write_builtin_assets(output)?;

    // 11. Copy resource directories
    assets::copy_resources(&config.project.resources, &base_dir, output)?;

    if !quiet {
        eprintln!("Site built: {}", output.display());
    }

    Ok(())
}

/// Serve a built site directory using the built-in HTTP server.
pub fn serve(output: &std::path::Path, port: u16) -> anyhow::Result<()> {
    use tiny_http::{Header, Response, Server, StatusCode};

    let output = output.canonicalize()
        .with_context(|| format!("Site directory not found: {}", output.display()))?;

    // Try requested port, then fall back to nearby ports
    let (server, actual_port) = {
        let mut result = None;
        for p in port..=port + 10 {
            if let Ok(s) = Server::http(format!("0.0.0.0:{}", p)) {
                if p != port {
                    eprintln!("\x1b[33mWarning:\x1b[0m port {} in use, using {} instead", port, p);
                }
                result = Some((s, p));
                break;
            }
        }
        result.ok_or_else(|| anyhow::anyhow!("Could not find an available port in range {}–{}", port, port + 10))?
    };

    eprintln!("Serving at http://localhost:{}", actual_port);
    let _ = open::that(format!("http://localhost:{}", actual_port));

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        let rel = url.split('?').next().unwrap_or(&url).trim_start_matches('/');

        // Try the path directly, then as index.html
        let mut file_path = output.join(rel);
        if file_path.is_dir() {
            file_path = file_path.join("index.html");
        }

        if file_path.is_file() {
            match fs::read(&file_path) {
                Ok(data) => {
                    let mime = match file_path.extension().and_then(|e| e.to_str()) {
                        Some("html") => "text/html; charset=utf-8",
                        Some("css") => "text/css",
                        Some("js") => "application/javascript",
                        Some("json") => "application/json",
                        Some("svg") => "image/svg+xml",
                        Some("png") => "image/png",
                        Some("jpg" | "jpeg") => "image/jpeg",
                        Some("gif") => "image/gif",
                        Some("pdf") => "application/pdf",
                        Some("woff2") => "font/woff2",
                        Some("woff") => "font/woff",
                        Some("qmd") => "text/plain; charset=utf-8",
                        _ => "application/octet-stream",
                    };
                    let header = Header::from_bytes("Content-Type", mime).unwrap();
                    let _ = request.respond(Response::from_data(data).with_header(header));
                }
                Err(_) => {
                    let _ = request.respond(Response::from_string("Not found").with_status_code(StatusCode(404)));
                }
            }
        } else {
            let _ = request.respond(Response::from_string("Not found").with_status_code(StatusCode(404)));
        }
    }

    Ok(())
}
