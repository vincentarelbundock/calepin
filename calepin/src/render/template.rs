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
    let mut vars = TemplateVars::with_writer("html");
    vars.config.insert("title".to_string(), title.to_string());
    vars.calepin.insert("toc_list".to_string(), toc_list);
    vars.config.insert("depth".to_string(), String::new());
    let tpl = include_str!("../partials/html/toc.html");
    apply_template(tpl, &vars)
}

use crate::render::metadata::{strip_markdown_formatting, build_appendix, build_authors};

/// Load a page template by name and base (layered resolution).
///
/// Checks user templates first (sidecar, then project-level), then falls
/// through to built-in templates embedded in the binary.
pub fn load_page_template(template_name: &str, base: &str) -> String {
    // Try filesystem first (sidecar → project)
    if let Some(content) = crate::paths::resolve_template(template_name, base)
        .and_then(|path| std::fs::read_to_string(&path).ok())
    {
        return content;
    }
    // Fall through to built-in
    crate::render::elements::resolve_builtin_template(template_name, base)
        .unwrap_or("")
        .to_string()
}


pub fn load_default_css() -> String {
    let mut css = String::new();

    // 1. Base CSS: user override or built-in
    let root = crate::paths::get_project_dir();
    let p = crate::paths::templates_dir(&root).join("html").join("page.css");
    if p.exists() {
        if let Ok(s) = std::fs::read_to_string(&p) {
            css.push_str(&s);
        }
    } else {
        if let Some(builtin) = crate::render::elements::BUILTIN_TEMPLATES
            .get_file("html/page.css")
            .and_then(|f| f.contents_utf8())
        {
            css.push_str(builtin);
        }
    }

    // 2. Extension CSS (active target + side-loaded extensions)
    css.push_str(&load_all_extension_assets(&root, |r, name| load_extension_css(r, name)));

    css
}

/// Load assets from the active target's extension chain plus all side-loaded extensions.
fn load_all_extension_assets(
    project_root: &std::path::Path,
    loader: impl Fn(&std::path::Path, &str) -> String,
) -> String {
    let target = crate::paths::get_active_target().unwrap_or_default();
    let mut result = loader(project_root, &target);
    for ext_name in crate::paths::get_sideloaded_extensions() {
        let ext = loader(project_root, &ext_name);
        if !ext.is_empty() {
            result.push_str(&ext);
        }
    }
    result
}

/// Walk the extension inheritance chain and collect asset file contents.
/// Returns concatenated content in parent-first order (so child overrides parent).
fn load_extension_assets(project_root: &std::path::Path, target_name: &str, get_files: impl Fn(&crate::config::extension::ExtensionAssets) -> &[String]) -> String {
    // Collect (extension_dir, file_list) in child-first order
    let chain: Vec<(std::path::PathBuf, Vec<String>)> =
        crate::config::extension::walk_chain(project_root, target_name, |_, ext_dir, manifest| {
            let files = get_files(&manifest.assets).to_vec();
            if files.is_empty() { None } else { Some((ext_dir.to_path_buf(), files)) }
        });

    // Reverse to parent-first order, load file contents
    let mut result = String::new();
    for (ext_dir, files) in chain.iter().rev() {
        let assets_base = ext_dir.join("assets");
        for file in files {
            let path = assets_base.join(file);
            if let (Ok(canonical), Ok(base)) = (path.canonicalize(), assets_base.canonicalize()) {
                if !canonical.starts_with(&base) { continue; }
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                result.push('\n');
                result.push_str(&content);
            }
        }
    }
    result
}

/// Load CSS from extensions in the active target's inheritance chain.
fn load_extension_css(project_root: &std::path::Path, target_name: &str) -> String {
    load_extension_assets(project_root, target_name, |a| &a.css)
}

/// Load JS from extensions in the active target's inheritance chain.
fn load_extension_js(project_root: &std::path::Path, target_name: &str) -> String {
    load_extension_assets(project_root, target_name, |a| &a.js)
}

/// Two-namespace template variable container.
///
/// - `config`: variables the user authored (front matter, config.toml, div/span
///   attributes, CLI `-s`). Accessed in templates as `{{ config.title }}`.
/// - `calepin`: variables the engine computed or resolved. Accessed in templates
///   as `{{ calepin.body }}`.
pub struct TemplateVars {
    pub config: HashMap<String, String>,
    pub calepin: HashMap<String, String>,
}

