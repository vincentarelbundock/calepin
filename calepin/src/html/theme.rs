use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::{anyhow, Result};
use serde::Serialize;

use super::syntax::HtmlSyntaxTheme;
use crate::utils::html::escape as html_escape;
use crate::utils::template::no_autoescape_env;

#[derive(Serialize)]
struct CssEntry {
    name: String,
    content: String,
}

#[derive(Serialize)]
struct JsEntry {
    name: String,
    content: String,
}

#[derive(Serialize)]
struct DocContext {
    head: String,
    body_open: String,
    body: String,
    body_close: String,
    title: String,
}

#[derive(Serialize)]
struct ThemeContext {
    doc: DocContext,
    site: SiteContext,
    store: serde_json::Value,
    css: Vec<CssEntry>,
    js: Vec<JsEntry>,
    highlight_css: String,
    theme: String,
    target: String,
}

#[derive(Serialize, Debug, Clone, Default)]
pub(crate) struct SiteContextInput {
    pub(crate) sidebar: Vec<SiteNavEntry>,
    pub(crate) sidebar_sections: Vec<SiteNavSection>,
    pub(crate) sidebar_fold: bool,
    pub(crate) menus: BTreeMap<String, Vec<SiteNavEntry>>,
    pub(crate) languages: Vec<SiteLanguageEntry>,
    pub(crate) translations: Vec<SiteLanguageEntry>,
    pub(crate) language: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) base_url: Option<String>,
    pub(crate) logo: Option<String>,
    pub(crate) logo_alt: Option<String>,
    pub(crate) home_url: Option<String>,
    pub(crate) favicon: Option<String>,
    pub(crate) page_url: Option<String>,
    pub(crate) page_title: Option<String>,
    pub(crate) pagefind: Option<SitePagefindEntry>,
    #[serde(skip)]
    pub(crate) store: serde_json::Value,
    /// Resolved max heading level included in the on-page TOC: `Some(0)` means
    /// no TOC, `Some(n)` includes headings up to level `n`. `None` (the
    /// default, used when no caller resolves it) falls back to the original
    /// always-on depth-3 behavior. Not serialized: only `annotate_body_headings`
    /// reads it, the computed `site.toc` entries are what templates see.
    #[serde(skip)]
    pub(crate) toc_depth: Option<usize>,
    #[serde(skip)]
    pub(crate) toc_floating: Option<bool>,
}

/// The site context handed to the template: the caller-supplied input (or a
/// filesystem-derived fallback) plus the per-page `toc` computed during render.
#[derive(Serialize)]
struct SiteContext {
    #[serde(flatten)]
    input: SiteContextInput,
    toc: Vec<TocEntry>,
    toc_floating: bool,
}

#[derive(Serialize, Debug, Clone)]
pub(crate) struct SitePagefindEntry {
    pub(crate) css: String,
    pub(crate) js: String,
    pub(crate) bundle: String,
}

#[derive(Serialize, Debug, Clone)]
pub(crate) struct SiteNavSection {
    pub(crate) title: Option<String>,
    pub(crate) active: bool,
    pub(crate) items: Vec<SiteNavEntry>,
}

#[derive(Serialize, Debug, Clone)]
pub(crate) struct SiteLanguageEntry {
    pub(crate) code: String,
    pub(crate) label: String,
    pub(crate) href: String,
    pub(crate) active: bool,
}

#[derive(Serialize, Debug, Clone)]
pub(crate) struct SiteNavEntry {
    pub(crate) href: String,
    pub(crate) label: String,
    pub(crate) label_html: String,
    pub(crate) active: bool,
}

#[derive(Serialize)]
struct TocEntry {
    level: usize,
    href: String,
    label: String,
}

struct HtmlDocumentParts<'a> {
    head: &'a str,
    body_open: &'a str,
    body: &'a str,
    body_close: &'a str,
    title: Option<String>,
}

