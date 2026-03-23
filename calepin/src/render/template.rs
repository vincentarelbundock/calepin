use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::types::Metadata;

/// Build an HTML table of contents from heading metadata collected during the AST walk.
pub fn build_toc_html(headings: &[crate::render::ast::TocEntry], depth: u8, title: &str) -> String {
    let items: Vec<(u8, &str, &str)> = headings.iter()
        .filter(|h| h.level <= depth)
        .filter(|h| !h.classes.iter().any(|c| c == "unlisted"))
        .filter(|h| !h.text.is_empty())
        .map(|h| (h.level, h.id.as_str(), h.text.as_str()))
        .collect();
    build_toc_html_from_items(&items, title)
}

/// Build an HTML table of contents by extracting headings from rendered HTML (fallback).
pub fn build_toc_html_from_body(body: &str, depth: u8, title: &str) -> String {
    static RE_TOC_HEADING: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"<h([1-6])\s[^>]*id="([^"]+)"[^>]*>(.*?)</h[1-6]>"#).unwrap()
    });
    static RE_TOC_TAG: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"<[^>]+>").unwrap()
    });
    // Can't return refs to captures, so use owned version
    let owned_items: Vec<(u8, String, String)> = RE_TOC_HEADING.captures_iter(body)
        .filter_map(|cap| {
            let level: u8 = cap[1].parse().ok()?;
            if level > depth { return None; }
            let full_tag = cap.get(0)?.as_str();
            if full_tag.contains("unlisted") { return None; }
            let id = cap[2].to_string();
            let text = RE_TOC_TAG.replace_all(&cap[3], "").trim().to_string();
            if text.is_empty() { return None; }
            Some((level, id, text))
        })
        .collect();
    let items: Vec<(u8, &str, &str)> = owned_items.iter()
        .map(|(l, id, text)| (*l, id.as_str(), text.as_str()))
        .collect();
    build_toc_html_from_items(&items, title)
}

fn build_toc_html_from_items(items: &[(u8, &str, &str)], title: &str) -> String {
    if items.is_empty() { return String::new(); }

    let min_level = items.iter().map(|(l, _, _)| *l).min().unwrap_or(1);
    let mut html = format!("<nav class=\"toc\" aria-label=\"{}\">\n<p class=\"toc-title\">{}</p>\n<ul>\n", title, title);
    let mut current_level = min_level;
    let mut first = true;

    for (level, id, text) in items {
        if *level > current_level {
            // Going deeper: nest inside the current <li> (which was left open)
            while current_level < *level {
                html.push_str("\n<ul>\n");
                current_level += 1;
            }
        } else {
            // Close the previous <li> before siblings or shallower items
            if !first {
                html.push_str("</li>\n");
            }
            // Going shallower: close nested lists
            while current_level > *level {
                html.push_str("</ul>\n</li>\n");
                current_level -= 1;
            }
        }
        html.push_str(&format!("<li><a href=\"#{}\">{}</a>", id, text));
        first = false;
    }

    // Close all remaining open tags
    if !first {
        html.push_str("</li>\n");
    }
    while current_level > min_level {
        html.push_str("</ul>\n</li>\n");
        current_level -= 1;
    }

    html.push_str("</ul>\n</nav>");
    html
}

use crate::render::metadata::{strip_markdown_formatting, build_appendix, build_author_block};

/// Load a page template by name and base.
///
/// Resolution order:
///   1. Project filesystem (templates/{target}/, templates/{base}/, or templates/common/)
///   2. Built-in (discovered from embedded project tree)
pub fn load_page_template(template_name: &str, base: &str) -> String {
    // Filesystem resolution
    if let Some(path) = crate::paths::resolve_template(template_name, base) {
        if let Ok(s) = std::fs::read_to_string(&path) {
            return s;
        }
    }
    // Built-in: discovered from embedded project tree
    crate::render::elements::resolve_builtin_template(template_name, base)
        .unwrap_or("")
        .to_string()
}


pub fn load_default_css() -> String {
    // Check project/user overrides for CSS
    let root = std::path::Path::new(".");
    // Try new name first, then legacy
    let p = root.join("templates").join("html").join("page.css");
    if p.exists() {
        if let Ok(s) = std::fs::read_to_string(&p) {
            return s;
        }
    }
    let p = root.join("templates").join("html").join("calepin.css");
    if p.exists() {
        if let Ok(s) = std::fs::read_to_string(&p) {
            return s;
        }
    }
    // Built-in: discovered from embedded project tree
    crate::render::elements::BUILTIN_PROJECT
        .get_file("templates/html/page.css")
        .and_then(|f| f.contents_utf8())
        .unwrap_or("")
        .to_string()
}