impl TemplateVars {
    pub fn new() -> Self {
        Self {
            config: HashMap::new(),
            calepin: HashMap::new(),
        }
    }

    /// Create a new TemplateVars with the writer already set.
    pub fn with_writer(writer: &str) -> Self {
        let mut vars = Self::new();
        vars.calepin.insert("writer".to_string(), writer.to_string());
        vars
    }
}

/// Build a MiniJinja render context from namespaced template variables.
///
/// Produces a context with two top-level objects (`config` and `calepin`).
/// When `user_vars` is provided, its entries are merged into the `config`
/// object so custom front matter keys are accessible as `{{ config.key }}`.
fn build_jinja_context(
    vars: &TemplateVars,
    user_vars: Option<&minijinja::Value>,
) -> std::collections::BTreeMap<&'static str, minijinja::Value> {
    let mut ctx = std::collections::BTreeMap::new();

    // Build config object: flat string vars + nested user vars
    let mut config_map = std::collections::BTreeMap::new();
    for (key, value) in &vars.config {
        config_map.insert(key.as_str(), minijinja::Value::from(value.as_str()));
    }
    // Merge user vars (custom front matter keys) into config
    if let Some(uv) = user_vars {
        if let Ok(iter) = uv.try_iter() {
            for key in iter {
                let key_str = key.to_string();
                // Don't overwrite explicit config entries
                if !config_map.contains_key(key_str.as_str()) {
                    if let Ok(val) = uv.get_attr(&key_str) {
                        config_map.insert(
                            Box::leak(key_str.into_boxed_str()),
                            val,
                        );
                    }
                }
            }
        }
    }
    ctx.insert("config", minijinja::Value::from_serialize(&config_map));

    // Build calepin object
    let mut calepin_map = std::collections::BTreeMap::new();
    for (key, value) in &vars.calepin {
        calepin_map.insert(key.as_str(), minijinja::Value::from(value.as_str()));
    }
    calepin_map.insert("_lb", minijinja::Value::from("{"));
    calepin_map.insert("_rb", minijinja::Value::from("}"));
    ctx.insert("calepin", minijinja::Value::from_serialize(&calepin_map));

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
pub fn apply_template(template: &str, vars: &TemplateVars) -> String {
    let mut env = minijinja::Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);
    if let Err(e) = env.add_template("__tpl__", template) {
        cwarn!("template parse error: {}", e);
        return template.to_string();
    }
    let ctx = build_jinja_context(vars, None);
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
    /// Document-level user vars (custom front matter keys), merged into `config`.
    user_vars: Option<minijinja::Value>,
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
        Self { env, sources, user_vars: None }
    }

    /// Set document-level user vars for injection into the `config` namespace.
    pub fn set_user_vars(&mut self, vars: &std::collections::HashMap<String, crate::value::Value>) {
        if vars.is_empty() {
            return;
        }
        self.user_vars = Some(crate::config::build_jinja_vars(vars));
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
    pub fn render_dynamic(&self, name: &str, template_source: &str, vars: &TemplateVars) -> String {
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
    pub fn render(&self, name: &str, vars: &TemplateVars) -> String {
        let tpl = match self.env.get_template(name) {
            Ok(t) => t,
            Err(_) => return String::new(),
        };
        let ctx = build_jinja_context(vars, self.user_vars.as_ref());
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
pub fn render_element(name: &str, ext: &str, vars: &TemplateVars) -> String {
    use crate::render::elements::resolve_element_template;
    if let Some(tpl) = resolve_element_template(name, ext) {
        let mut vars = TemplateVars {
            config: vars.config.clone(),
            calepin: vars.calepin.clone(),
        };
        vars.calepin.insert("writer".to_string(), ext.to_string());
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
    _target: Option<&crate::config::Target>,
) -> TemplateVars {
    let mut vars = TemplateVars::new();

    let defs = meta;

    // calepin.* (engine-computed)
    vars.calepin.insert("body".to_string(), body.to_string());
    vars.calepin.insert(
        "generator".to_string(),
        format!("calepin {}", env!("CARGO_PKG_VERSION")),
    );
    vars.calepin.insert("preamble".to_string(), String::new());
    vars.calepin.insert("writer".to_string(), ext.to_string());

    // config.* (user-authored)
    vars.config.insert("target".to_string(), ext.to_string());

    // Language
    vars.config.insert("lang".to_string(), defs.lang.as_deref().unwrap_or("en").to_string());

    // Labels (localisable strings)
    let labels = defs.labels.as_ref();
    let label_defs: &[(&str, fn(&crate::config::LabelsConfig) -> &Option<String>, &str)] = &[
        ("label_abstract",  |l| &l.abstract_title, "Abstract"),
        ("label_keywords",  |l| &l.keywords,       "Keywords"),
        ("label_appendix",  |l| &l.appendix,       "Appendix"),
        ("label_citation",  |l| &l.citation,       "Citation"),
        ("label_reuse",     |l| &l.reuse,          "Reuse"),
        ("label_funding",   |l| &l.funding,        "Funding"),
        ("label_copyright", |l| &l.copyright,      "Copyright"),
        ("label_listing",   |l| &l.listing,        "Listing"),
        ("label_proof",     |l| &l.proof,          "Proof"),
        ("label_contents",  |l| &l.contents,       "Contents"),
    ];
    for (key, getter, default) in label_defs {
        let val = labels.and_then(|l| getter(l).clone()).unwrap_or_else(|| default.to_string());
        vars.config.insert(key.to_string(), val);
    }

    // Plain title (used in <title> etc.) -- strip markdown image/link syntax
    let plain_title = meta.title.as_deref().unwrap_or("Untitled");
    let plain_title = strip_markdown_formatting(plain_title);
    vars.config.insert("plain_title".to_string(), plain_title);
    vars.config.insert("title".to_string(),
        meta.title.as_deref()
            .map(|t| crate::render::convert::render_inline(t, ext))
            .unwrap_or_default(),
    );
    {
        let names = meta.author_names();
        vars.config.insert(
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
    vars.config.insert("date".to_string(), meta.formatted_date().unwrap_or_default());

    // Subtitle
    if let Some(ref subtitle) = meta.subtitle {
        vars.config.insert("subtitle".to_string(), crate::render::convert::render_inline(subtitle, ext));
    }

    // Author block (rendered by engine from user metadata)
    vars.calepin.insert("authors".to_string(), build_authors(meta, ext));

    // Abstract block
    if let Some(ref abs) = meta.abstract_text {
        vars.config.insert("abstract".to_string(), crate::render::convert::render_inline(abs, ext));
    } else {
        vars.config.insert("abstract".to_string(), String::new());
    }

    // Keywords
    if !meta.keywords.is_empty() {
        let joined = meta.keywords.join(", ");
        vars.config.insert("keywords".to_string(), joined);
    }

    // Appendix (engine-rendered from user metadata)
    vars.calepin.insert("appendix".to_string(), build_appendix(meta, ext));

    // Default values for format-specific template variables (engine assets)
    vars.calepin.insert("css".to_string(), load_default_css());
    vars.calepin.insert("js".to_string(), {
        let root = crate::paths::get_project_dir();
        load_all_extension_assets(&root, |r, name| load_extension_js(r, name))
    });
    vars.calepin.insert("bib_preamble".to_string(), String::new());
    vars.calepin.insert("bib_end".to_string(), String::new());

    // Math include for html-writer targets
    if ext == "html" {
        let mut math_vars = TemplateVars::new();
        math_vars.config.insert("html_math_method".to_string(),
            meta.html_math_method.as_deref()
                .unwrap_or_else(|| defs.math.as_deref().unwrap_or("katex")).to_string());
        vars.calepin.insert("math".to_string(), render_element("math", ext, &math_vars));
    } else {
        vars.calepin.insert("math".to_string(), String::new());
    }

    // Bibliography block (format-specific via element template)
    if !meta.bibliography.is_empty() {
        let bib_path = &meta.bibliography[0];
        let mut bvars = TemplateVars::new();
        bvars.config.insert("path".to_string(), bib_path.clone());
        vars.calepin.insert("bibliography".to_string(),
            render_element("bibliography", ext, &bvars));
    }

    // Table of contents
    let toc_cfg = meta.toc.as_ref();
    let toc_enabled = toc_cfg.and_then(|t| t.enabled).unwrap_or(ext == "html");
    if toc_enabled {
        let (toc_depth, toc_title) = meta.toc_depth_title();
        let toc = if ext == "html" {
            // HTML: build nested list in Rust, wrap with template
            build_toc_html(headings, toc_depth, toc_title)
        } else {
            // LaTeX, Typst, others: use the toc template directly
            let mut toc_vars = TemplateVars::with_writer(ext);
            toc_vars.config.insert("title".to_string(), toc_title.to_string());
            toc_vars.config.insert("depth".to_string(), toc_depth.to_string());
            toc_vars.calepin.insert("toc_list".to_string(), String::new());
            let tpl_owned = crate::render::elements::resolve_element_template("toc", ext).unwrap_or_default();
            let tpl = tpl_owned.as_str();
            apply_template(tpl, &toc_vars)
        };
        vars.calepin.insert("toc".to_string(), toc);
    } else {
        vars.calepin.insert("toc".to_string(), String::new());
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
/// template variable (in the `calepin` namespace).
pub fn inject_preamble(vars: &mut TemplateVars, preamble: &[String]) {
    let content = deduplicate_preamble(preamble);
    if !content.is_empty() {
        let entry = vars.calepin.entry("preamble".to_string()).or_default();
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
    target: Option<&crate::config::Target>,
    customize: impl FnOnce(&mut TemplateVars),
) -> String {
    let mut vars = build_template_vars_with_headings(meta, body, format, headings, target);
    inject_preamble(&mut vars, preamble);
    customize(&mut vars);
    let tpl = load_page_template("page", format);
    render_page_template(&tpl, &vars, format, &meta.var)
}

/// Render a page template with {% include %} support.
///
/// Sets up a MiniJinja environment with:
///   1. templates/{target}/ (target-specific, from active target)
///   2. templates/{base}/ (base-specific)
///   3. templates/common/ (format-agnostic fallback)
///   4. Built-in templates/{base}/ (embedded in binary)
///   5. Built-in templates/common/ (embedded in binary)
///
/// The page template and all included component templates share the same
/// context, so `{% include "preamble.html" %}` in the page template can
/// access all variables (base, title, author, body, etc.).
pub fn render_page_template(
    page_template: &str,
    vars: &TemplateVars,
    base: &str,
    user_vars: &std::collections::HashMap<String, crate::value::Value>,
) -> String {
    // Collect all template sources into an owned map, then use set_loader
    // so minijinja takes ownership -- no Box::leak needed.
    let mut templates = HashMap::new();

    let root = crate::paths::get_project_dir();
    let active_target = crate::paths::get_active_target();
    let tpl_dir = crate::paths::templates_dir(&root);

    // Load templates from filesystem directories
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Some(ref target) = active_target {
        if target != base {
            dirs.push(tpl_dir.join(target));
        }
    }
    dirs.push(tpl_dir.join(base));
    dirs.push(tpl_dir.join("common"));

    // Also check extension template directories (child-first order)
    for ext_dir in crate::paths::get_extension_template_dirs() {
        if let Some(ref target) = active_target {
            if target != base {
                dirs.push(ext_dir.join(target));
            }
        }
        dirs.push(ext_dir.join(base));
        dirs.push(ext_dir.join("common"));
    }

    for dir in &dirs {
        if !dir.is_dir() { continue; }
        let pattern = dir.join("**").join("*.*");
        let pattern_str = pattern.display().to_string();
        for entry in crate::util::safe_glob(&pattern_str) {
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
    if let Some(base_dir) = crate::render::elements::BUILTIN_TEMPLATES.get_dir(base) {
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
    if let Some(common_dir) = crate::render::elements::BUILTIN_TEMPLATES.get_dir("common") {
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

    let uv = if !user_vars.is_empty() {
        Some(crate::config::build_jinja_vars(user_vars))
    } else {
        None
    };
    let ctx = build_jinja_context(vars, uv.as_ref());
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
        let template = "<title>{{config.title}}</title>\n<body>{{calepin.body}}</body>";
        let mut vars = TemplateVars::new();
        vars.config.insert("title".to_string(), "Hello".to_string());
        vars.calepin.insert("body".to_string(), "<p>World</p>".to_string());
        let result = apply_template(template, &vars);
        assert_eq!(result, "<title>Hello</title>\n<body><p>World</p></body>");
    }

    #[test]
    fn test_missing_vars_become_empty() {
        let template = "{{config.title}}: {{config.missing}}";
        let mut vars = TemplateVars::new();
        vars.config.insert("title".to_string(), "Hello".to_string());
        let result = apply_template(template, &vars);
        assert_eq!(result, "Hello: ");
    }
}