pub(super) fn apply_html_theme(
    html: &str,
    entry: Option<&crate::theme::HtmlEntry>,
    syntax_theme: &HtmlSyntaxTheme,
    output_path: Option<&Path>,
    project_root: Option<&Path>,
    site_context: Option<&SiteContextInput>,
) -> Result<String> {
    let Some(entry) = entry else {
        return Ok(html.to_string());
    };
    // Rewrite Typst's inline `style="color: ..."` spans to syntax classes so
    // the theme's highlight CSS (placeholders or `highlight_css`) can drive colors.
    let rewritten = syntax_theme.rewrite_classes(html);
    // Typst emits a bare HTML fragment (no <html>/<head>/<body> wrapper) when
    // the document has no explicit html.elem("html") root element. Wrap those
    // fragments in a minimal well-formed document so split_html_document can
    // always locate </head>, <body>, and </body>.
    let normalized = if rewritten.contains("</head>") {
        rewritten
    } else {
        format!(
            "<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"></head><body>{}</body></html>",
            rewritten.trim()
        )
    };
    let parts = split_html_document(&normalized)?;
    render_theme(
        entry,
        &parts,
        syntax_theme,
        output_path,
        project_root,
        site_context,
    )
}

fn render_theme(
    entry: &crate::theme::HtmlEntry,
    parts: &HtmlDocumentParts<'_>,
    syntax_theme: &HtmlSyntaxTheme,
    output_path: Option<&Path>,
    project_root: Option<&Path>,
    site_context_input: Option<&SiteContextInput>,
) -> Result<String> {
    let name = entry.theme_name.clone();
    let mut env = no_autoescape_env();
    // Every injected fragment is already-escaped Typst HTML or trusted theme
    // content, so emit raw and avoid double-escaping (e.g. of an escaped title).
    env.add_template_owned("layout.html", entry.layout.clone())
        .map_err(|error| theme_error(&name, error))?;
    for (template_name, source) in &entry.partials {
        env.add_template_owned(template_name.clone(), source.clone())
            .map_err(|error| theme_error(&name, error))?;
    }

    let css: Vec<CssEntry> = entry
        .styles
        .iter()
        .map(|(name, source)| CssEntry {
            name: name.clone(),
            content: theme_css(source, syntax_theme),
        })
        .collect();
    let js: Vec<JsEntry> = entry
        .scripts
        .iter()
        .map(|(name, content)| JsEntry {
            name: name.clone(),
            content: content.clone(),
        })
        .collect();

    let site_title_heading = site_context_input
        .is_some()
        .then_some(parts.title.as_deref())
        .flatten();
    let toc_depth = site_context_input
        .and_then(|input| input.toc_depth)
        .unwrap_or(3);
    let (body, toc) = annotate_body_headings(parts.body, site_title_heading, toc_depth);
    let site = site_context(project_root, output_path, toc, site_context_input);
    let store = site.input.store.clone();
    let context = ThemeContext {
        doc: DocContext {
            head: parts.head.to_string(),
            body_open: parts.body_open.to_string(),
            body,
            body_close: parts.body_close.to_string(),
            title: parts.title.clone().unwrap_or_default(),
        },
        site,
        store,
        css,
        js,
        highlight_css: highlight_css(syntax_theme),
        theme: name.clone(),
        target: "html".to_string(),
    };

    let template = env
        .get_template("layout.html")
        .map_err(|error| theme_error(&name, error))?;
    template
        .render(&context)
        .map_err(|error| theme_error(&name, error))
}

fn site_context(
    project_root: Option<&Path>,
    output_path: Option<&Path>,
    toc: Vec<TocEntry>,
    site_context_input: Option<&SiteContextInput>,
) -> SiteContext {
    let input = site_context_input
        .cloned()
        .unwrap_or_else(|| default_site_context(project_root, output_path));
    let toc_floating = input.toc_floating.unwrap_or(true);
    SiteContext {
        input,
        toc,
        toc_floating,
    }
}

/// Fallback site context used when no website orchestrator supplied one: a
/// single sidebar section listing the sibling `.typ` documents.
fn default_site_context(
    project_root: Option<&Path>,
    output_path: Option<&Path>,
) -> SiteContextInput {
    let nav = nav_entries(project_root, output_path);
    let sidebar_sections = if nav.is_empty() {
        Vec::new()
    } else {
        vec![SiteNavSection {
            title: None,
            active: false,
            items: nav.clone(),
        }]
    };
    SiteContextInput {
        sidebar: nav,
        sidebar_sections,
        ..Default::default()
    }
}

