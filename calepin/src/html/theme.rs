use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use minijinja::{AutoEscape, Environment};
use serde::Serialize;

use super::syntax::HtmlSyntaxTheme;

/// A theme bundled into the binary. User themes on disk mirror this shape:
/// a `layout.html`, plus optional `partials/`, `styles/`, and `scripts/`.
struct BuiltinTheme {
    name: &'static str,
    files: &'static [TemplateFile],
}

struct TemplateFile {
    path: &'static str,
    source: &'static str,
}

static PICO: BuiltinTheme = BuiltinTheme {
    name: "pico",
    files: &[
        TemplateFile {
            path: "layout.html",
            source: include_str!("../assets/themes/html/document/pico/layout.html"),
        },
        TemplateFile {
            path: "partials/theme-switcher.html",
            source: include_str!(
                "../assets/themes/html/document/pico/partials/theme-switcher.html"
            ),
        },
        TemplateFile {
            path: "styles/main.css",
            source: include_str!("../assets/themes/html/document/pico/styles/main.css"),
        },
        TemplateFile {
            path: "scripts/main.js",
            source: include_str!("../assets/themes/html/document/pico/scripts/main.js"),
        },
    ],
};

static BASIC: BuiltinTheme = BuiltinTheme {
    name: "basic",
    files: &[
        TemplateFile {
            path: "layout.html",
            source: include_str!("../assets/themes/html/document/basic/layout.html"),
        },
        TemplateFile {
            path: "styles/main.css",
            source: include_str!("../assets/themes/html/document/basic/styles/main.css"),
        },
    ],
};

static CALEPIN_WEBSITE: BuiltinTheme = BuiltinTheme {
    name: "calepin-website",
    files: &[
        TemplateFile {
            path: "layout.html",
            source: include_str!("../assets/themes/html/website/calepin-website/layout.html"),
        },
        TemplateFile {
            path: "styles/main.css",
            source: include_str!("../assets/themes/html/website/calepin-website/styles/main.css"),
        },
        TemplateFile {
            path: "scripts/app.js",
            source: include_str!("../assets/themes/html/website/calepin-website/scripts/app.js"),
        },
    ],
};

// Keep "pico" first so existing docs that compare full HTML structure stay stable.
static BUILTINS: &[&BuiltinTheme] = &[&PICO, &BASIC, &CALEPIN_WEBSITE];

fn builtin(name: &str) -> Option<&'static BuiltinTheme> {
    BUILTINS.iter().copied().find(|theme| theme.name == name)
}

#[derive(Serialize)]
struct StyleEntry {
    name: String,
    css: String,
}

#[derive(Serialize)]
struct ScriptEntry {
    name: String,
    content: String,
}

/// A theme resolved to concrete, render-ready sources.
struct LoadedTheme {
    name: String,
    layout: String,
    /// `(template name, source)` where the name is `partials/<file>.html`, so
    /// `{% include "partials/<file>.html" %}` resolves it.
    partials: Vec<(String, String)>,
    styles: Vec<StyleEntry>,
    scripts: Vec<ScriptEntry>,
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
    styles: Vec<StyleEntry>,
    scripts: Vec<ScriptEntry>,
    syntax_css: String,
    theme: String,
    target: String,
}

#[derive(Serialize, Debug, Clone, Default)]
pub(crate) struct SiteContextInput {
    pub(crate) nav: Vec<SiteNavEntry>,
    pub(crate) nav_sections: Vec<SiteNavSection>,
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) base_url: Option<String>,
    pub(crate) logo: Option<String>,
    pub(crate) logo_alt: Option<String>,
    pub(crate) home_url: Option<String>,
    pub(crate) github_url: Option<String>,
    pub(crate) current_url: Option<String>,
    pub(crate) page_title: Option<String>,
}

