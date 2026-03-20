use regex::Regex;
use std::sync::LazyLock;

use crate::render::elements::ElementRenderer;
use crate::render::template::{self, build_html_vars};
use crate::formats::OutputRenderer;
use crate::types::Metadata;
use crate::util::slugify;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;

pub struct HtmlRenderer;

impl OutputRenderer for HtmlRenderer {
    fn format(&self) -> &str { "html" }
    fn extension(&self) -> &str { "html" }

    fn postprocess(&self, body: &str, _renderer: &ElementRenderer) -> String {
        postprocess_html(body)
    }

    fn apply_template(
        &self,
        body: &str,
        meta: &Metadata,
        renderer: &ElementRenderer,
    ) -> Option<String> {
        let mut vars = build_html_vars(meta, body);
        vars.insert("preamble".to_string(), renderer.get_template("preamble"));
        let syntax_css = renderer.syntax_css_with_scope(
            crate::filters::highlighting::ColorScope::Both,
        );
        let css = vars.entry("css".to_string()).or_default();
        css.push_str(&format!("\n<style>\n{}</style>", syntax_css));
        let tpl = template::html_template();
        let html = template::apply_template(&tpl, &vars);
        Some(embed_images_base64(&html))
    }
}

// ---------------------------------------------------------------------------
// Image embedding
// ---------------------------------------------------------------------------

static IMG_SRC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<img\s([^>]*?)src="([^"]+)"([^>]*)>"#).unwrap());

/// Replace all `<img src="path">` with base64 data URIs.
/// Skips images that are already data URIs or remote URLs.
fn embed_images_base64(html: &str) -> String {
    IMG_SRC_RE.replace_all(html, |caps: &regex::Captures| {
        let before = &caps[1];
        let src = &caps[2];
        let after = &caps[3];

        // Skip data URIs and remote URLs
        if src.starts_with("data:") || src.starts_with("http://") || src.starts_with("https://") {
            return caps[0].to_string();
        }

        let path = std::path::Path::new(src);
        match std::fs::read(path) {
            Ok(data) => {
                let mime = match path.extension().and_then(|e| e.to_str()) {
                    Some("png") => "image/png",
                    Some("jpg") | Some("jpeg") => "image/jpeg",
                    Some("svg") => "image/svg+xml",
                    Some("gif") => "image/gif",
                    Some("webp") => "image/webp",
                    _ => "application/octet-stream",
                };
                let encoded = BASE64.encode(&data);
                format!("<img {}src=\"data:{};base64,{}\"{}/>", before, mime, encoded, after)
            }
            Err(_) => caps[0].to_string(),
        }
    }).to_string()
}

// ---------------------------------------------------------------------------
// Precompiled regexes
// ---------------------------------------------------------------------------

/// Matches headings with or without attributes: <h1>, <h2 id="foo">, etc.
static HEADING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?s)<(h([1-6]))([^>]*)>(.*?)</h[1-6]>"#).unwrap());

static STRIP_TAGS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

/// Matches `{#id}` or `{#id .class}` attribute syntax at end of heading content.
static HEADING_ATTR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*\{#([-a-zA-Z0-9_]+)[^}]*\}\s*$").unwrap());

static FOOTNOTE_REF_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"<sup class="footnote-ref"><a href="([^"]*)" id="([^"]*)" data-footnote-ref>(.*?)</a></sup>"#
    ).unwrap()
});

static FOOTNOTE_SECTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)<section class="footnotes"[^>]*>.*?</section>"#).unwrap()
});

static FOOTNOTE_LI_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?s)<li[^>]*>.*?</li>"#).unwrap());

static FOOTNOTE_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"id="fn-([^"]+)""#).unwrap());

static BACKREF_ATTRS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"data-footnote-backref-idx="\d+" aria-label="Back to reference \d+""#).unwrap()
});

// ---------------------------------------------------------------------------
// HTML post-processing
//
// These functions assume comrak's tightly controlled HTML output. We use regex
// rather than an HTML parser (scraper, html5ever) because:
//   1. comrak produces predictable, well-formed HTML with known structure
//   2. avoiding a DOM dependency keeps the binary small and compile fast
//   3. the transformations are simple string replacements, not tree rewrites
// If calepin ever consumes external HTML, this should be reconsidered.
// ---------------------------------------------------------------------------

/// Post-process rendered HTML: fix headings, footnotes.
pub fn postprocess_html(html: &str) -> String {
    let html = add_heading_ids(html);
    let html = fix_footnote_refs(&html);
    consolidate_footnotes(&html)
}

