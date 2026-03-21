use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use super::config::SiteConfig;
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
    config: &SiteConfig,
    base_dir: &Path,
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
                s.spawn(move || {
                    let key = page.source.display().to_string();
                    let result = render_one_page(page, overrides, base_dir);
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
) -> Result<SiteRenderResult> {
    let input = base_dir.join(&page.source);
    let output_path = base_dir.join(&page.output);

    // Call calepin's render_core() directly — no subprocess, no JSON round-trip.
    // render_core stops before page template application, giving us the body.
    let result = crate::render_core(&input, &output_path, Some("html"), overrides)?;

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
        let toc_html = crate::render::template::build_html_toc(&body, depth, title);
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

/// Render inline markdown to HTML, stripping the <p> wrapper comrak adds.
fn render_inline_markdown(text: &str) -> String {
    let html = comrak::markdown_to_html(text, &comrak::Options::default());
    // comrak wraps in <p>...</p>\n — strip it for inline use
    let trimmed = html.trim();
    if trimmed.starts_with("<p>") && trimmed.ends_with("</p>") {
        trimmed[3..trimmed.len() - 4].to_string()
    } else {
        trimmed.to_string()
    }
}

fn build_overrides(config: &SiteConfig) -> Vec<String> {
    let mut overrides = Vec::new();
    if let Some(html) = &config.format.html {
        if let Some(toc) = html.toc {
            overrides.push(format!("toc={}", toc));
        }
        if let Some(ref hs) = html.highlight_style {
            match hs {
                super::config::HighlightStyle::Single(s) => {
                    overrides.push(format!("highlight-style={}", s));
                }
                super::config::HighlightStyle::DualTheme { light, dark } => {
                    overrides.push(format!("highlight-style.light={}", light));
                    overrides.push(format!("highlight-style.dark={}", dark));
                }
            }
        }
        if let Some(cc) = html.code_copy {
            overrides.push(format!("code-copy={}", cc));
        }
        if let Some(ref co) = html.code_overflow {
            overrides.push(format!("code-overflow={}", co));
        }
    }
    overrides
}
