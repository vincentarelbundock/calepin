use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::types::Metadata;

/// Build an HTML table of contents from heading metadata collected during the AST walk.
pub fn build_html_toc(headings: &[crate::render::ast::TocEntry], depth: u8, title: &str) -> String {
    let items: Vec<(u8, &str, &str)> = headings.iter()
        .filter(|h| h.level <= depth)
        .filter(|h| !h.classes.iter().any(|c| c == "unlisted"))
        .filter(|h| !h.text.is_empty())
        .map(|h| (h.level, h.id.as_str(), h.text.as_str()))
        .collect();
    build_html_toc_from_items(&items, title)
}

/// Build an HTML table of contents by extracting headings from rendered HTML (fallback).
pub fn build_html_toc_from_body(body: &str, depth: u8, title: &str) -> String {
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
    build_html_toc_from_items(&items, title)
}

fn build_html_toc_from_items(items: &[(u8, &str, &str)], title: &str) -> String {
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

/// Strip markdown image/link syntax to produce plain text for <title> etc.
/// `![alt](url)` → `alt`, `[text](url)` → `text`.
/// For images with no alt text, extracts the filename stem from the URL.
fn strip_markdown_formatting(text: &str) -> String {
    static RE_IMG: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"!\[([^\]]*)\]\(([^)]*)\)(\{[^}]*\})?").unwrap()
    });
    static RE_LINK: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\[([^\]]*)\]\([^)]*\)(\{[^}]*\})?").unwrap()
    });
    let result = RE_IMG.replace_all(text, |caps: &regex::Captures| {
        let alt = caps.get(1).map_or("", |m| m.as_str());
        if !alt.is_empty() {
            return alt.to_string();
        }
        let url = caps.get(2).map_or("", |m| m.as_str());
        std::path::Path::new(url)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string()
    });
    let result = RE_LINK.replace_all(&result, "$1");
    result.trim().to_string()
}

/// Load a page template by name and base.
///
/// Resolution order:
///   1. Project filesystem (templates/{base}/ or templates/common/)
///   2. User ~/.config/calepin/templates/
///   3. Built-in (discovered from embedded project tree)
pub fn load_page_template_for_base(template_name: &str, base: &str) -> String {
    // Filesystem resolution
    if let Some(path) = crate::paths::resolve_template(template_name, base) {
        if let Ok(s) = std::fs::read_to_string(&path) {
            return s;
        }
    }
    // Built-in: discovered from embedded project tree
    crate::render::elements::builtin_template(template_name, base)
        .unwrap_or("")
        .to_string()
}

/// Legacy entry point: accepts a filename like "calepin.html".
pub fn load_page_template(filename: &str) -> String {
    if let Some(dot) = filename.rfind('.') {
        let name = &filename[..dot];
        let ext = &filename[dot + 1..];
        let base = crate::formats::format_from_extension(ext);
        let result = load_page_template_for_base(name, base);
        if !result.is_empty() {
            return result;
        }
    }
    String::new()
}

pub fn html_template() -> String { load_page_template_for_base("page", "html") }
pub fn latex_template() -> String { load_page_template_for_base("page", "latex") }
pub fn typst_template() -> String { load_page_template_for_base("page", "typst") }
pub fn default_css() -> String {
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
    let mut ctx = std::collections::BTreeMap::new();
    for (key, value) in vars {
        ctx.insert(key.as_str(), minijinja::Value::from(value.as_str()));
    }
    // Literal brace helpers:
    // use {{_lb}} and {{_rb}} when LaTeX braces collide with Jinja delimiters.
    ctx.insert("_lb", minijinja::Value::from("{"));
    ctx.insert("_rb", minijinja::Value::from("}"));
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
        let mut ctx = std::collections::BTreeMap::new();
        for (key, value) in vars {
            ctx.insert(key.as_str(), minijinja::Value::from(value.as_str()));
        }
        ctx.insert("_lb", minijinja::Value::from("{"));
        ctx.insert("_rb", minijinja::Value::from("}"));
        match tpl.render(minijinja::Value::from_serialize(&ctx)) {
            Ok(rendered) => rendered,
            Err(e) => {
                cwarn!("template render error for '{}': {}", name, e);
                String::new()
            }
        }
    }
}