fn nav_entries(project_root: Option<&Path>, output_path: Option<&Path>) -> Vec<SiteNavEntry> {
    let Some(root) = project_root else {
        return Vec::new();
    };
    let docs_src2 = root.join("docs-src2");
    let docs_dir = if docs_src2.is_dir() {
        docs_src2
    } else {
        root.to_path_buf()
    };
    let Ok(entries) = std::fs::read_dir(&docs_dir) else {
        return Vec::new();
    };
    let current_stem = output_path
        .and_then(|path| path.file_stem())
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();

    let mut stems = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("typ"))
        .filter_map(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| stem.to_string())
        })
        .collect::<Vec<_>>();
    stems.sort_by(|left, right| match (left.as_str(), right.as_str()) {
        ("index", "index") => std::cmp::Ordering::Equal,
        ("index", _) => std::cmp::Ordering::Less,
        (_, "index") => std::cmp::Ordering::Greater,
        _ => left.cmp(right),
    });

    stems
        .into_iter()
        .map(|stem| SiteNavEntry {
            href: format!("{stem}.html"),
            label: html_escape(&format!("{stem}.typ")),
            label_html: html_escape(&format!("{stem}.typ")),
            active: stem == current_stem,
        })
        .collect()
}

fn annotate_body_headings(
    body: &str,
    title_heading: Option<&str>,
    toc_depth: usize,
) -> (String, Vec<TocEntry>) {
    let mut out = String::with_capacity(body.len());
    let mut toc = Vec::new();
    let mut counts = HashMap::<String, usize>::new();
    let mut loc_redirects = Vec::<(String, String)>::new();
    let mut cursor = 0;
    let mut skipped_title_heading = false;

    while let Some((relative_start, role_heading)) = next_heading_start(&body[cursor..]) {
        let start = cursor + relative_start;
        let standard_level = if role_heading {
            None
        } else {
            let tag_start = start + 2;
            let Some(level_byte) = body.as_bytes().get(tag_start).copied() else {
                break;
            };
            if !(b'1'..=b'6').contains(&level_byte) {
                out.push_str(&body[cursor..start + 2]);
                cursor = start + 2;
                continue;
            }
            let after_level = tag_start + 1;
            let Some(after_byte) = body.as_bytes().get(after_level).copied() else {
                break;
            };
            if after_byte != b'>' && !after_byte.is_ascii_whitespace() {
                out.push_str(&body[cursor..after_level]);
                cursor = after_level;
                continue;
            }
            Some((level_byte - b'0') as usize)
        };

        let Some(open_offset) = body[start..].find(">") else {
            break;
        };
        let open_end = start + open_offset + 1;
        let open_tag = &body[start..open_end];
        let level = if role_heading {
            let Some(level) = role_heading_level(open_tag) else {
                out.push_str(&body[cursor..open_end]);
                cursor = open_end;
                continue;
            };
            level
        } else {
            let Some(level) = standard_level else {
                break;
            };
            level
        };
        let close_tag = if role_heading {
            "</div>".to_string()
        } else {
            format!("</h{level}>")
        };
        let Some(close_offset) = body[open_end..].find(&close_tag) else {
            break;
        };
        let close_start = open_end + close_offset;
        let close_end = close_start + close_tag.len();
        let inner = &body[open_end..close_start];
        // Derive the slug from the plain text, NOT from the re-escaped label:
        // escaping turns a stray `&` back into `&amp;`, which slugify would then
        // mangle into an `amp-` prefix. Keep `label` (escaped) for the TOC.
        let text = strip_html_tags(inner);
        let label = html_escape(&text);
        // An explicit Typst label is ferried in by the runtime as a marker
        // element just before the heading (see `heading_anchor_show_rule`).
        let marker = trailing_heading_anchor(&body[cursor..start]);
        // Typst's HTML export emits its own id on headings that are internal
        // reference targets; that id stays on the element, so the TOC must
        // link to it rather than to a computed one.
        let id = if let Some(existing) = existing_heading_id(open_tag) {
            *counts.entry(existing.clone()).or_insert(0) += 1;
            existing
        } else {
            // The marker label takes precedence over the slugified text.
            let explicit_id = marker.as_ref().map(|marker| marker.label.clone());
            let base_id = explicit_id.unwrap_or_else(|| slugify(&text));
            let count = counts.entry(base_id.clone()).or_insert(0);
            let id = if *count == 0 {
                base_id
            } else {
                format!("{base_id}-{}", *count + 1)
            };
            *count += 1;
            id
        };
        // When the heading is a reference target, Typst attaches its internal
        // id to the marker element (the first element the show rule emits) and
        // points reference links at it. The marker is stripped below, so those
        // links must be redirected to the heading's actual id.
        if let Some(loc) = marker.and_then(|marker| marker.typst_id) {
            if loc != id {
                loc_redirects.push((loc, id.clone()));
            }
        }

        out.push_str(&body[cursor..start]);
        if open_tag.contains(r#" id=""#) {
            out.push_str(open_tag);
        } else {
            out.push_str(&open_tag[..open_tag.len() - 1]);
            out.push_str(r#" id=""#);
            out.push_str(&html_escape(&id));
            out.push('"');
            out.push('>');
        }
        out.push_str(inner);
        out.push_str(&close_tag);
        let skip_toc = level == 1
            && !skipped_title_heading
            && title_heading.is_some_and(|title| title == label);
        if skip_toc {
            skipped_title_heading = true;
        } else if level <= toc_depth {
            toc.push(TocEntry {
                level,
                href: heading_fragment_href(&id),
                label,
            });
        }
        cursor = close_end;
    }

    out.push_str(&body[cursor..]);
    // Markers consumed above sit just before their heading and are removed
    // here; malformed or otherwise unassociated markers must not reach output.
    let mut out = if out.contains(HEADING_ANCHOR_TAG) {
        strip_heading_anchor_markers(&out)
    } else {
        out
    };
    for (loc, target) in loc_redirects {
        let from = format!("href=\"#{}\"", html_escape(&loc));
        let to = format!("href=\"{}\"", heading_fragment_href(&target));
        out = out.replace(&from, &to);
    }
    (out, toc)
}

const ROLE_HEADING_OPEN: &str = r#"<div role="heading" aria-level=""#;
const HEADING_ANCHOR_TAG: &str = "<calepin-heading-anchor";
const HEADING_ANCHOR_CLOSE: &str = "</calepin-heading-anchor>";

fn next_heading_start(value: &str) -> Option<(usize, bool)> {
    let standard = value.find("<h");
    let role = value.find(ROLE_HEADING_OPEN);
    match (standard, role) {
        (Some(standard), Some(role)) if standard <= role => Some((standard, false)),
        (Some(_), Some(role)) => Some((role, true)),
        (Some(standard), None) => Some((standard, false)),
        (None, Some(role)) => Some((role, true)),
        (None, None) => None,
    }
}

fn role_heading_level(open_tag: &str) -> Option<usize> {
    let rest = open_tag.strip_prefix(ROLE_HEADING_OPEN)?;
    let value_end = rest.find('"')?;
    rest[..value_end].parse().ok()
}

/// A heading-anchor marker emitted by the runtime just before a labeled
/// heading. `label` is the Typst label (`data-id`); `typst_id` is the id
/// Typst's HTML export attaches to the marker when the heading is the target
/// of an internal reference (attribute order is not guaranteed).
struct HeadingAnchor {
    label: String,
    typst_id: Option<String>,
}

/// Parses a heading-anchor marker that immediately precedes a heading
/// (trailing whitespace allowed). Returns `None` when `gap` does not end with
/// a well-formed marker carrying a non-empty label.
fn trailing_heading_anchor(gap: &str) -> Option<HeadingAnchor> {
    let before_close = gap.trim_end().strip_suffix(HEADING_ANCHOR_CLOSE)?;
    let open = before_close.rfind(HEADING_ANCHOR_TAG)?;
    let tag = &before_close[open..];
    // The marker is an empty element: its open tag must run to the close tag.
    if !tag.ends_with('>') || tag[..tag.len() - 1].contains('>') {
        return None;
    }
    let label = decode_basic_entities(&attribute_value(tag, "data-id")?);
    if label.is_empty() {
        return None;
    }
    let typst_id = attribute_value(tag, "id")
        .map(|value| decode_basic_entities(&value))
        .filter(|value| !value.is_empty());
    Some(HeadingAnchor { label, typst_id })
}

/// Reads a double-quoted attribute value from an element's open tag, returning
/// the raw (still-escaped) value.
fn attribute_value(tag: &str, name: &str) -> Option<String> {
    let marker = format!(" {name}=\"");
    let start = tag.find(&marker)? + marker.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_string())
}

