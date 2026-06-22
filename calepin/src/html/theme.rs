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
    vars: serde_json::Value,
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
    pub(crate) vars: serde_json::Value,
    /// Resolved max heading level included in the on-page TOC: `Some(0)` means
    /// no TOC, `Some(n)` includes headings up to level `n`. `None` (the
    /// default, used when no caller resolves it) falls back to the original
    /// always-on depth-3 behavior. Not serialized: only `annotate_body_headings`
    /// reads it, the computed `site.toc` entries are what templates see.
    #[serde(skip)]
    pub(crate) toc_depth: Option<usize>,
}

/// The site context handed to the template: the caller-supplied input (or a
/// filesystem-derived fallback) plus the per-page `toc` computed during render.
#[derive(Serialize)]
struct SiteContext {
    #[serde(flatten)]
    input: SiteContextInput,
    toc: Vec<TocEntry>,
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
    let vars = site.input.vars.clone();
    let context = ThemeContext {
        doc: DocContext {
            head: parts.head.to_string(),
            body_open: parts.body_open.to_string(),
            body,
            body_close: parts.body_close.to_string(),
            title: parts.title.clone().unwrap_or_default(),
        },
        site,
        vars,
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
    SiteContext { input, toc }
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
    let mut cursor = 0;
    let mut skipped_title_heading = false;

    while let Some(relative_start) = body[cursor..].find("<h") {
        let start = cursor + relative_start;
        let tag_start = start + 2;
        let Some(level_byte) = body.as_bytes().get(tag_start).copied() else {
            break;
        };
        if !(b'1'..=b'6').contains(&level_byte) {
            out.push_str(&body[cursor..start + 2]);
            cursor = start + 2;
            continue;
        }
        let level = (level_byte - b'0') as usize;
        let after_level = tag_start + 1;
        let Some(after_byte) = body.as_bytes().get(after_level).copied() else {
            break;
        };
        if after_byte != b'>' && !after_byte.is_ascii_whitespace() {
            out.push_str(&body[cursor..after_level]);
            cursor = after_level;
            continue;
        }

        let Some(open_offset) = body[start..].find(">") else {
            break;
        };
        let open_end = start + open_offset + 1;
        let close_tag = format!("</h{level}>");
        let Some(close_offset) = body[open_end..].find(&close_tag) else {
            break;
        };
        let close_start = open_end + close_offset;
        let close_end = close_start + close_tag.len();
        let open_tag = &body[start..open_end];
        let inner = &body[open_end..close_start];
        let label = html_escape(&strip_html_tags(inner));
        let base_id = slugify(&label);
        let count = counts.entry(base_id.clone()).or_insert(0);
        let id = if *count == 0 {
            base_id
        } else {
            format!("{base_id}-{}", *count + 1)
        };
        *count += 1;

        out.push_str(&body[cursor..start]);
        if open_tag.contains(r#" id=""#) {
            out.push_str(open_tag);
        } else {
            out.push_str(&open_tag[..open_tag.len() - 1]);
            out.push_str(r#" id=""#);
            out.push_str(&id);
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
                href: format!("#{id}"),
                label,
            });
        }
        cursor = close_end;
    }

    out.push_str(&body[cursor..]);
    (out, toc)
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
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
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
