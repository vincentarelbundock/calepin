use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use super::discover::PageInfo;
use super::render::SiteRenderResult;

#[derive(Serialize)]
struct SearchEntry {
    title: String,
    url: String,
    text: String,
    headings: Vec<String>,
}

/// Generate a search index JSON file at `output_dir/search-index.json`.
pub fn generate_search_index(
    pages: &[PageInfo],
    results: &std::collections::HashMap<String, SiteRenderResult>,
    output_dir: &Path,
) -> Result<()> {
    let mut entries = Vec::new();

    for page in pages {
        let key = page.source.display().to_string();
        let body = results
            .get(&key)
            .map(|r| r.body.as_str())
            .unwrap_or("");

        let text = strip_html(body);
        let headings = extract_headings(body);

        entries.push(SearchEntry {
            title: page.meta.title.clone().unwrap_or_default(),
            url: page.url.clone(),
            text,
            headings,
        });
    }

    let json = serde_json::to_string(&entries)?;
    fs::write(output_dir.join("search-index.json"), json)?;

    Ok(())
}

/// Crude HTML tag stripping for search indexing.
fn strip_html(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;

    let chars: Vec<char> = html.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if !in_tag && chars[i] == '<' {
            in_tag = true;
            // Check for script/style start
            let remaining: String = chars[i..].iter().take(10).collect();
            let lower = remaining.to_lowercase();
            if lower.starts_with("<script") {
                in_script = true;
            } else if lower.starts_with("<style") {
                in_style = true;
            } else if lower.starts_with("</script") {
                in_script = false;
            } else if lower.starts_with("</style") {
                in_style = false;
            }
        } else if in_tag && chars[i] == '>' {
            in_tag = false;
        } else if !in_tag && !in_script && !in_style {
            result.push(chars[i]);
        }
        i += 1;
    }

    // Collapse whitespace
    let mut collapsed = String::with_capacity(result.len());
    let mut prev_space = false;
    for ch in result.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                collapsed.push(' ');
                prev_space = true;
            }
        } else {
            collapsed.push(ch);
            prev_space = false;
        }
    }

    collapsed.trim().to_string()
}

/// Extract heading text from HTML.
fn extract_headings(html: &str) -> Vec<String> {
    let mut headings = Vec::new();
    let mut pos = 0;
    let bytes = html.as_bytes();

    while pos < bytes.len() {
        // Look for <h1...<h6
        if bytes[pos] == b'<'
            && pos + 3 < bytes.len()
            && bytes[pos + 1] == b'h'
            && bytes[pos + 2].is_ascii_digit()
        {
            // Find the end of the opening tag
            if let Some(tag_end) = html[pos..].find('>') {
                let content_start = pos + tag_end + 1;
                // Find closing tag
                let close_tag = format!("</h{}", bytes[pos + 2] as char);
                if let Some(close_pos) = html[content_start..].find(&close_tag) {
                    let heading_html = &html[content_start..content_start + close_pos];
                    let heading_text = strip_html(heading_html);
                    if !heading_text.is_empty() {
                        headings.push(heading_text);
                    }
                    pos = content_start + close_pos;
                } else {
                    pos += 1;
                }
            } else {
                pos += 1;
            }
        } else {
            pos += 1;
        }
    }

    headings
}
