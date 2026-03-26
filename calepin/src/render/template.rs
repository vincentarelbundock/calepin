use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::config::Metadata;

/// Build an HTML table of contents from heading metadata collected during the AST walk.
pub fn build_toc_html(headings: &[crate::emit::TocEntry], depth: u8, title: &str) -> String {
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

/// Build just the `<ul>...</ul>` nested list for the TOC (no nav wrapper).
fn build_toc_list_html(items: &[(u8, &str, &str)]) -> String {
    // Only show TOC when there are at least 2 entries
    if items.len() < 2 { return String::new(); }

    let min_level = items.iter().map(|(l, _, _)| *l).min().unwrap_or(1);
    let mut html = String::from("<ul>\n");
    let mut current_level = min_level;
    let mut first = true;

    for (level, id, text) in items {
        if *level > current_level {
            while current_level < *level {
                html.push_str("\n<ul>\n");
                current_level += 1;
            }
        } else {
            if !first {
                html.push_str("</li>\n");
            }
            while current_level > *level {
                html.push_str("</ul>\n</li>\n");
                current_level -= 1;
            }
        }
        html.push_str(&format!("<li><a href=\"#{}\">{}</a>", id, text));
        first = false;
    }

    if !first {
        html.push_str("</li>\n");
    }
    while current_level > min_level {
        html.push_str("</ul>\n</li>\n");
        current_level -= 1;
    }

    html.push_str("</ul>");
    html
}

fn build_toc_html_from_items(items: &[(u8, &str, &str)], title: &str) -> String {
    let toc_list = build_toc_list_html(items);
    if toc_list.is_empty() { return String::new(); }
    let mut vars = HashMap::new();
    vars.insert("base".to_string(), "html".to_string());
    vars.insert("engine".to_string(), "html".to_string());
    vars.insert("title".to_string(), title.to_string());
    vars.insert("toc_list".to_string(), toc_list);
    vars.insert("depth".to_string(), String::new());
    let tpl = include_str!("../partials/html/toc.html");
    apply_template(tpl, &vars)
}

use crate::render::metadata::{strip_markdown_formatting, build_appendix, build_authors};

/// Load a page template by name and base.
///
/// Resolution order:
///   1. Project filesystem (partials/{target}/, partials/{base}/, or templates/common/)
///   2. Built-in (discovered from embedded project tree)
pub fn load_page_template(template_name: &str, base: &str) -> String {
    // Filesystem resolution
    if let Some(path) = crate::paths::resolve_partial(template_name, base) {
        if let Ok(s) = std::fs::read_to_string(&path) {
            return s;
        }
    }
    // Built-in: discovered from embedded project tree
    crate::render::elements::resolve_builtin_partial(template_name, base)
        .unwrap_or("")
        .to_string()
}


pub fn load_default_css() -> String {
    // Check project/user overrides for CSS
    let root = crate::paths::get_project_root();
    let p = crate::paths::partials_dir(&root).join("html").join("page.css");
    if p.exists() {
        if let Ok(s) = std::fs::read_to_string(&p) {
            return s;
        }
    }
    // Built-in: discovered from embedded project tree
    crate::render::elements::BUILTIN_PARTIALS
        .get_file("html/page.css")
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
    sources: std::sync::Arc<std::sync::Mutex<HashMap<String, String>>>,
}

impl TemplateEnv {
    pub fn new() -> Self {
        let sources = std::sync::Arc::new(std::sync::Mutex::new(HashMap::new()));
        let mut env = minijinja::Environment::new();
        env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);
        let src = std::sync::Arc::clone(&sources);
        env.set_loader(move |name: &str| {
            Ok(src.lock().unwrap().get(name).cloned())
        });
        Self { env, sources }
    }

    /// Add a named template. Sources are owned by the loader and compiled
    /// on first access by minijinja (which caches the result internally).
    pub fn add(&mut self, name: &str, source: String) {
        self.sources.lock().unwrap().insert(name.to_string(), source);
    }

    /// Add a template at render time (when only &self is available).
    /// The loader will find it on the next `get_template` call (MiniJinja
    /// calls the loader on cache miss, so newly-added sources are picked up).
    pub fn add_dynamic(&self, name: &str, source: String) {
        self.sources.lock().unwrap().insert(name.to_string(), source);
    }

    /// Render a template by name, loading it dynamically if not already present.
    /// Falls back to `apply_template` for one-off templates not in the env.
    pub fn render_dynamic(&self, name: &str, template_source: &str, vars: &HashMap<String, String>) -> String {
        // Add the template if not already loaded
        {
            let sources = self.sources.lock().unwrap();
            if !sources.contains_key(name) {
                drop(sources);
                self.add_dynamic(name, template_source.to_string());
            }
        }
        self.render(name, vars)
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
pub fn render_element(name: &str, ext: &str, vars: &HashMap<String, String>) -> String {
    use crate::render::elements::resolve_element_partial;
    if let Some(tpl) = resolve_element_partial(name, ext) {
        let mut vars = vars.clone();
        vars.insert("base".to_string(), ext.to_string());
        vars.insert("engine".to_string(), ext.to_string());
        apply_template(&tpl, &vars)
    } else {
        String::new()
    }
}

/// Build page template variables from metadata and rendered body.
/// Build page template variables with pre-collected heading metadata for TOC.
pub fn build_template_vars_with_headings(
    meta: &Metadata,
    body: &str,
    ext: &str,
    headings: &[crate::emit::TocEntry],
    _target: Option<&crate::project::Target>,
) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    let defs = meta;

    vars.insert("body".to_string(), body.to_string());
    vars.insert(
        "generator".to_string(),
        format!("calepin {}", env!("CARGO_PKG_VERSION")),
    );
    vars.insert("preamble".to_string(), String::new());

    // `base` = rendering engine (html, latex, typst, markdown)
    // `target` = named output profile (defaults to base when no target specified)
    vars.insert("base".to_string(), ext.to_string());
    vars.insert("engine".to_string(), ext.to_string());
    vars.insert("target".to_string(), ext.to_string());

    // Language
    vars.insert("lang".to_string(), defs.lang.as_deref().unwrap_or("en").to_string());

    // Labels (localisable strings)
    let labels = defs.labels.as_ref();
    vars.insert("label_abstract".to_string(), labels.and_then(|l| l.abstract_title.clone()).unwrap_or_else(|| "Abstract".to_string()));
    vars.insert("label_keywords".to_string(), labels.and_then(|l| l.keywords.clone()).unwrap_or_else(|| "Keywords".to_string()));
    vars.insert("label_appendix".to_string(), labels.and_then(|l| l.appendix.clone()).unwrap_or_else(|| "Appendix".to_string()));
    vars.insert("label_citation".to_string(), labels.and_then(|l| l.citation.clone()).unwrap_or_else(|| "Citation".to_string()));
    vars.insert("label_reuse".to_string(), labels.and_then(|l| l.reuse.clone()).unwrap_or_else(|| "Reuse".to_string()));
    vars.insert("label_funding".to_string(), labels.and_then(|l| l.funding.clone()).unwrap_or_else(|| "Funding".to_string()));
    vars.insert("label_copyright".to_string(), labels.and_then(|l| l.copyright.clone()).unwrap_or_else(|| "Copyright".to_string()));
    vars.insert("label_listing".to_string(), labels.and_then(|l| l.listing.clone()).unwrap_or_else(|| "Listing".to_string()));
    vars.insert("label_proof".to_string(), labels.and_then(|l| l.proof.clone()).unwrap_or_else(|| "Proof".to_string()));
    vars.insert("label_contents".to_string(), labels.and_then(|l| l.contents.clone()).unwrap_or_else(|| "Contents".to_string()));

    // Plain title (used in <title> etc.) — strip markdown image/link syntax
    let plain_title = meta.title.as_deref().unwrap_or("Untitled");
    let plain_title = strip_markdown_formatting(plain_title);
    vars.insert("plain_title".to_string(), plain_title);
    vars.insert("title".to_string(),
        meta.title.as_deref()
            .map(|t| crate::render::convert::render_inline(t, ext))
            .unwrap_or_default(),
    );
    {
        let names = meta.author_names();
        vars.insert(
            "author".to_string(),
            if names.is_empty() {
                String::new()
            } else {
                names.iter()
                    .map(|name| crate::render::convert::render_inline(name, ext))
                    .collect::<Vec<_>>()
                    .join(", ")
            },
        );
    }
    vars.insert("date".to_string(), meta.date.clone().unwrap_or_default());


    // Subtitle (already available as {{subtitle}} via vars set above)
    if let Some(ref subtitle) = meta.subtitle {
        vars.insert("subtitle".to_string(), crate::render::convert::render_inline(subtitle, ext));
    }

    // Author block
    vars.insert("authors".to_string(), build_authors(meta, ext));


    // Abstract block
    if let Some(ref abs) = meta.abstract_text {
        vars.insert("abstract".to_string(), crate::render::convert::render_inline(abs, ext));
    } else {
        vars.insert("abstract".to_string(), String::new());
    }

    // Keywords
    if !meta.keywords.is_empty() {
        let joined = meta.keywords.join(", ");
        vars.insert("keywords".to_string(), joined);
    }

    // Appendix
    vars.insert("appendix".to_string(), build_appendix(meta, ext));

    // Default values for format-specific template variables.
    vars.insert("css".to_string(), load_default_css());
    vars.insert("js".to_string(), String::new());
    vars.insert("bib_preamble".to_string(), String::new());
    vars.insert("bib_end".to_string(), String::new());
    vars.insert("colors".to_string(), String::new());

    // Math include for html-engine targets
    if ext == "html" {
        let mut math_vars = HashMap::new();
        math_vars.insert("html_math_method".to_string(),
            meta.html_math_method.as_deref()
                .unwrap_or_else(|| defs.math.as_deref().unwrap_or("katex")).to_string());
        vars.insert("math".to_string(), render_element("math", ext, &math_vars));
    } else {
        vars.insert("math".to_string(), String::new());
    }

    // Bibliography block (format-specific via element template)
    if !meta.bibliography.is_empty() {
        let bib_path = &meta.bibliography[0];
        let mut bvars = HashMap::new();
        bvars.insert("path".to_string(), bib_path.clone());
        vars.insert("bibliography".to_string(),
            render_element("bibliography", ext, &bvars));
    }

    // Table of contents
    let toc_cfg = meta.toc.as_ref();
    let toc_enabled = toc_cfg.and_then(|t| t.enabled).unwrap_or(ext == "html");
    if toc_enabled {
        let toc_depth = toc_cfg.and_then(|t| t.depth).unwrap_or(3) as u8;
        let toc_title = toc_cfg.and_then(|t| t.title.as_deref()).unwrap_or("Contents");
        let toc = if ext == "html" {
            // HTML: build nested list in Rust, wrap with template
            build_toc_html(headings, toc_depth, toc_title)
        } else {
            // LaTeX, Typst, others: use the toc template directly
            let mut toc_vars = HashMap::new();
            toc_vars.insert("base".to_string(), ext.to_string());
            toc_vars.insert("engine".to_string(), ext.to_string());
            toc_vars.insert("title".to_string(), toc_title.to_string());
            toc_vars.insert("depth".to_string(), toc_depth.to_string());
            toc_vars.insert("toc_list".to_string(), String::new());
            let tpl = crate::render::elements::resolve_builtin_partial("toc", ext).unwrap_or("");
            apply_template(tpl, &toc_vars)
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

/// Deduplicate preamble lines, preserving first-occurrence order.
/// Each entry in `lines` may contain multiple newline-separated lines;
/// deduplication is per-line so identical `\usepackage` entries from
/// different chunks appear only once.
pub fn deduplicate_preamble(lines: &[String]) -> String {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for chunk in lines {
        for line in chunk.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && seen.insert(trimmed.to_string()) {
                result.push(trimmed);
            }
        }
    }
    result.join("\n")
}

/// Inject deduplicated preamble content into template variables.
///
/// Preamble lines from code chunks (e.g., `\usepackage{...}` for LaTeX,
/// `<link>` tags for HTML) are deduplicated and merged into the `preamble`
/// template variable.
pub fn inject_preamble(vars: &mut HashMap<String, String>, preamble: &[String]) {
    let content = deduplicate_preamble(preamble);
    if !content.is_empty() {
        let entry = vars.entry("preamble".to_string()).or_default();
        if !entry.is_empty() { entry.push('\n'); }
        entry.push_str(&content);
    }
}

/// Assemble a complete page: build vars, inject preamble, customize, render.
///
/// Single entry point for page template assembly across all built-in formats.
/// The pipeline is:
///   1. Build template variables from metadata (`build_template_vars_with_headings`)
///   2. Inject deduplicated preamble
///   3. Apply format-specific customizations via the `customize` closure
///   4. Load and render the page template
///
/// Formats pre-process the body before calling this (e.g., append footnotes,
/// prepend color definitions) and post-process the rendered output after
/// (e.g., embed base64 images).
pub fn assemble_page(
    body: &str,
    meta: &Metadata,
    format: &str,
    headings: &[crate::emit::TocEntry],
    preamble: &[String],
    target: Option<&crate::project::Target>,
    customize: impl FnOnce(&mut HashMap<String, String>),
) -> String {
    let mut vars = build_template_vars_with_headings(meta, body, format, headings, target);
    inject_preamble(&mut vars, preamble);
    customize(&mut vars);
    let tpl = load_page_template("page", format);
    render_page_template(&tpl, &vars, format)
}

/// Render a page template with {% include %} support.
///
/// Sets up a MiniJinja environment with:
///   1. partials/{target}/ (target-specific, from active target)
///   2. partials/{base}/ (base-specific)
///   3. templates/common/ (format-agnostic fallback)
///   4. Built-in partials/{base}/ (embedded in binary)
///   5. Built-in templates/common/ (embedded in binary)
///
/// The page template and all included component templates share the same
/// context, so `{% include "preamble.html" %}` in the page template can
/// access all variables (base, title, author, body, etc.).
pub fn render_page_template(
    page_template: &str,
    vars: &HashMap<String, String>,
    base: &str,
) -> String {
    // Collect all template sources into an owned map, then use set_loader
    // so minijinja takes ownership -- no Box::leak needed.
    let mut templates = HashMap::new();

    let root = crate::paths::get_project_root();
    let active_target = crate::paths::get_active_target();
    let tpl_dir = crate::paths::partials_dir(&root);

    // Load templates from filesystem directories
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Some(ref target) = active_target {
        if target != base {
            dirs.push(tpl_dir.join(target));
        }
    }
    dirs.push(tpl_dir.join(base));
    dirs.push(tpl_dir.join("common"));

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
                    templates.entry(name).or_insert(content);
                }
            }
        }
    }

    // Load built-in base-specific templates as fallback
    if let Some(base_dir) = crate::render::elements::BUILTIN_PARTIALS.get_dir(base) {
        for entry in base_dir.files() {
            if let Some(content) = entry.contents_utf8() {
                let name = entry.path().file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                if !name.is_empty() {
                    templates.entry(name.to_string()).or_insert_with(|| content.to_string());
                }
            }
        }
    }

    // Load built-in common templates as fallback
    if let Some(common_dir) = crate::render::elements::BUILTIN_PARTIALS.get_dir("common") {
        for entry in common_dir.files() {
            if let Some(content) = entry.contents_utf8() {
                let name = entry.path().file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                if !name.is_empty() {
                    templates.entry(name.to_string()).or_insert_with(|| content.to_string());
                }
            }
        }
    }

    // Add the page template itself
    templates.insert("__page__".to_string(), page_template.to_string());

    let mut env = minijinja::Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);
    let sources = std::sync::Arc::new(templates);
    env.set_loader(move |name: &str| {
        Ok(sources.get(name).cloned())
    });

    let ctx = build_jinja_context(vars);
    let tpl = match env.get_template("__page__") {
        Ok(t) => t,
        Err(e) => {
            cwarn!("page template parse error: {}", e);
            return page_template.to_string();
        }
    };
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