#[derive(Serialize)]
struct SiteContext {
    nav: Vec<NavEntry>,
    nav_sections: Vec<NavSection>,
    toc: Vec<TocEntry>,
    title: Option<String>,
    description: Option<String>,
    base_url: Option<String>,
    logo: Option<String>,
    logo_alt: Option<String>,
    home_url: Option<String>,
    github_url: Option<String>,
    current_url: Option<String>,
    page_title: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub(crate) struct SiteNavSection {
    pub(crate) title: Option<String>,
    pub(crate) items: Vec<SiteNavEntry>,
}

#[derive(Serialize)]
struct NavSection {
    title: Option<String>,
    items: Vec<NavEntry>,
}

#[derive(Serialize, Clone)]
struct NavEntry {
    href: String,
    label: String,
    active: bool,
}

#[derive(Serialize, Debug, Clone)]
pub(crate) struct SiteNavEntry {
    pub(crate) href: String,
    pub(crate) label: String,
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
    html_theme: Option<&str>,
    themes_dir: &Path,
    syntax_theme: &HtmlSyntaxTheme,
    output_path: Option<&Path>,
    project_root: Option<&Path>,
    site_context: Option<&SiteContextInput>,
) -> Result<String> {
    let Some(name) = html_theme else {
        return Ok(html.to_string());
    };
    let loaded = load_theme(name, themes_dir, syntax_theme)?;
    // Rewrite Typst's inline `style="color: ..."` spans to syntax classes so
    // the theme's syntax CSS (placeholders or `syntax_css`) can drive colors.
    let rewritten = syntax_theme.rewrite_classes(html);
    // Typst emits a bare HTML fragment (no <html>/<head>/<body> wrapper) when
    // the document has no explicit html.elem("html") root element, i.e. when
    // the user has not called #calepin.html[...].  Wrap those fragments in a
    // minimal well-formed document so html_document_parts can always locate
    // </head>, <body>, and </body>.
    let normalized = if rewritten.contains("</head>") {
        rewritten
    } else {
        format!(
            "<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"></head><body>{}</body></html>",
            rewritten.trim()
        )
    };
    let parts = html_document_parts(&normalized)?;
    render_theme(
        loaded,
        &parts,
        syntax_theme,
        output_path,
        project_root,
        site_context,
    )
}

fn load_theme(
    name: &str,
    themes_dir: &Path,
    syntax_theme: &HtmlSyntaxTheme,
) -> Result<LoadedTheme> {
    let dir = themes_dir.join(name);
    if dir.is_dir() {
        load_directory_theme(name, &dir, syntax_theme)
    } else if let Some(theme) = builtin(name) {
        load_builtin_theme(theme, syntax_theme)
    } else {
        Err(anyhow!("unknown HTML theme `{name}`"))
    }
}

fn load_builtin_theme(theme: &BuiltinTheme, syntax_theme: &HtmlSyntaxTheme) -> Result<LoadedTheme> {
    let layout_path = theme
        .files
        .iter()
        .find(|file| file.path == "layout.html")
        .map(|file| file.source)
        .ok_or_else(|| anyhow!("missing `layout.html` for builtin theme `{}`", theme.name))?;

    let mut styles = theme
        .files
        .iter()
        .filter(|file| file.path.starts_with("styles/") && file.path.ends_with(".css"))
        .map(|file| {
            let name = file
                .path
                .rsplit_once('/')
                .map(|(_, name)| name)
                .unwrap_or(file.path);
            (name, file.source)
        })
        .collect::<Vec<_>>();
    styles.sort_by_key(|(name, _)| *name);

    let mut scripts = theme
        .files
        .iter()
        .filter(|file| file.path.starts_with("scripts/") && file.path.ends_with(".js"))
        .map(|file| {
            let name = file
                .path
                .rsplit_once('/')
                .map(|(_, name)| name)
                .unwrap_or(file.path);
            (name, file.source)
        })
        .collect::<Vec<_>>();
    scripts.sort_by_key(|(name, _)| *name);

    let mut partials = theme
        .files
        .iter()
        .filter(|file| file.path.starts_with("partials/") && file.path.ends_with(".html"))
        .map(|file| {
            let name = file
                .path
                .rsplit_once('/')
                .map(|(_, name)| name)
                .unwrap_or(file.path);
            (name, file.source.to_string())
        })
        .collect::<Vec<_>>();
    partials.sort_by(|a, b| a.0.cmp(&b.0));

    Ok(LoadedTheme {
        name: theme.name.to_string(),
        layout: layout_path.to_string(),
        partials: partials
            .into_iter()
            .map(|(file, source)| (format!("partials/{file}"), source))
            .collect(),
        styles: styles
            .into_iter()
            .map(|(file, css)| StyleEntry {
                name: file.to_string(),
                css: theme_css(css, syntax_theme),
            })
            .collect(),
        scripts: scripts
            .into_iter()
            .map(|(file, content)| ScriptEntry {
                name: file.to_string(),
                content: content.to_string(),
            })
            .collect(),
    })
}

fn load_directory_theme(
    name: &str,
    dir: &Path,
    syntax_theme: &HtmlSyntaxTheme,
) -> Result<LoadedTheme> {
    let layout_path = dir.join("layout.html");
    let layout = std::fs::read_to_string(&layout_path)
        .with_context(|| format!("theme `{name}`: failed to read {}", layout_path.display()))?;

    let partials = read_theme_files(&dir.join("partials"), "html")?
        .into_iter()
        .map(|(file, source)| (format!("partials/{file}"), source))
        .collect();
    let styles = read_theme_files(&dir.join("styles"), "css")?
        .into_iter()
        .map(|(file, css)| StyleEntry {
            name: file,
            css: theme_css(&css, syntax_theme),
        })
        .collect();
    let scripts = read_theme_files(&dir.join("scripts"), "js")?
        .into_iter()
        .map(|(file, content)| ScriptEntry {
            name: file,
            content,
        })
        .collect();

    Ok(LoadedTheme {
        name: name.to_string(),
        layout,
        partials,
        styles,
        scripts,
    })
}

/// Read every `*.<ext>` file in `dir`, sorted by filename for deterministic
/// order. A missing directory yields no files.
fn read_theme_files(dir: &Path, ext: &str) -> Result<Vec<(String, String)>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("failed to read {}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some(ext))
        .collect();
    paths.sort();

    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        files.push((name, contents));
    }
    Ok(files)
}

