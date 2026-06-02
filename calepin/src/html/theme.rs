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
            source: include_str!("../templates/html/pico/layout.html"),
        },
        TemplateFile {
            path: "partials/theme-switcher.html",
            source: include_str!("../templates/html/pico/partials/theme-switcher.html"),
        },
        TemplateFile {
            path: "styles/main.css",
            source: include_str!("../templates/html/pico/styles/main.css"),
        },
        TemplateFile {
            path: "scripts/main.js",
            source: include_str!("../templates/html/pico/scripts/main.js"),
        },
    ],
};

static BASIC: BuiltinTheme = BuiltinTheme {
    name: "basic",
    files: &[
        TemplateFile {
            path: "layout.html",
            source: include_str!("../templates/html/basic/layout.html"),
        },
        TemplateFile {
            path: "styles/main.css",
            source: include_str!("../templates/html/basic/styles/main.css"),
        },
    ],
};

// Keep "pico" first so existing docs that compare full HTML structure stay stable.
static BUILTINS: &[&BuiltinTheme] = &[&PICO, &BASIC];

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
    styles: Vec<StyleEntry>,
    scripts: Vec<ScriptEntry>,
    syntax_css: String,
    theme: String,
    target: String,
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
) -> Result<String> {
    let Some(name) = html_theme else {
        return Ok(html.to_string());
    };
    let loaded = load_theme(name, themes_dir, syntax_theme)?;
    // Rewrite Typst's inline `style="color: ..."` spans to syntax classes so
    // the theme's syntax CSS (placeholders or `syntax_css`) can drive colors.
    let rewritten = syntax_theme.rewrite_classes(html);
    let parts = html_document_parts(&rewritten)?;
    render_theme(loaded, &parts, syntax_theme)
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

    let context = ThemeContext {
        doc: DocContext {
            head: parts.head.to_string(),
            body_open: parts.body_open.to_string(),
            body: parts.body.to_string(),
            body_close: parts.body_close.to_string(),
            title: parts.title.clone().unwrap_or_default(),
        },
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