/// Reads a pre-existing `id` attribute from a heading's open tag. Typst's HTML
/// export sets one on headings that are targets of internal references.
fn existing_heading_id(open_tag: &str) -> Option<String> {
    let id = decode_basic_entities(&attribute_value(open_tag, "id")?);
    (!id.is_empty()).then_some(id)
}

fn heading_fragment_href(id: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    let mut href = String::with_capacity(id.len() + 1);
    href.push('#');
    for byte in id.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b':') {
            href.push(byte as char);
        } else {
            href.push('%');
            href.push(HEX[(byte >> 4) as usize] as char);
            href.push(HEX[(byte & 0x0f) as usize] as char);
        }
    }
    href
}

fn strip_heading_anchor_markers(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut cursor = 0;
    while let Some(rel) = html[cursor..].find(HEADING_ANCHOR_TAG) {
        let open = cursor + rel;
        let Some(close_rel) = html[open..].find(HEADING_ANCHOR_CLOSE) else {
            break;
        };
        out.push_str(&html[cursor..open]);
        cursor = open + close_rel + HEADING_ANCHOR_CLOSE.len();
    }
    out.push_str(&html[cursor..]);
    out
}

fn strip_html_tags(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    decode_basic_entities(out.trim())
}

fn decode_basic_entities(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp..];
        if let Some(semi) = tail.find(';') {
            let entity = &tail[1..semi];
            let decoded = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => decode_numeric_entity(entity),
            };
            if let Some(ch) = decoded {
                out.push(ch);
                rest = &tail[semi + 1..];
                continue;
            }
        }
        // Not a recognized entity: keep the '&' literally and move on.
        out.push('&');
        rest = &tail[1..];
    }
    out.push_str(rest);
    out
}

