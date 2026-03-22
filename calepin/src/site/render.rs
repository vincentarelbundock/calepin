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
pub fn render_pages(
    pages: &[PageInfo],
    config: &ProjectConfig,
    base_dir: &Path,
    output_dir: &Path,
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

    // Render all pages in parallel using thread::scope
    let results: Vec<(String, Result<SiteRenderResult>)> = std::thread::scope(|s| {
        let handles: Vec<_> = pages
            .iter()
            .map(|page| {
                let overrides = &overrides;
                let base_dir = base_dir;
                let output_dir = output_dir;
                s.spawn(move || {
                    let key = page.source.display().to_string();
                    let result = render_one_page(page, overrides, base_dir, output_dir);
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
) -> Result<SiteRenderResult> {
    let input = base_dir.join(&page.source);
    let output_path = output_dir.join(&page.output);

    let result = crate::render_core(&input, &output_path, Some("html"), overrides, None)?;

    // Prepend syntax highlighting CSS — normally injected by apply_template(),
    // which site mode skips since it has its own page shell.
    // Site uses data-theme attribute for theme switching (not media queries)
    let syntax_css = result.element_renderer.syntax_css_with_scope(
        crate::filters::highlighting::ColorScope::DataTheme,
    );
    let body = if syntax_css.is_empty() {
        result.rendered
    } else {
        format!("<style>\n{}</style>\n{}", syntax_css, result.rendered)
    };

    // Build TOC from rendered headings if toc is enabled
    let toc = if result.metadata.toc.unwrap_or(true) {
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
        title: result.metadata.title.map(|t| render_inline_markdown(&t)),
        date: result.metadata.date,
        subtitle: result.metadata.subtitle.map(|t| render_inline_markdown(&t)),
        abstract_text: result.metadata.abstract_text,
    })
}

/// Render inline markdown to HTML, stripping the <p> wrapper.
fn render_inline_markdown(text: &str) -> String {
    crate::render::markdown::render_inline(text, "html")
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