/// Add hierarchical section numbers to HTML headings.
///
/// When headings have been shifted (e.g. `#` → `<h2>` because a title exists),
/// the numbering uses the minimum heading level as the base so that top-level
/// headings are numbered 1, 2, 3 rather than 0.1, 0.2, 0.3.
pub fn number_sections_html(html: &str) -> String {
    // Find the minimum heading level present in the document.
    let min_level = HEADING_RE.captures_iter(html)
        .filter_map(|caps| caps[2].parse::<usize>().ok())
        .min()
        .unwrap_or(1);

    let mut counters = [0usize; 6];

    HEADING_RE.replace_all(html, |caps: &regex::Captures| {
        let tag = &caps[1];
        let level: usize = caps[2].parse().unwrap_or(1);
        let attrs = &caps[3];
        let content = &caps[4];
        // Normalize so the shallowest heading maps to depth 0.
        let depth = level - min_level;

        counters[depth] += 1;
        for c in counters.iter_mut().skip(depth + 1) {
            *c = 0;
        }

        let number: String = counters[..=depth]
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(".");

        format!(
            r#"<{}{}><span class="section-number">{}</span> {}</{}>"#,
            tag, attrs, number, content, tag
        )
    }).to_string()
}

// ---------------------------------------------------------------------------
// Heading IDs
// ---------------------------------------------------------------------------