/// Build a MiniJinja render context from a variable map.
/// Inserts `_lb` and `_rb` helpers for literal braces in LaTeX templates.
fn build_jinja_context<'a>(vars: &'a HashMap<String, String>) -> std::collections::BTreeMap<&'a str, minijinja::Value> {
    let mut ctx = std::collections::BTreeMap::new();
    for (key, value) in vars {
        ctx.insert(key.as_str(), minijinja::Value::from(value.as_str()));
    }
    ctx.insert("_lb", minijinja::Value::from("{"));
    ctx.insert("_rb", minijinja::Value::from("}"));
    ctx
}

/// Apply MiniJinja template rendering to a template string with variable substitution.
// ---------------------------------------------------------------------------
// One-shot template rendering
// ---------------------------------------------------------------------------
//
// `apply_template` parses, compiles, and renders a template in a single call.
// This is convenient for templates that are only rendered once per document
// (page templates, metadata blocks) or for dynamically-resolved templates
// whose source isn't known at init time (div/span plugin overrides).
//
// For templates that are rendered many times per document (code chunks,
// figures, divs, theorems), use `TemplateEnv` below to parse once and
// render many times.

#[inline(never)]
pub fn apply_template(template: &str, vars: &HashMap<String, String>) -> String {
    let mut env = minijinja::Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);
    if let Err(e) = env.add_template("__tpl__", template) {
        cwarn!("template parse error: {}", e);
        return template.to_string();
    }
    let ctx = build_jinja_context(vars);
    let tpl = env.get_template("__tpl__").unwrap();
    match tpl.render(minijinja::Value::from_serialize(&ctx)) {
        Ok(rendered) => rendered,
        Err(e) => {
            cwarn!("template error: {}", e);
            template.to_string()
        }
    }
}

// ---------------------------------------------------------------------------
// Pre-compiled template environment
// ---------------------------------------------------------------------------
//
// Element templates (code_source, code_output, div, figure, theorem, etc.)
// are rendered once per element -- potentially hundreds of times in a single
// document. Parsing and compiling the template on every call adds ~3 us of
// overhead each time. TemplateEnv pays the parse/compile cost once at init,
// then each render() call only executes the pre-compiled template (~0.8 us).
//
// Callers add templates by name at construction time, then call render()
// on the hot path with just a name + vars map.

pub struct TemplateEnv {
    env: minijinja::Environment<'static>,
    /// Template sources must outlive the environment. We keep owned copies
    /// here so the `'static` lifetime bound on Environment is satisfied
    /// (sources are leaked into &'static str on insertion).
    _sources: Vec<String>,
}

impl TemplateEnv {
    pub fn new() -> Self {
        let mut env = minijinja::Environment::new();
        env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);
        Self { env, _sources: Vec::new() }
    }

    /// Add a named template. The source string is leaked to satisfy
    /// minijinja's `'static` lifetime requirement on template sources.
    /// This is fine because TemplateEnv lives for the duration of a
    /// single document render.
    pub fn add(&mut self, name: &'static str, source: String) {
        // Leak the source string so it lives as long as the environment.
        let leaked: &'static str = Box::leak(source.into_boxed_str());
        // We don't reclaim these -- the allocations are small (a few KB
        // total across all element templates) and freed when the process
        // exits or the document render completes.
        if let Err(e) = self.env.add_template(name, leaked) {
            cwarn!("template compile error for '{}': {}", name, e);
        }
    }

    /// Render a pre-compiled template by name. Returns empty string if
    /// the template was never added.
    pub fn render(&self, name: &str, vars: &HashMap<String, String>) -> String {
        let tpl = match self.env.get_template(name) {
            Ok(t) => t,
            Err(_) => return String::new(),
        };
        let ctx = build_jinja_context(vars);
        match tpl.render(minijinja::Value::from_serialize(&ctx)) {
            Ok(rendered) => rendered,
            Err(e) => {
                cwarn!("template render error for '{}': {}", name, e);
                String::new()
            }
        }
    }
}

/// Render a metadata field through an element template if available.
/// Returns empty string if no template is found.
pub fn render_element_block(name: &str, ext: &str, vars: &HashMap<String, String>) -> String {
    use crate::render::elements::resolve_element_template;
    if let Some(tpl) = resolve_element_template(name, ext) {
        let mut vars = vars.clone();
        vars.insert("base".to_string(), ext.to_string());
        apply_template(&tpl, &vars)
    } else {
        String::new()
    }
}

