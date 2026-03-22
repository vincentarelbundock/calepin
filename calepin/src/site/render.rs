use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use crate::project::ProjectConfig;
use super::discover::PageInfo;

/// Result of rendering a single page for the site.
pub struct SiteRenderResult {
    pub body: String,
    pub toc: Option<String>,
    pub title: Option<String>,
    pub date: Option<String>,
    pub subtitle: Option<String>,
    pub abstract_text: Option<String>,
}

/// Render all pages by calling calepin's render_core() directly.
/// Returns a map from source path to rendered result.
///
/// When `apply_page_template` is true, each page's body is wrapped in the
/// project's page template (e.g., `templates/latex/page.tex`). This is
/// used for orchestrated builds where fragments need to be complete files.
/// When false (HTML sites), the body is returned raw for wrapping by site
/// Jinja templates.
pub fn render_pages(
    pages: &[PageInfo],
    config: &ProjectConfig,
    base_dir: &Path,
    output_dir: &Path,
    format: &str,
    apply_page_template: bool,
    target_name: Option<&str>,
    quiet: bool,
) -> Result<HashMap<String, SiteRenderResult>> {
    if pages.is_empty() {
        return Ok(HashMap::new());
    }

    // Build overrides from format config
    let overrides = build_overrides(config);

    if !quiet {
        eprintln!("Rendering {} pages...", pages.len());
    }

    let format_owned = format.to_string();
    let target_owned = target_name.map(|s| s.to_string());

    // Render all pages in parallel using thread::scope
    let results: Vec<(String, Result<SiteRenderResult>)> = std::thread::scope(|s| {
        let handles: Vec<_> = pages
            .iter()
            .map(|page| {
                let overrides = &overrides;
                let base_dir = base_dir;
                let output_dir = output_dir;
                let format = &format_owned;
                let target = &target_owned;
                s.spawn(move || {
                    // Set active target in this thread for template resolution
                    crate::paths::set_active_target(target.as_deref());
                    let key = page.source.display().to_string();
                    let result = render_one_page(page, overrides, base_dir, output_dir, format, apply_page_template);
                    (key, result)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let mut map = HashMap::new();
    for (key, result) in results {
        match result {
            Ok(render_result) => {
                map.insert(key, render_result);
            }
            Err(e) => {
                eprintln!("Error rendering {}: {:#}", key, e);
            }
        }
    }

    Ok(map)
}

fn render_one_page(
    page: &PageInfo,
    overrides: &[String],
    base_dir: &Path,
    output_dir: &Path,
    format: &str,
    apply_page_template: bool,
) -> Result<SiteRenderResult> {
    let input = base_dir.join(&page.source);
    let output_path = output_dir.join(&page.output);

    let result = crate::render_core(&input, &output_path, Some(format), overrides, None)?;

    let body = if apply_page_template {
        // Apply the project's page template (e.g., book's minimal page.tex)
        let renderer = crate::formats::create_renderer(format)?;
        renderer.apply_template(&result.rendered, &result.metadata, &result.element_renderer)
            .unwrap_or(result.rendered)
    } else if format == "html" {
        // HTML site mode: prepend syntax highlighting CSS
        let syntax_css = result.element_renderer.syntax_css_with_scope(
            crate::filters::highlighting::ColorScope::DataTheme,
        );
        if syntax_css.is_empty() {
            result.rendered
        } else {
            format!("<style>\n{}</style>\n{}", syntax_css, result.rendered)
        }
    } else {
        result.rendered
    };

    // Build TOC from rendered headings (HTML only)
    let toc = if format == "html" && result.metadata.toc.unwrap_or(true) {
        let depth = if result.metadata.toc_depth == 0 { 3 } else { result.metadata.toc_depth };
        let title = result.metadata.toc_title.as_deref().unwrap_or("Contents");
        let toc_html = crate::render::template::build_html_toc_from_body(&body, depth, title);
        if toc_html.is_empty() { None } else { Some(toc_html) }
    } else {
        None
    };

    Ok(SiteRenderResult {
        body,
        toc,
        title: result.metadata.title.map(|t| render_inline(&t, format)),
        date: result.metadata.date,
        subtitle: result.metadata.subtitle.map(|t| render_inline(&t, format)),
        abstract_text: result.metadata.abstract_text,
    })
}

/// Render inline markdown, stripping the <p> wrapper.
fn render_inline(text: &str, format: &str) -> String {
    crate::render::markdown::render_inline(text, format)
}

fn build_overrides(config: &ProjectConfig) -> Vec<String> {
    let mut overrides = Vec::new();

    // Highlight style from [meta]
    if let Some(ref meta) = config.meta {
        if let Some(ref hl) = meta.highlight {
            if let Some(ref light) = hl.light {
                overrides.push(format!("highlight-style.light={}", light));
            }
            if let Some(ref dark) = hl.dark {
                overrides.push(format!("highlight-style.dark={}", dark));
            }
        }
    }

    overrides
}