/// Decode a numeric character reference body such as `#39`, `#x20`, or `#xE9`
/// (the part between `&` and `;`). Returns `None` for non-numeric entities.
fn decode_numeric_entity(entity: &str) -> Option<char> {
    let digits = entity.strip_prefix('#')?;
    let code = match digits.strip_prefix(['x', 'X']) {
        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
        None => digits.parse::<u32>().ok()?,
    };
    char::from_u32(code)
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut pending_dash = false;
    for ch in value.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(ch);
            pending_dash = false;
        } else {
            pending_dash = true;
        }
    }
    if slug.is_empty() {
        "section".to_string()
    } else {
        slug
    }
}

fn theme_error(name: &str, error: minijinja::Error) -> anyhow::Error {
    anyhow!("theme `{name}`: {error}")
}

fn split_html_document(html: &str) -> Result<HtmlDocumentParts<'_>> {
    let head_close = html
        .find("</head>")
        .ok_or_else(|| anyhow!("HTML output is missing </head>"))?;
    let body_start = html
        .find("<body")
        .ok_or_else(|| anyhow!("HTML output is missing <body>"))?;
    let body_open_end = html[body_start..]
        .find('>')
        .map(|offset| body_start + offset + 1)
        .ok_or_else(|| anyhow!("HTML output has an unterminated <body> tag"))?;
    let body_close = html
        .rfind("</body>")
        .ok_or_else(|| anyhow!("HTML output is missing </body>"))?;
    if body_close < body_open_end {
        return Err(anyhow!("HTML output has </body> before the body content"));
    }

    Ok(HtmlDocumentParts {
        head: &html[..head_close],
        body_open: &html[head_close..body_open_end],
        body: &html[body_open_end..body_close],
        body_close: &html[body_close..],
        title: html_title(&html[..head_close]),
    })
}

fn html_title(head: &str) -> Option<String> {
    let start = head.find("<title>")? + "<title>".len();
    let end = head[start..].find("</title>")? + start;
    Some(head[start..end].trim().to_string())
}

/// Substitute the syntax-color placeholders inside a theme CSS asset.
pub(crate) fn theme_css(source: &str, syntax_theme: &HtmlSyntaxTheme) -> String {
    source
        .replace(
            "__CALEPIN_SYNTAX_LIGHT__",
            &syntax_theme.declarations(false),
        )
        .replace("__CALEPIN_SYNTAX_CLASSES__", &syntax_theme.class_rules())
        .replace("__CALEPIN_SYNTAX_DARK__", &syntax_theme.declarations(true))
}