/// Build page template variables from metadata and rendered body.
/// Shared across all output formats; format-specific blocks are rendered
/// through overridable element templates.
pub fn build_template_vars(meta: &Metadata, body: &str, ext: &str) -> HashMap<String, String> {
    build_template_vars_with_headings(meta, body, ext, &[])
}

/// Build page template variables with pre-collected heading metadata for TOC.
pub fn build_template_vars_with_headings(
    meta: &Metadata,
    body: &str,
    ext: &str,
    headings: &[crate::render::ast::TocEntry],
) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    vars.insert("body".to_string(), body.to_string());
    vars.insert(
        "generator".to_string(),
        format!("calepin {}", env!("CARGO_PKG_VERSION")),
    );
    vars.insert("preamble".to_string(), String::new());

    // `base` = rendering engine (html, latex, typst, markdown)
    // `target` = named output profile (defaults to base when no target specified)
    vars.insert("base".to_string(), ext.to_string());
    vars.insert("target".to_string(), ext.to_string());

    // Plain title (used in <title> etc.) — strip markdown image/link syntax
    let plain_title = meta.title.as_deref().unwrap_or("Untitled");
    let plain_title = strip_markdown_formatting(plain_title);
    vars.insert("plain_title".to_string(), plain_title);
    vars.insert("title".to_string(),
        meta.title.as_deref()
            .map(|t| crate::render::markdown::render_inline(t, ext))
            .unwrap_or_default(),
    );
    vars.insert(
        "author".to_string(),
        meta.author.as_ref()
            .map(|a| a.iter()
                .map(|name| crate::render::markdown::render_inline(name, ext))
                .collect::<Vec<_>>()
                .join(", "))
            .unwrap_or_default(),
    );
    vars.insert("date".to_string(), meta.date.clone().unwrap_or_default());

    // Title command (LaTeX preamble)
    if let Some(ref title) = meta.title {
        let rendered_title = crate::render::markdown::render_inline(title, ext);
        vars.insert("title_cmd".to_string(), format!("\\title{{{}}}", rendered_title));
    }

    // Subtitle (already available as {{subtitle}} via vars set above)
    if let Some(ref subtitle) = meta.subtitle {
        vars.insert("subtitle".to_string(), crate::render::markdown::render_inline(subtitle, ext));
    }

    // Author block
    vars.insert("author_block".to_string(), build_author_block(meta, ext));

    // Date command (LaTeX preamble)
    if let Some(ref date) = meta.date {
        vars.insert("date_cmd".to_string(), format!("\\date{{{}}}", date));
    }

    // Abstract block
    if let Some(ref abs) = meta.abstract_text {
        vars.insert("abstract".to_string(), crate::render::markdown::render_inline(abs, ext));
    } else {
        vars.insert("abstract".to_string(), String::new());
    }

    // Keywords
    if !meta.keywords.is_empty() {
        let joined = meta.keywords.join(", ");
        vars.insert("keywords".to_string(), joined.clone());

        // HTML meta tag for keywords
        if ext == "html" {
            vars.insert(
                "preamble".to_string(),
                format!(
                    "{}<meta name=\"keywords\" content=\"{}\">",
                    vars.get("preamble").cloned().unwrap_or_default(),
                    joined
                ),
            );
        }
    }

    // Appendix
    vars.insert("appendix_block".to_string(), build_appendix(meta, ext));

    // CSS (HTML only)
    if ext == "html" {
        vars.insert("css".to_string(), format!("<style>\n{}\n</style>", load_default_css()));
        vars.insert("js".to_string(), String::new());
        let mut math_vars = HashMap::new();
        math_vars.insert("html_math_method".to_string(),
            meta.html_math_method.as_deref().unwrap_or("katex").to_string());
        vars.insert("math_block".to_string(),
            render_element_block("math", ext, &math_vars));
    }

    // LaTeX-specific defaults
    if ext == "latex" {
        vars.insert("bib_preamble".to_string(), String::new());
        vars.insert("bib_end".to_string(), String::new());
    }

    // Bibliography block (format-specific via element template)
    if !meta.bibliography.is_empty() {
        let bib_path = &meta.bibliography[0];
        let mut bvars = HashMap::new();
        bvars.insert("path".to_string(), bib_path.clone());
        vars.insert("bibliography".to_string(),
            render_element_block("bibliography", ext, &bvars));
    }

    // Table of contents (defaults to true for HTML)
    let toc_enabled = match meta.toc {
        Some(v) => v,
        None => ext == "html",
    };
    if toc_enabled {
        let toc_depth = if meta.toc_depth == 0 { 3 } else { meta.toc_depth };
        let toc_title = meta.toc_title.as_deref().unwrap_or("Contents");
        let toc = match ext {
            "html" => build_toc_html(headings, toc_depth, toc_title),
            "latex" => format!("\\setcounter{{tocdepth}}{{{}}}\n\\tableofcontents", toc_depth),
            "typst" => format!("#outline(depth: {})", toc_depth),
            _ => String::new(),
        };
        vars.insert("toc".to_string(), toc);
    } else {
        vars.insert("toc".to_string(), String::new());
    }


    // Extra YAML fields override defaults (e.g., classoption, documentclass)
    for (key, value) in &meta.var {
        let s = if let Some(s) = value.as_str() {
            s.to_string()
        } else if let Some(b) = value.as_bool() {
            b.to_string()
        } else if let Some(n) = value.as_integer() {
            n.to_string()
        } else if let Some(f) = value.as_floating_point() {
            f.to_string()
        } else {
            continue
        };
        vars.insert(key.clone(), s);
    }

    vars
}