/// Render a metadata field through an element template if available,
/// falling back to the provided default.
fn render_block(name: &str, ext: &str, vars: &HashMap<String, String>, fallback: &str) -> String {
    use crate::render::elements::resolve_element_template;
    if let Some(tpl) = resolve_element_template(name, ext) {
        let mut vars = vars.clone();
        vars.insert("base".to_string(), ext.to_string());
        apply_template(&tpl, &vars)
    } else {
        fallback.to_string()
    }
}

/// Public wrapper for rendering an element block template with variables.
pub fn render_element_block(name: &str, ext: &str, vars: &HashMap<String, String>) -> String {
    render_block(name, ext, vars, "")
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

    // Title block
    if let Some(ref title) = meta.title {
        let rendered_title = crate::render::markdown::render_inline(title, ext);
        let mut bvars = HashMap::new();
        bvars.insert("title".to_string(), rendered_title.clone());
        bvars.insert("title_cmd".to_string(), format!("\\title{{{}}}", rendered_title));
        vars.insert("title_block".to_string(), render_block("title", ext, &bvars, &rendered_title));
    } else {
        // LaTeX requires \title{} even if empty (for \maketitle)
        vars.insert("title_block".to_string(), match ext {
            "latex" => "\\title{}".to_string(),
            _ => String::new(),
        });
    }

    // Subtitle block (HTML only by default, but templates can provide others)
    if let Some(ref subtitle) = meta.subtitle {
        let rendered_subtitle = crate::render::markdown::render_inline(subtitle, ext);
        let mut bvars = HashMap::new();
        bvars.insert("subtitle".to_string(), rendered_subtitle.clone());
        vars.insert("subtitle_block".to_string(), render_block("subtitle", ext, &bvars, &rendered_subtitle));
    } else {
        vars.insert("subtitle_block".to_string(), String::new());
    }

    // Author block
    vars.insert("author_block".to_string(), build_author_block(meta, ext));

    // Date block
    if let Some(ref date) = meta.date {
        let mut bvars = HashMap::new();
        bvars.insert("date".to_string(), date.clone());
        bvars.insert("date_cmd".to_string(), format!("\\date{{{}}}", date));
        let fallback = match ext {
            "latex" => format!("\\date{{{}}}", date),
            _ => date.clone(),
        };
        vars.insert("date_block".to_string(), render_block("date", ext, &bvars, &fallback));
    } else {
        // LaTeX needs an empty \date{} to suppress "today"
        vars.insert("date_block".to_string(), match ext {
            "latex" => "\\date{}".to_string(),
            _ => String::new(),
        });
    }

    // Abstract block
    if let Some(ref abs) = meta.abstract_text {
        let rendered_abs = crate::render::markdown::render_inline(abs, ext);
        let mut bvars = HashMap::new();
        bvars.insert("abstract".to_string(), rendered_abs.clone());
        vars.insert("abstract".to_string(), render_block("abstract", ext, &bvars, &rendered_abs));
    } else {
        vars.insert("abstract".to_string(), String::new());
    }

    // Keywords block
    if !meta.keywords.is_empty() {
        let joined = meta.keywords.join(", ");
        let mut bvars = HashMap::new();
        bvars.insert("keywords".to_string(), joined.clone());
        vars.insert("keywords_block".to_string(), render_block("keywords", ext, &bvars, &joined));

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
    } else {
        vars.insert("keywords_block".to_string(), String::new());
    }

    // Appendix
    vars.insert("appendix_block".to_string(), build_appendix(meta, ext));

    // CSS (HTML only)
    if ext == "html" {
        vars.insert("css".to_string(), format!("<style>\n{}\n</style>", default_css()));
        vars.insert("js".to_string(), String::new());
        let mut math_vars = HashMap::new();
        math_vars.insert("html_math_method".to_string(),
            meta.html_math_method.as_deref().unwrap_or("katex").to_string());
        vars.insert("math_block".to_string(),
            render_block("math", ext, &math_vars, ""));
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
            render_block("bibliography", ext, &bvars, ""));
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
            "html" => build_html_toc(headings, toc_depth, toc_title),
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

/// Convenience: build HTML template variables.
pub fn build_html_vars(meta: &Metadata, body: &str) -> HashMap<String, String> {
    build_template_vars(meta, body, "html")
}

/// Build HTML template variables with pre-collected heading metadata.
pub fn build_html_vars_with_headings(
    meta: &Metadata,
    body: &str,
    headings: &[crate::render::ast::TocEntry],
) -> HashMap<String, String> {
    build_template_vars_with_headings(meta, body, "html", headings)
}

/// Build the appendix block for any format using element templates.
/// Format-specific content (links, lists) is computed in Rust;
/// structural markup comes from overridable templates.
fn build_appendix(meta: &Metadata, ext: &str) -> String {
    use crate::render::elements::resolve_element_template;

    let style = meta.appendix_style.as_deref().unwrap_or("default");
    if style == "none" {
        return String::new();
    }

    let mut sections: Vec<String> = Vec::new();
    let fmt = ext.to_string();

    // License
    if let Some(ref lic) = meta.license {
        if let Some(ref text) = lic.text {
            let content = format_link(text, lic.url.as_deref(), ext);
            if let Some(tpl) = resolve_element_template("license", ext) {
                let mut vars = HashMap::new();
                vars.insert("base".to_string(), fmt.clone());
                vars.insert("content".to_string(), content);
                sections.push(apply_template(&tpl, &vars));
            }
        }
    }

    // Citation
    if let Some(ref cite) = meta.citation {
        let content = build_citation_text(meta, cite, ext);
        if let Some(tpl) = resolve_element_template("citation", ext) {
            let mut vars = HashMap::new();
            vars.insert("base".to_string(), fmt.clone());
            vars.insert("content".to_string(), content);
            sections.push(apply_template(&tpl, &vars));
        }
    }

    // Copyright
    if let Some(ref cr) = meta.copyright {
        let text = build_copyright_text(cr);
        if !text.is_empty() {
            if let Some(tpl) = resolve_element_template("copyright", ext) {
                let mut vars = HashMap::new();
                vars.insert("base".to_string(), fmt.clone());
                vars.insert("content".to_string(), text);
                sections.push(apply_template(&tpl, &vars));
            }
        }
    }

    // Funding
    if !meta.funding.is_empty() {
        let items = build_funding_items(&meta.funding, ext);
        if !items.is_empty() {
            if let Some(tpl) = resolve_element_template("funding", ext) {
                let mut vars = HashMap::new();
                vars.insert("base".to_string(), fmt.clone());
                vars.insert("items".to_string(), items);
                sections.push(apply_template(&tpl, &vars));
            }
        }
    }

    if sections.is_empty() {
        String::new()
    } else if let Some(tpl) = resolve_element_template("appendix", ext) {
        let mut vars = HashMap::new();
        vars.insert("base".to_string(), fmt);
        vars.insert("sections".to_string(), sections.join("\n"));
        apply_template(&tpl, &vars)
    } else {
        sections.join("\n\n")
    }
}

/// Format a text+url as a link in the appropriate output format.
fn format_link(text: &str, url: Option<&str>, ext: &str) -> String {
    match url {
        Some(url) => match ext {
            "html" => format!("<a href=\"{}\">{}</a>", url, text),
            "latex" => format!("{} (\\url{{{}}})", text, url),
            "typst" => format!("#link(\"{}\")[{}]", url, text),
            _ => format!("[{}]({})", text, url),
        },
        None => text.to_string(),
    }
}

/// Build the copyright text from a Copyright struct.
fn build_copyright_text(cr: &crate::types::Copyright) -> String {
    if let Some(ref stmt) = cr.statement {
        stmt.clone()
    } else {
        let holder = cr.holder.as_deref().unwrap_or("");
        let year = cr.year.as_deref().unwrap_or("");
        match (holder.is_empty(), year.is_empty()) {
            (false, false) => format!("Copyright {} {}", year, holder),
            (false, true) => format!("Copyright {}", holder),
            (true, false) => format!("Copyright {}", year),
            (true, true) => String::new(),
        }
    }
}

/// Build the citation text from metadata and citation info.
fn build_citation_text(meta: &Metadata, cite: &crate::types::CitationMeta, ext: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(ref a) = meta.author {
        parts.push(a.join(", "));
    }
    if let Some(ref t) = meta.title {
        parts.push(format!("\"{}\"", t));
    }
    if let Some(ref ct) = cite.container_title {
        match ext {
            "html" => parts.push(format!("<em>{}</em>", ct)),
            "latex" => parts.push(format!("\\emph{{{}}}", ct)),
            "typst" => parts.push(format!("#emph[{}]", ct)),
            _ => parts.push(format!("*{}*", ct)),
        }
    }
    if let Some(ref vol) = cite.volume {
        let vol_str = if let Some(ref iss) = cite.issue {
            format!("{}({})", vol, iss)
        } else {
            vol.clone()
        };
        parts.push(vol_str);
    }
    if let Some(ref pg) = cite.page {
        parts.push(format!("pp. {}", pg));
    }
    if let Some(ref issued) = cite.issued {
        parts.push(format!("({})", issued));
    }
    let mut citation = parts.join(". ");
    if !citation.is_empty() {
        citation.push('.');
    }
    if let Some(ref doi) = cite.doi {
        let doi_url = if doi.starts_with("http") { doi.clone() } else { format!("https://doi.org/{}", doi) };
        match ext {
            "html" => citation.push_str(&format!(" DOI: <a href=\"{}\">{}</a>", doi_url, doi)),
            "latex" => citation.push_str(&format!(" DOI: \\url{{{}}}", doi_url)),
            "typst" => citation.push_str(&format!(" DOI: #link(\"{}\")[{}]", doi_url, doi)),
            _ => citation.push_str(&format!(" DOI: [{}]({})", doi, doi_url)),
        }
    }
    if let Some(ref url) = cite.url {
        match ext {
            "html" => citation.push_str(&format!(" URL: <a href=\"{}\">{}</a>", url, url)),
            "latex" => citation.push_str(&format!(" URL: \\url{{{}}}", url)),
            "typst" => citation.push_str(&format!(" URL: #link(\"{}\")", url)),
            _ => citation.push_str(&format!(" URL: <{}>", url)),
        }
    }
    citation
}

/// Build funding items as a pre-formatted list string.
fn build_funding_items(funding: &[crate::types::Funding], ext: &str) -> String {
    let items: Vec<String> = funding.iter().filter_map(|f| {
        if let Some(ref stmt) = f.statement {
            Some(stmt.clone())
        } else {
            let mut parts = Vec::new();
            if let Some(ref src) = f.source { parts.push(src.as_str()); }
            if let Some(ref award) = f.award { parts.push(award.as_str()); }
            if let Some(ref recip) = f.recipient { parts.push(recip.as_str()); }
            if parts.is_empty() { None } else { Some(parts.join(", ")) }
        }
    }).collect();
    if items.is_empty() {
        return String::new();
    }
    match ext {
        "html" => items.iter().map(|i| format!("<li>{}</li>", i)).collect::<Vec<_>>().join("\n"),
        "latex" => items.iter().map(|i| {
            let escaped = i.replace('#', "\\#").replace('%', "\\%").replace('&', "\\&").replace('_', "\\_");
            format!("\\item {}", escaped)
        }).collect::<Vec<_>>().join("\n"),
        _ => items.iter().map(|i| format!("- {}", i)).collect::<Vec<_>>().join("\n"),
    }
}

/// Build the author block for any format using element templates.
/// Rich metadata (affiliations, ORCID, etc.) renders through author-item and
/// affiliation-item sub-templates; simple author lists use a plain fallback.
fn build_author_block(meta: &Metadata, ext: &str) -> String {
    use crate::render::elements::resolve_element_template;

    let has_rich = !meta.authors.is_empty()
        && meta.authors.iter().any(|a| {
            !a.affiliation_ids.is_empty()
                || a.email.is_some()
                || a.orcid.is_some()
                || a.corresponding
        });

    if has_rich {
        // Render each author through the author-item template
        let author_tpl = resolve_element_template("author_item", ext);
        let authors_rendered: Vec<String> = meta.authors.iter().map(|author| {
            let superscripts = if !author.affiliation_ids.is_empty() && meta.affiliations.len() > 1 {
                let sups: Vec<String> = author.affiliation_ids.iter()
                    .filter_map(|&i| meta.affiliations.get(i).map(|a| a.number.to_string()))
                    .collect();
                match ext {
                    "html" => format!("<sup>{}</sup>", sups.join(",")),
                    "latex" => format!("\\textsuperscript{{{}}}", sups.join(",")),
                    "typst" => format!("#super[{}]", sups.join(",")),
                    _ => String::new(),
                }
            } else {
                String::new()
            };

            let corresponding = if author.corresponding {
                match ext {
                    "html" => "<sup>*</sup>".to_string(),
                    "latex" => "\\textsuperscript{*}".to_string(),
                    "typst" => "#super[\\*]".to_string(),
                    _ => String::new(),
                }
            } else {
                String::new()
            };

            let orcid_link = if let Some(ref orcid) = author.orcid {
                let url = if orcid.starts_with("http") { orcid.clone() } else { format!("https://orcid.org/{}", orcid) };
                match ext {
                    "html" => format!(
                        " <a href=\"{}\" class=\"author-orcid\" title=\"ORCID\">\
                         <svg width=\"16\" height=\"16\" viewBox=\"0 0 256 256\" fill=\"#A6CE39\">\
                         <path d=\"M256,128c0,70.7-57.3,128-128,128S0,198.7,0,128,57.3,0,128,0s128,57.3,128,128Z\"/>\
                         <path fill=\"#fff\" d=\"M86.3,186.2H70.9V79.1h15.4v107.1ZM78.6,56.8c-5.5,0-10,4.5-10,10s4.5,10,10,10,10-4.5,10-10-4.5-10-10-10Z\
                         M162.2,79.1c-24.4,0-37.2,12.8-43.6,23.6V79.1H103.2V186.2h15.4V120.8c0-12.4,14.3-27.3,33.5-27.3,20.3,0,28.3,14.5,28.3,34v58.7h15.4V125.3C195.8,99.6,184.3,79.1,162.2,79.1Z\"/>\
                         </svg></a>",
                        url
                    ),
                    _ => String::new(),
                }
            } else {
                String::new()
            };

            if let Some(ref tpl) = author_tpl {
                let mut vars = HashMap::new();
                vars.insert("base".to_string(), ext.to_string());
                vars.insert("name".to_string(), author.name.literal.clone());
                vars.insert("superscripts".to_string(), superscripts);
                vars.insert("corresponding".to_string(), corresponding);
                vars.insert("orcid_link".to_string(), orcid_link);
                apply_template(tpl, &vars)
            } else {
                format!("{}{}{}{}", author.name.literal, superscripts, corresponding, orcid_link)
            }
        }).collect();

        // Render each affiliation through the affiliation-item template
        let aff_tpl = resolve_element_template("affiliation_item", ext);
        let affs_rendered: Vec<String> = meta.affiliations.iter().filter_map(|aff| {
            let display = aff.display();
            if display.is_empty() {
                return None;
            }
            let number = if meta.affiliations.len() > 1 {
                match ext {
                    "html" => format!("<sup>{}</sup> ", aff.number),
                    "latex" => format!("\\textsuperscript{{{}}} ", aff.number),
                    "typst" => format!("#super[{}] ", aff.number),
                    _ => String::new(),
                }
            } else {
                String::new()
            };
            if let Some(ref tpl) = aff_tpl {
                let mut vars = HashMap::new();
                vars.insert("base".to_string(), ext.to_string());
                vars.insert("number".to_string(), number);
                vars.insert("display".to_string(), display);
                Some(apply_template(tpl, &vars))
            } else {
                Some(format!("{}{}", number, display))
            }
        }).collect();

        // Corresponding author note
        let corresponding_note = meta.authors.iter().filter_map(|author| {
            if author.corresponding {
                if let Some(ref email) = author.email {
                    Some(match ext {
                        "html" => format!(
                            "  <p class=\"corresponding\">* Corresponding author: <a href=\"mailto:{}\">{}</a></p>",
                            email, email
                        ),
                        "latex" => format!("\\textsuperscript{{*}} Corresponding author: \\url{{{}}}", email),
                        "typst" => format!("#super[\\*] Corresponding author: #link(\"mailto:{}\")", email),
                        _ => format!("* Corresponding author: {}", email),
                    })
                } else {
                    None
                }
            } else {
                None
            }
        }).collect::<Vec<_>>().join("\n");

        // Assemble through author-block template
        let authors_joined = match ext {
            "latex" => authors_rendered.join(" \\and "),
            "typst" => authors_rendered.join(", "),
            _ => authors_rendered.join("\n"),
        };

        let affiliations_joined = match ext {
            "html" => {
                if affs_rendered.is_empty() {
                    String::new()
                } else {
                    format!("  <div class=\"affiliations\">\n{}\n  </div>", affs_rendered.join("\n"))
                }
            }
            "latex" => {
                if affs_rendered.is_empty() {
                    String::new()
                } else {
                    format!(
                        "\\newcommand{{\\affiliationblock}}{{\\begin{{center}}\\small {}\\end{{center}}}}",
                        affs_rendered.join(" \\\\\n")
                    )
                }
            }
            "typst" => {
                if affs_rendered.is_empty() {
                    String::new()
                } else {
                    format!(
                        "\n  #v(0.3em)\n  #text(size: 9pt, style: \"italic\")[{}]",
                        affs_rendered.join(" \\\n")
                    )
                }
            }
            _ => affs_rendered.join(", "),
        };

        if let Some(tpl) = resolve_element_template("author_block", ext) {
            let mut vars = HashMap::new();
            vars.insert("base".to_string(), ext.to_string());
            vars.insert("authors_cmd".to_string(), format!("\\author{{{}}}", authors_joined));
            vars.insert("authors".to_string(), authors_joined);
            vars.insert("affiliations".to_string(), affiliations_joined);
            vars.insert("corresponding_note".to_string(), corresponding_note);
            apply_template(&tpl, &vars)
        } else {
            format!("{}\n{}\n{}", authors_joined, affiliations_joined, corresponding_note)
        }
    } else if let Some(ref authors) = meta.author {
        match ext {
            "html" => format!("<h2>{}</h2>", authors.join(", ")),
            "latex" => format!("\\author{{{}}}", authors.join(" \\and ")),
            "typst" => format!("#text(size: 12pt)[{}]", authors.join(", ")),
            _ => authors.join(", "),
        }
    } else {
        String::new()
    }
}

/// Convenience: build LaTeX template variables.
pub fn build_latex_vars(meta: &Metadata, body: &str) -> HashMap<String, String> {
    build_template_vars(meta, body, "latex")
}

/// Convenience: build Typst template variables.
pub fn build_typst_vars(meta: &Metadata, body: &str) -> HashMap<String, String> {
    build_template_vars(meta, body, "typst")
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

    // Build context
    let mut ctx = std::collections::BTreeMap::new();
    for (key, value) in vars {
        ctx.insert(key.as_str(), minijinja::Value::from(value.as_str()));
    }
    ctx.insert("_lb", minijinja::Value::from("{"));
    ctx.insert("_rb", minijinja::Value::from("}"));

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