/// The standalone highlight CSS exposed as `highlight_css` (inner CSS; the
/// layout wraps it in a `<style>` element). Themes that embed the
/// `__CALEPIN_SYNTAX_*__` placeholders in their own CSS do not need it.
fn highlight_css(syntax_theme: &HtmlSyntaxTheme) -> String {
    format!(
        ":root {{\n{}}}\n\n{}\nhtml[data-theme=\"dark\"] {{\n{}}}\n\n@media (prefers-color-scheme: dark) {{\n  html:not([data-theme]) {{\n{}  }}\n}}\n",
        syntax_theme.declarations(false),
        syntax_theme.class_rules(),
        syntax_theme.declarations(true),
        syntax_theme.declarations(true),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heading_id(body: &str) -> String {
        let (out, _) = annotate_body_headings(body, None, 3);
        let marker = r#" id=""#;
        let start = out.find(marker).expect("heading should get an id") + marker.len();
        let end = out[start..].find('"').unwrap() + start;
        out[start..end].to_string()
    }

    #[test]
    fn plain_heading_slug_is_unchanged() {
        assert_eq!(heading_id("<h2>Plain Research</h2>"), "plain-research");
    }

    #[test]
    fn inline_markup_does_not_leak_into_the_slug() {
        // An icon element plus Typst's `&#x20;` spacing span used to produce
        // `amp-x20-iconic-research`; the slug should come from the text only.
        let body = r#"<h2><i class="bi bi-pen"></i><span style="white-space: pre-wrap">&#x20;</span>Iconic Research</h2>"#;
        assert_eq!(heading_id(body), "iconic-research");
    }

    #[test]
    fn numeric_character_references_are_decoded() {
        // `&#xE9;` decodes to `é`, which the ASCII-only slugify then drops — so
        // the slug is a clean `caf`, not the old leaked `caf-xe9`.
        assert_eq!(heading_id("<h2>caf&#xE9;</h2>"), "caf");
        // The decoder itself handles decimal and hex refs.
        assert_eq!(decode_basic_entities("a&#38;b"), "a&b");
        assert_eq!(decode_basic_entities("&#x20;hi"), " hi");
    }

    #[test]
    fn unrecognized_ampersands_are_preserved() {
        assert_eq!(decode_basic_entities("Tom & Jerry"), "Tom & Jerry");
    }

    fn anchor(label: &str) -> String {
        format!(r#"<calepin-heading-anchor data-id="{label}"></calepin-heading-anchor>"#)
    }

    #[test]
    fn explicit_label_marker_sets_the_id() {
        let body = format!("{}<h3>Labeled Plain</h3>", anchor("plainlabel"));
        assert_eq!(heading_id(&body), "plainlabel");
    }

    #[test]
    fn explicit_label_wins_over_inline_content() {
        // The label is honored even when inline markup would otherwise drive
        // the slug — this is the case the workaround in #70 worked around.
        let body = format!(
            r#"{}<h3><i class="bi bi-pen"></i><span style="white-space: pre-wrap">&#x20;</span>Iconic Research</h3>"#,
            anchor("myresearch")
        );
        assert_eq!(heading_id(&body), "myresearch");
    }

    #[test]
    fn label_marker_is_stripped_and_toc_points_at_the_label_id() {
        let body = format!("{}<h2>Intro</h2>", anchor("sec:intro"));
        let (out, toc) = annotate_body_headings(&body, None, 3);
        assert!(!out.contains("calepin-heading-anchor"), "{out}");
        // Colon labels are kept verbatim (valid in HTML ids and URL fragments).
        assert!(out.contains(r#"<h2 id="sec:intro">Intro</h2>"#), "{out}");
        assert_eq!(toc.len(), 1);
        assert_eq!(toc[0].href, "#sec:intro");
        assert_eq!(toc[0].label, "Intro");
    }

    #[test]
    fn labeled_and_slugged_ids_share_one_dedup_space() {
        let body = format!(
            "{}<h2>One</h2><h2>One</h2>",
            anchor("one") // explicit "one" collides with the slug of "One"
        );
        let (out, _) = annotate_body_headings(&body, None, 3);
        assert!(out.contains(r#"<h2 id="one">One</h2>"#), "{out}");
        assert!(out.contains(r#"<h2 id="one-2">One</h2>"#), "{out}");
    }

    #[test]
    fn explicit_label_sets_id_on_role_heading() {
        // Typst renders a level-6 heading as a role-based <div> because HTML
        // has no <h7>; its explicit label must still provide a stable anchor.
        let body = format!(
            r#"{}<div role="heading" aria-level="7">Deep</div>"#,
            anchor("deep")
        );
        let (out, _) = annotate_body_headings(&body, None, 3);
        assert!(!out.contains("calepin-heading-anchor"), "{out}");
        assert!(
            out.contains(r#"<div role="heading" aria-level="7" id="deep">Deep</div>"#),
            "{out}"
        );
    }

    #[test]
    fn explicit_label_is_safe_in_id_and_toc_href() {
        let body = format!("{}<h2>Unsafe</h2>", anchor("x&quot;&amp; y"));
        let (out, toc) = annotate_body_headings(&body, None, 3);

        assert!(out.contains(r#"id="x&quot;&amp; y""#), "{out}");
        assert!(!out.contains(r#"id="x"& y""#), "{out}");
        assert_eq!(toc[0].href, "#x%22%26%20y");
    }

    #[test]
    fn explicit_label_preserves_surrounding_whitespace() {
        let body = format!("{}<h2>Spaced</h2>", anchor("&#x20;space&#x20;"));
        let (out, toc) = annotate_body_headings(&body, None, 3);

        assert!(out.contains(r#"id=" space ""#), "{out}");
        assert_eq!(toc[0].href, "#%20space%20");
    }

    #[test]
    fn referenced_labeled_heading_keeps_label_and_redirects_reference_links() {
        // When a labeled heading is referenced with `@label`, Typst attaches
        // its internal id to the marker element, before `data-id`. The label
        // must still drive the heading id, the marker must not leak into the
        // output, and the reference link must point at the heading.
        let body = concat!(
            r##"<p>See <a href="#loc-1">here</a>.</p>"##,
            r#"<calepin-heading-anchor id="loc-1" data-id="sec:intro"></calepin-heading-anchor>"#,
            "<h3>Intro</h3>",
        );
        let (out, toc) = annotate_body_headings(body, None, 3);
        assert!(!out.contains("calepin-heading-anchor"), "{out}");
        assert!(out.contains(r#"<h3 id="sec:intro">Intro</h3>"#), "{out}");
        assert!(out.contains(r##"<a href="#sec:intro">here</a>"##), "{out}");
        assert_eq!(toc.len(), 1);
        assert_eq!(toc[0].href, "#sec:intro");
    }

    #[test]
    fn toc_links_to_a_preexisting_heading_id() {
        // Typst's HTML export puts its own id (e.g. `loc-1`) on headings that
        // are internal reference targets; the tag is kept as-is, so the TOC
        // must link to that id, not to the label or slug.
        let body = format!("{}<h2 id=\"loc-1\">Intro</h2>", anchor("sec:intro"));
        let (out, toc) = annotate_body_headings(&body, None, 3);
        assert!(out.contains(r#"<h2 id="loc-1">Intro</h2>"#), "{out}");
        assert_eq!(toc.len(), 1);
        assert_eq!(toc[0].href, "#loc-1");
    }

    #[test]
    fn preexisting_ids_share_the_dedup_space() {
        // A later heading whose slug collides with a Typst-emitted id must be
        // suffixed away from it.
        let body = r#"<h2 id="loc-1">First</h2><h2>Loc 1</h2>"#;
        let (out, toc) = annotate_body_headings(body, None, 3);
        assert!(out.contains(r#"<h2 id="loc-1-2">Loc 1</h2>"#), "{out}");
        assert_eq!(toc[0].href, "#loc-1");
        assert_eq!(toc[1].href, "#loc-1-2");
    }

    #[test]
    fn unlabeled_heading_falls_back_to_the_slug() {
        assert_eq!(heading_id("<h2>Plain Research</h2>"), "plain-research");
        // A malformed marker (no closing tag) is ignored, not used as an id.
        let body = r#"<calepin-heading-anchor data-id="x"><h2>Fallback Here</h2>"#;
        assert_eq!(heading_id(body), "fallback-here");
    }
}