/// Render a page template with {% include %} support.
///
/// Sets up a MiniJinja environment with:
///   1. templates/{target}/ (target-specific, from active target)
///   2. templates/{base}/ (base-specific)
///   3. templates/common/ (format-agnostic .jinja)
///   4. Built-in templates/common/ (embedded in binary)
///
/// The page template and all included component templates share the same
/// context, so `{% include "preamble.jinja" %}` in the page template can
/// access all variables (base, title, author, body, etc.).
pub fn render_page_template(
    page_template: &str,
    vars: &HashMap<String, String>,
    base: &str,
) -> String {
    let mut env = minijinja::Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);

    let root = std::path::Path::new(".");
    let active_target = crate::paths::get_active_target();

    // Load templates from filesystem directories
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Some(ref target) = active_target {
        if target != base {
            dirs.push(root.join("templates").join(target));
        }
    }
    dirs.push(root.join("templates").join(base));
    dirs.push(root.join("templates").join("common"));

    for dir in &dirs {
        if !dir.is_dir() { continue; }
        let pattern = dir.join("**").join("*.*");
        let pattern_str = pattern.display().to_string();
        for entry in glob::glob(&pattern_str).unwrap_or_else(|_| glob::glob("").unwrap()) {
            if let Ok(path) = entry {
                if !path.is_file() { continue; }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let rel = path.strip_prefix(dir).unwrap_or(&path);
                    let name = rel.display().to_string();
                    let content: &'static str = Box::leak(content.into_boxed_str());
                    let name: &'static str = Box::leak(name.into_boxed_str());
                    if env.get_template(name).is_err() {
                        let _ = env.add_template(name, content);
                    }
                }
            }
        }
    }

    // Load built-in common templates as fallback
    if let Some(common_dir) = crate::render::elements::BUILTIN_PROJECT.get_dir("templates/common") {
        for entry in common_dir.files() {
            if let Some(content) = entry.contents_utf8() {
                let name = entry.path().file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                if !name.is_empty() && env.get_template(name).is_err() {
                    let content: &'static str = Box::leak(content.to_string().into_boxed_str());
                    let name: &'static str = Box::leak(name.to_string().into_boxed_str());
                    let _ = env.add_template(name, content);
                }
            }
        }
    }

    // Add the page template itself
    if let Err(e) = env.add_template("__page__", page_template) {
        cwarn!("page template parse error: {}", e);
        return page_template.to_string();
    }

    let ctx = build_jinja_context(vars);
    let tpl = env.get_template("__page__").unwrap();
    match tpl.render(minijinja::Value::from_serialize(&ctx)) {
        Ok(rendered) => rendered,
        Err(e) => {
            cwarn!("page template render error: {}", e);
            page_template.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_template() {
        let template = "<title>{{title}}</title>\n<body>{{body}}</body>";
        let mut vars = HashMap::new();
        vars.insert("title".to_string(), "Hello".to_string());
        vars.insert("body".to_string(), "<p>World</p>".to_string());
        let result = apply_template(template, &vars);
        assert_eq!(result, "<title>Hello</title>\n<body><p>World</p></body>");
    }

    #[test]
    fn test_missing_vars_become_empty() {
        let template = "{{title}}: {{missing}}";
        let mut vars = HashMap::new();
        vars.insert("title".to_string(), "Hello".to_string());
        let result = apply_template(template, &vars);
        assert_eq!(result, "Hello: ");
    }
}