/// Add id to headings from their text content. Preserves existing attributes.
/// Parses `{#id}` attribute syntax from heading content (Quarto-compatible).
/// Skips headings that already have an id.
fn add_heading_ids(html: &str) -> String {
    HEADING_RE.replace_all(html, |caps: &regex::Captures| {
        let tag = &caps[1];
        let attrs = &caps[3];
        let content = &caps[4];

        // Don't overwrite existing id
        if attrs.contains(" id=") {
            return caps[0].to_string();
        }

        // Check for {#id} attribute syntax in content
        let plain = STRIP_TAGS_RE.replace_all(content, "");
        if let Some(attr_caps) = HEADING_ATTR_RE.captures(&plain) {
            let id = &attr_caps[1];
            let clean_content = HEADING_ATTR_RE.replace(content, "");
            format!(r#"<{}{} id="{}">{}</{}>"#, tag, attrs, id, clean_content, tag)
        } else {
            let id = slugify(&plain);
            format!(r#"<{}{} id="{}">{}</{}>"#, tag, attrs, id, content, tag)
        }
    }).to_string()
}

// ---------------------------------------------------------------------------
// Footnote processing
// ---------------------------------------------------------------------------

/// Move `id` from inner `<a>` to the `<sup>` for footnote references.
fn fix_footnote_refs(html: &str) -> String {
    FOOTNOTE_REF_RE.replace_all(html, |caps: &regex::Captures| {
        format!(
            "<sup class=\"footnote-ref\" id=\"{}\"><a href=\"{}\" data-footnote-ref>{}</a></sup>",
            &caps[2], &caps[1], &caps[3],
        )
    }).to_string()
}

/// A single footnote item extracted from a `<section class="footnotes">`.
struct FootnoteItem {
    id: String,
    li_html: String,
}

/// Extract all footnote sections, merge, renumber sequentially.
fn consolidate_footnotes(html: &str) -> String {
    let items = extract_footnote_items(html);
    if items.is_empty() {
        return html.to_string();
    }

    let mut result = remove_footnote_sections(html);
    result = renumber_footnote_refs(&result, &items);
    let items = renumber_backref_attrs(&items);
    let section = render_footnote_section(&items);

    format!("{}\n{}", result.trim_end(), section)
}

/// Extract all `<li>` items from footnote sections, with their IDs.
fn extract_footnote_items(html: &str) -> Vec<FootnoteItem> {
    let mut items = Vec::new();
    for section in FOOTNOTE_SECTION_RE.find_iter(html) {
        for li in FOOTNOTE_LI_RE.find_iter(section.as_str()) {
            let li_html = li.as_str().to_string();
            let id = FOOTNOTE_ID_RE.captures(&li_html)
                .map(|caps| caps[1].to_string())
                .unwrap_or_default();
            items.push(FootnoteItem { id, li_html });
        }
    }
    items
}

/// Remove all footnote sections from the HTML body.
fn remove_footnote_sections(html: &str) -> String {
    FOOTNOTE_SECTION_RE.replace_all(html, "").to_string()
}

/// Renumber superscript references in the body to match sequential order.
fn renumber_footnote_refs(html: &str, items: &[FootnoteItem]) -> String {
    let mut result = html.to_string();
    for (i, item) in items.iter().enumerate() {
        if item.id.is_empty() { continue; }
        let new_num = (i + 1).to_string();
        let escaped_id = regex::escape(&item.id);

        // Update display number in superscript
        let ref_pattern = format!(r#"id="fnref-{}" data-footnote-ref>\d+</a>"#, escaped_id);
        let ref_re = Regex::new(&ref_pattern).unwrap();
        let ref_replacement = format!(r#"id="fnref-{}" data-footnote-ref>{}</a>"#, item.id, new_num);
        result = ref_re.replace_all(&result, ref_replacement.as_str()).to_string();

        // Update backref in body
        let backref_pattern = format!(
            "href=\"#fnref-{}\" class=\"footnote-backref\" data-footnote-backref data-footnote-backref-idx=\"\\d+\" aria-label=\"Back to reference \\d+\"",
            escaped_id
        );
        let backref_re = Regex::new(&backref_pattern).unwrap();
        let backref_replacement = format!(
            "href=\"#fnref-{}\" class=\"footnote-backref\" data-footnote-backref data-footnote-backref-idx=\"{}\" aria-label=\"Back to reference {}\"",
            item.id, new_num, new_num
        );
        result = backref_re.replace_all(&result, backref_replacement.as_str()).to_string();
    }
    result
}

/// Renumber backref attributes inside the `<li>` items themselves.
fn renumber_backref_attrs(items: &[FootnoteItem]) -> Vec<FootnoteItem> {
    items.iter().enumerate().map(|(i, item)| {
        if item.id.is_empty() {
            return FootnoteItem { id: item.id.clone(), li_html: item.li_html.clone() };
        }
        let new_num = (i + 1).to_string();
        let replacement = format!(
            r#"data-footnote-backref-idx="{}" aria-label="Back to reference {}""#,
            new_num, new_num
        );
        let li_html = BACKREF_ATTRS_RE.replace_all(&item.li_html, replacement.as_str()).to_string();
        FootnoteItem { id: item.id.clone(), li_html }
    }).collect()
}

/// Render a consolidated `<section class="footnotes">`.
fn render_footnote_section(items: &[FootnoteItem]) -> String {
    let li_html: String = items.iter()
        .map(|item| item.li_html.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<section class=\"footnotes\" data-footnotes>\n<ol>\n{}\n</ol>\n</section>",
        li_html
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_ids_plain() {
        let html = "<h1>Hello World</h1>\n<h2>Sub Section</h2>";
        let result = add_heading_ids(html);
        assert!(result.contains(r#"<h1 id="hello-world">"#), "{}", result);
        assert!(result.contains(r#"<h2 id="sub-section">"#), "{}", result);
    }

    #[test]
    fn test_heading_ids_with_attributes() {
        let html = r#"<h2 class="title">Hello World</h2>"#;
        let result = add_heading_ids(html);
        assert!(result.contains(r#"id="hello-world""#), "{}", result);
        assert!(result.contains(r#"class="title""#), "{}", result);
    }

    #[test]
    fn test_heading_ids_preserves_existing_id() {
        let html = r#"<h2 id="custom-id">Hello</h2>"#;
        let result = add_heading_ids(html);
        assert!(result.contains(r#"id="custom-id""#), "{}", result);
        assert!(!result.contains(r#"id="hello""#), "{}", result);
    }

    #[test]
    fn test_heading_ids_with_inline_html() {
        let html = "<h2>Hello <em>World</em></h2>";
        let result = add_heading_ids(html);
        assert!(result.contains(r#"id="hello-world""#), "{}", result);
        assert!(result.contains("<em>World</em>"), "{}", result);
    }

    #[test]
    fn test_number_sections_plain() {
        let html = r#"<h1 id="a">First</h1><h2 id="b">Sub</h2><h1 id="c">Second</h1>"#;
        let result = number_sections_html(html);
        assert!(result.contains(r#"<span class="section-number">1</span> First"#), "{}", result);
        assert!(result.contains(r#"<span class="section-number">1.1</span> Sub"#), "{}", result);
        assert!(result.contains(r#"<span class="section-number">2</span> Second"#), "{}", result);
    }

    #[test]
    fn test_number_sections_no_attributes() {
        let html = "<h1>Hello</h1>";
        let result = number_sections_html(html);
        assert!(result.contains(r#"<span class="section-number">1</span> Hello"#), "{}", result);
    }

    #[test]
    fn test_number_sections_shifted() {
        // When headings are shifted (title present), h2 is the top level
        let html = r#"<h2 id="a">Intro</h2><h3 id="b">Sub</h3><h2 id="c">Methods</h2>"#;
        let result = number_sections_html(html);
        assert!(result.contains(r#"<span class="section-number">1</span> Intro"#), "{}", result);
        assert!(result.contains(r#"<span class="section-number">1.1</span> Sub"#), "{}", result);
        assert!(result.contains(r#"<span class="section-number">2</span> Methods"#), "{}", result);
    }

    #[test]
    fn test_no_footnotes_unchanged() {
        let html = "<p>Hello world</p>";
        let result = consolidate_footnotes(html);
        assert_eq!(result, html);
    }
}