fn render_theme(
    loaded: LoadedTheme,
    parts: &HtmlDocumentParts<'_>,
    syntax_theme: &HtmlSyntaxTheme,
    output_path: Option<&Path>,
    project_root: Option<&Path>,
    site_context_input: Option<&SiteContextInput>,
) -> Result<String> {
    let name = loaded.name;
    let mut env = Environment::new();
    // Every injected fragment is already-escaped Typst HTML or trusted theme
    // content, so emit raw and avoid double-escaping (e.g. of an escaped title).
    env.set_auto_escape_callback(|_| AutoEscape::None);
    env.add_template_owned("layout.html", loaded.layout)
        .map_err(|error| theme_error(&name, error))?;
    for (template_name, source) in loaded.partials {
        env.add_template_owned(template_name, source)
            .map_err(|error| theme_error(&name, error))?;
    }

    let (body, toc) = body_with_heading_ids(parts.body);
    let context = ThemeContext {
        doc: DocContext {
            head: parts.head.to_string(),
            body_open: parts.body_open.to_string(),
            body,
            body_close: parts.body_close.to_string(),
            title: parts.title.clone().unwrap_or_default(),
        },
        site: site_context(project_root, output_path, toc, site_context_input),
        styles: loaded.styles,
        scripts: loaded.scripts,
        syntax_css: syntax_css(syntax_theme),
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
    if let Some(input) = site_context_input {
        return SiteContext {
            nav: input
                .nav
                .iter()
                .map(|item| NavEntry {
                    href: item.href.clone(),
                    label: item.label.clone(),
                    active: item.active,
                })
                .collect(),
            nav_sections: input
                .nav_sections
                .iter()
                .map(|section| NavSection {
                    title: section.title.clone(),
                    items: section
                        .items
                        .iter()
                        .map(|item| NavEntry {
                            href: item.href.clone(),
                            label: item.label.clone(),
                            active: item.active,
                        })
                        .collect(),
                })
                .collect(),
            toc,
            title: input.title.clone(),
            description: input.description.clone(),
            base_url: input.base_url.clone(),
            logo: input.logo.clone(),
            logo_alt: input.logo_alt.clone(),
            home_url: input.home_url.clone(),
            github_url: input.github_url.clone(),
            current_url: input.current_url.clone(),
            page_title: input.page_title.clone(),
        };
    }

    let nav = nav_entries(project_root, output_path);
    SiteContext {
        nav_sections: if nav.is_empty() {
            Vec::new()
        } else {
            vec![NavSection {
                title: None,
                items: nav.clone(),
            }]
        },
        nav,
        toc,
        title: None,
        description: None,
        base_url: None,
        logo: None,
        logo_alt: None,
        home_url: None,
        github_url: None,
        current_url: None,
        page_title: None,
    }
}

fn nav_entries(project_root: Option<&Path>, output_path: Option<&Path>) -> Vec<NavEntry> {
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
        .map(|stem| NavEntry {
            href: format!("{stem}.html"),
            label: html_escape(&format!("{stem}.typ")),
            active: stem == current_stem,
        })
        .collect()
}

fn body_with_heading_ids(body: &str) -> (String, Vec<TocEntry>) {
    let mut out = String::with_capacity(body.len());
    let mut toc = Vec::new();
    let mut counts = HashMap::<String, usize>::new();
    let mut cursor = 0;

    while let Some(relative_start) = body[cursor..].find("<h") {
        let start = cursor + relative_start;
        let tag_start = start + 2;
        let Some(level_byte) = body.as_bytes().get(tag_start).copied() else {
            break;
        };
        if !(b"1"[0]..=b"6"[0]).contains(&level_byte) {
            out.push_str(&body[cursor..start + 2]);
            cursor = start + 2;
            continue;
        }
        let level = (level_byte - b"0"[0]) as usize;
        let after_level = tag_start + 1;
        let Some(after_byte) = body.as_bytes().get(after_level).copied() else {
            break;
        };
        if after_byte != b">"[0] && !after_byte.is_ascii_whitespace() {
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
            out.push(34 as char);
            out.push(62 as char);
        }
        out.push_str(inner);
        out.push_str(&close_tag);
        if level <= 3 {
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
        match ch as u32 {
            60 => in_tag = true,
            62 => in_tag = false,
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
        .replace("&#39;", "\u{27}")
}

fn html_escape(value: &str) -> String {
    value
        .replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut pending_dash = false;
    for ch in value.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push(45 as char);
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

fn html_document_parts(html: &str) -> Result<HtmlDocumentParts<'_>> {
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
fn theme_css(source: &str, syntax_theme: &HtmlSyntaxTheme) -> String {
    source
        .replace(
            "__CALEPIN_SYNTAX_LIGHT__",
            &syntax_theme.declarations(false),
        )
        .replace("__CALEPIN_SYNTAX_CLASSES__", &syntax_theme.class_rules())
        .replace("__CALEPIN_SYNTAX_DARK__", &syntax_theme.declarations(true))
}

/// The standalone syntax CSS exposed as `syntax_css` (inner CSS; the layout
/// wraps it in a `<style>` element). Themes that embed the
/// `__CALEPIN_SYNTAX_*__` placeholders in their own CSS do not need it.
fn syntax_css(syntax_theme: &HtmlSyntaxTheme) -> String {
    format!(
        ":root {{\n{}}}\n\n{}\nhtml[data-theme=\"dark\"] {{\n{}}}\n\n@media (prefers-color-scheme: dark) {{\n  html:not([data-theme]) {{\n{}  }}\n}}\n",
        syntax_theme.declarations(false),
        syntax_theme.class_rules(),
        syntax_theme.declarations(true),
        syntax_theme.declarations(true),
    )
}
