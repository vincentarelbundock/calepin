// Collection templating: wrap rendered documents through Jinja site templates.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::context::{self, build_document_context, build_collection_context, build_nav_tree_for_lang, mark_active, ListingItem};
use super::discover::DocumentInfo;
use super::render;
use super::partials;

/// Extract the first <img> src from rendered HTML.
fn extract_first_image(html: &str) -> Option<String> {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r#"<img[^>]+src="([^"]+)""#).unwrap()
    });
    RE.captures(html).map(|c| c[1].to_string())
}

/// Build a ListingItem from a page info and optional rendered result.
pub(super) fn build_listing_item(
    lp: &DocumentInfo,
    results: &HashMap<String, render::CollectionRenderResult>,
) -> ListingItem {
    let image = lp.meta.image.clone().or_else(|| {
        let key = lp.source.display().to_string();
        results.get(&key).and_then(|r| extract_first_image(&r.body))
    });
    ListingItem {
        title: lp.meta.title.as_ref().map(|t| crate::render::convert::render_inline(t, "html")),
        date: lp.meta.date.as_ref().map(|d| crate::utils::date::format_date_display(d, None)),
        description: lp.meta.description.clone(),
        image,
        url: lp.url.clone(),
    }
}

/// HTML collection path: wrap each document's body through Jinja site templates
/// (page.html, listing.html with extends/includes).
/// Overwrites the raw body files written in step 7 with fully templated HTML.
pub(super) fn apply_collection_partials(
    meta: &crate::config::Metadata,
    pages: &[DocumentInfo],
    results: &HashMap<String, render::CollectionRenderResult>,
    all_listing_documents: &HashMap<String, Vec<DocumentInfo>>,
    base_dir: &Path,
    output: &Path,
    format: &str,
    target_name: &str,
) -> Result<()> {
    // Initialize MiniJinja from templates/{target}/
    let env = partials::load_templates(base_dir, target_name)?
        .ok_or_else(|| anyhow::anyhow!(
            "No template files found in templates/{}/. \
             At least base and page templates are required for multi-file collection mode.",
            target_name
        ))?;

    // Build collection context
    let collection_ctx = build_collection_context(meta, pages, base_dir);

    // Convert var to minijinja Value for template access
    let var_ctx = crate::config::build_jinja_vars(&meta.var);

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
        // Skip pages not in results -- their output is already correct on disk
        // (from a previous build). Re-wrapping them would double-wrap the HTML.
        let result = match results.get(&source_key) {
            Some(r) => r,
            None => continue,
        };

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
        let mut nav_tree = if !meta.languages.is_empty() {
            if let Some(ref lang) = page.lang {
                build_nav_tree_for_lang(meta, pages, base_dir, lang)
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
            let mut doc_ctx = build_document_context(page, Some(result), pages, listing, &meta.languages, meta, base_dir);
            doc_ctx.pagination = pagination.clone();

            let collection_with_active = minijinja::context! {
                collection => context::CollectionContext {
                    title: collection_ctx.title.clone(),
                    subtitle: collection_ctx.subtitle.clone(),
                    url: collection_ctx.url.clone(),
                    favicon: collection_ctx.favicon.clone(),
                    navbar: collection_ctx.navbar.clone(),
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
        // Copy .qmd source files to _calepin_source/ for the source viewer
        for page in pages {
            let source_dest = output.join("_calepin_source").join(&page.source);
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
