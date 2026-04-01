mod includes;
mod protection;
mod variables;

pub use includes::expand_includes;

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
    vars.cfg.insert("title".to_string(), minijinja::Value::from(title.to_string()));
    vars.clp.insert("content".to_string(), minijinja::Value::from(toc_list.clone()));

    // Build structured items list for templates that use {% for %}
    let min_level = items.iter().map(|(l, _, _)| *l).min().unwrap_or(1);
    let structured: Vec<minijinja::Value> = items.iter()
        .map(|(level, id, text)| {
            let mut map = std::collections::BTreeMap::new();
            map.insert("id".to_string(), minijinja::Value::from(id.to_string()));
            map.insert("text".to_string(), minijinja::Value::from(text.to_string()));
            map.insert("level".to_string(), minijinja::Value::from(*level as i64));
            map.insert("depth".to_string(), minijinja::Value::from((*level - min_level) as i64));
            minijinja::Value::from(map)
        })
        .collect();
    vars.clp.insert("items".to_string(), minijinja::Value::from(structured));

    let tpl_owned = crate::render::elements::resolve_element_template("toc", "html")
        .unwrap_or_else(|| include_str!("../../targets/html/templates/toc.html").to_string());
    apply_template(&tpl_owned, &vars)
}

use crate::render::metadata::{strip_markdown_formatting, build_appendix, build_authors};

/// Load a page template by name and base.
///
/// Loads from the sidecar's filesystem templates. No built-in fallback;
/// sidecars are always populated with templates.
pub fn load_page_template(template_name: &str, base: &str) -> String {
    crate::paths::resolve_template(template_name, base)
        .and_then(|path| std::fs::read_to_string(&path).ok())
        .unwrap_or_default()
}

pub fn load_default_css() -> String {
    let mut css = concatenate_asset_dir("css", "css");

    // Extension CSS (active target + side-loaded extensions)
    let root = crate::paths::get_project_dir();
    css.push_str(&load_all_extension_assets(&root, |r, name| load_extension_css(r, name)));

    css
}

/// Load only extension CSS (not the base sidecar calepin.css).
/// Used when the base CSS is linked via `<link>` but extension CSS must be inlined.
pub fn load_extension_css_only() -> String {
    let root = crate::paths::get_project_dir();
    load_all_extension_assets(&root, |r, name| load_extension_css(r, name))
}

/// Return the relative path from the output file to a sidecar asset file,
/// e.g. `my_doc_calepin/assets/calepin.css`. Returns None if no sidecar or
/// the asset file does not exist.
fn sidecar_asset_path(filename: &str) -> Option<String> {
    let sidecar = crate::paths::get_page_sidecar()
        .or_else(|| crate::paths::get_root_sidecar())?;
    let asset = sidecar.join("assets").join(filename);
    if !asset.exists() { return None; }
    let sidecar_name = sidecar.file_name()?;
    Some(format!("{}/assets/{}", sidecar_name.to_string_lossy(), filename))
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

/// Load the default JS: assets/js/*.js from sidecar + extension JS.
/// This is inlined in document templates via `{{clp.js}}`.
pub fn load_default_js() -> String {
    let mut js = concatenate_asset_dir("js", "js");

    // Extension JS (active target + side-loaded extensions)
    let root = crate::paths::get_project_dir();
    js.push_str(&load_all_extension_assets(&root, |r, name| load_extension_js(r, name)));

    js
}

/// Concatenate all files with the given extension from a sidecar assets subdirectory,
/// sorted alphabetically by filename. Used for `assets/css/*.css` and `assets/js/*.js`.
pub fn concatenate_asset_dir(dir_name: &str, extension: &str) -> String {
    let dir = crate::paths::get_root_sidecar()
        .or_else(|| crate::paths::get_page_sidecar())
        .map(|s| s.join("assets").join(dir_name));
    let Some(dir) = dir.filter(|d| d.is_dir()) else { return String::new() };
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == extension))
        .collect();
    files.sort_by_key(|e| e.file_name());
    files
        .iter()
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Two-namespace template variable container.
///
/// - `config`: variables the user authored (front matter, config.toml, div/span
///   attributes, CLI `-s`). Accessed in templates as `{{ config.title }}`.
/// - `calepin`: variables the engine computed or resolved. Accessed in templates
///   as `{{ calepin.body }}`.
pub struct TemplateVars {
    pub cfg: HashMap<String, minijinja::Value>,
    pub clp: HashMap<String, minijinja::Value>,
    pub tpl: HashMap<String, String>,
}

impl TemplateVars {
    pub fn new() -> Self {
        Self {
            cfg: HashMap::new(),
            clp: HashMap::new(),
            tpl: HashMap::new(),
        }
    }

    /// Create a new TemplateVars with the writer already set.
    pub fn with_writer(writer: &str) -> Self {
        let mut vars = Self::new();
        vars.clp.insert("writer".to_string(), minijinja::Value::from(writer));
        vars
    }

}

/// Build a MiniJinja render context from namespaced template variables.
///
/// Produces a context with two top-level objects (`cfg` and `clp`).
/// When `user_vars` is provided, its entries are merged into the `cfg`
/// object so custom front matter keys are accessible as `{{ cfg.key }}`.
fn build_jinja_context(
    vars: &TemplateVars,
    user_vars: Option<&minijinja::Value>,
) -> std::collections::BTreeMap<&'static str, minijinja::Value> {
    let mut ctx = std::collections::BTreeMap::new();

    // Build cfg object: vars + nested user vars
    let mut config_map = std::collections::BTreeMap::new();
    for (key, value) in &vars.cfg {
        config_map.insert(key.as_str(), value.clone());
    }
    // Merge user vars (custom front matter keys) into cfg
    if let Some(uv) = user_vars {
        if let Ok(iter) = uv.try_iter() {
            for key in iter {
                let key_str = key.to_string();
                // Don't overwrite explicit cfg entries
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
    ctx.insert("cfg", minijinja::Value::from_serialize(&config_map));

    // Build clp object
    let mut calepin_map = std::collections::BTreeMap::new();
    for (key, value) in &vars.clp {
        calepin_map.insert(key.as_str(), value.clone());
    }
    ctx.insert("clp", minijinja::Value::from_serialize(&calepin_map));

    // Build tpl object (template variant selections)
    if !vars.tpl.is_empty() {
        let mut tpl_map = std::collections::BTreeMap::new();
        for (key, value) in &vars.tpl {
            tpl_map.insert(key.as_str(), minijinja::Value::from(value.as_str()));
        }
        ctx.insert("tpl", minijinja::Value::from_serialize(&tpl_map));
    }

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
pub fn render_element(name: &str, writer: &str, vars: &TemplateVars) -> String {
    use crate::render::elements::resolve_element_template;
    if let Some(tpl) = resolve_element_template(name, writer) {
        let mut vars = TemplateVars {
            cfg: vars.cfg.clone(),
            clp: vars.clp.clone(),
            tpl: vars.tpl.clone(),
        };
        vars.clp.insert("writer".to_string(), minijinja::Value::from(writer.to_string()));
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
    writer: &str,
    headings: &[crate::emit::TocEntry],
    _target: Option<&crate::config::Target>,
) -> TemplateVars {
    let mut vars = TemplateVars::new();

    // tpl.* (template variant selections)
    vars.tpl = meta.tpl.clone();

    let defs = meta;

    // calepin.* (engine-computed)
    vars.clp.insert("body".to_string(), minijinja::Value::from(body.to_string()));
    vars.clp.insert("preamble".to_string(), minijinja::Value::from(String::new()));
    vars.clp.insert("writer".to_string(), minijinja::Value::from(writer.to_string()));

    // config.* (user-authored)
    vars.cfg.insert("target".to_string(), minijinja::Value::from(writer.to_string()));

    // Language
    vars.cfg.insert("lang".to_string(), minijinja::Value::from(defs.lang.as_deref().unwrap_or("en").to_string()));

    // Plain title (used in <title> etc.) -- strip markdown image/link syntax
    let plain_title = meta.title.as_deref().unwrap_or("Untitled");
    let plain_title = strip_markdown_formatting(plain_title);
    vars.cfg.insert("title_plain".to_string(), minijinja::Value::from(plain_title));
    vars.cfg.insert("title".to_string(),
        minijinja::Value::from(meta.title.as_deref()
            .map(|t| crate::render::convert::render_inline(t, writer))
            .unwrap_or_default()),
    );
    vars.cfg.insert("date".to_string(), minijinja::Value::from(meta.formatted_date().unwrap_or_default()));

    // Subtitle
    if let Some(ref subtitle) = meta.subtitle {
        vars.cfg.insert("subtitle".to_string(), minijinja::Value::from(crate::render::convert::render_inline(subtitle, writer)));
    }

    // Author block (rendered by engine from user metadata)
    vars.clp.insert("authors".to_string(), minijinja::Value::from(build_authors(meta, writer)));

    // Abstract block
    if let Some(ref abs) = meta.abstract_text {
        vars.cfg.insert("abstract".to_string(), minijinja::Value::from(crate::render::convert::render_inline(abs, writer)));
    } else {
        vars.cfg.insert("abstract".to_string(), minijinja::Value::from(String::new()));
    }

    // Keywords
    if !meta.keywords.is_empty() {
        let joined = meta.keywords.join(", ");
        vars.cfg.insert("keywords".to_string(), minijinja::Value::from(joined));
    }

    // Appendix (engine-rendered from user metadata)
    vars.clp.insert("appendix".to_string(), minijinja::Value::from(build_appendix(meta, writer)));

    // CSS and JS: standalone mode inlines everything, non-standalone links external files.
    // Target-level standalone (e.g., website sets false) takes precedence over metadata.
    let standalone = _target.and_then(|t| t.standalone).or(meta.standalone).unwrap_or(true);
    if !standalone {
        let css_path = sidecar_asset_path("calepin.css");
        vars.clp.insert("css_path".to_string(), minijinja::Value::from(css_path.unwrap_or_default()));
        let js_path = sidecar_asset_path("calepin.js");
        vars.clp.insert("js_path".to_string(), minijinja::Value::from(js_path.unwrap_or_default()));
    }
    vars.clp.insert("css".to_string(), minijinja::Value::from(load_default_css()));
    vars.clp.insert("js".to_string(), minijinja::Value::from(load_default_js()));

    // Extension-only CSS (inlined even when base CSS is linked via <link>)
    vars.clp.insert("ext_css".to_string(), minijinja::Value::from(load_extension_css_only()));

    // Color scheme CSS and Tailwind colors.
    // Priority: first entry in cfg.colors > extension default_colors > nord fallback.
    if writer == "html" {
        let target_name = crate::paths::get_active_target().unwrap_or_else(|| writer.to_string());

        let colors = crate::config::extension::resolve_active_colors(&meta.cfg, &target_name);

        let colors_css = colors.as_ref()
            .map(|c| c.generate_color_css())
            .unwrap_or_default();
        vars.clp.insert("colors_css".to_string(), minijinja::Value::from(colors_css));

        // Tailwind color config generated from color token keys
        let tailwind_colors = colors.as_ref()
            .map(|c| c.generate_tailwind_colors())
            .unwrap_or_default();
        vars.clp.insert("tailwind_colors".to_string(), minijinja::Value::from(tailwind_colors));
    }

    // Math include for html-writer targets
    if writer == "html" {
        let math_vars = TemplateVars::new();
        vars.clp.insert("math".to_string(), minijinja::Value::from(render_element("math", writer, &math_vars)));
    } else {
        vars.clp.insert("math".to_string(), minijinja::Value::from(String::new()));
    }

    // Bibliography block (format-specific via element template)
    if !meta.bibliography.is_empty() {
        let bib_path = &meta.bibliography[0];
        let mut bvars = TemplateVars::new();
        bvars.cfg.insert("path".to_string(), minijinja::Value::from(bib_path.clone()));
        vars.clp.insert("bibliography".to_string(),
            minijinja::Value::from(render_element("bibliography", writer, &bvars)));
    }

    // Table of contents
    let toc_cfg = meta.toc.as_ref();
    let toc_enabled = toc_cfg.and_then(|t| t.enabled).unwrap_or(writer == "html");
    if toc_enabled {
        let (toc_depth, toc_title) = meta.toc_depth_title();
        let toc = if writer == "html" {
            // HTML: build nested list in Rust, wrap with template
            build_toc_html(headings, toc_depth, toc_title)
        } else {
            // LaTeX, Typst, others: use the toc template directly
            let mut toc_vars = TemplateVars::with_writer(writer);
            toc_vars.cfg.insert("title".to_string(), minijinja::Value::from(toc_title.to_string()));
            let tpl_owned = crate::render::elements::resolve_element_template("toc", writer).unwrap_or_default();
            let tpl = tpl_owned.as_str();
            apply_template(tpl, &toc_vars)
        };
        vars.clp.insert("toc".to_string(), minijinja::Value::from(toc));
    } else {
        vars.clp.insert("toc".to_string(), minijinja::Value::from(String::new()));
    }

    // Merge custom front matter keys into cfg (non-standard keys from config/front matter)
    if !meta.cfg.is_empty() {
        let jinja_vars = crate::config::build_jinja_vars(&meta.cfg);
        if let Ok(iter) = jinja_vars.try_iter() {
            for key in iter {
                let key_str = key.to_string();
                if !vars.cfg.contains_key(&key_str) {
                    if let Ok(val) = jinja_vars.get_attr(&key_str) {
                        vars.cfg.insert(key_str, val);
                    }
                }
            }
        }
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
        let existing = vars.clp.get("preamble")
            .map(|v| v.to_string())
            .unwrap_or_default();
        let new_val = if existing.is_empty() {
            content
        } else {
            format!("{}\n{}", existing, content)
        };
        vars.clp.insert("preamble".to_string(), minijinja::Value::from(new_val));
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
    writer: &str,
    headings: &[crate::emit::TocEntry],
    preamble: &[String],
    target: Option<&crate::config::Target>,
    customize: impl FnOnce(&mut TemplateVars),
) -> String {
    let mut vars = build_template_vars_with_headings(meta, body, writer, headings, target);
    inject_preamble(&mut vars, preamble);
    customize(&mut vars);
    let template_name = target.map(|t| t.template_name()).unwrap_or("main");
    let tpl = load_page_template(template_name, writer);
    render_page_template(&tpl, &vars, writer, &meta.cfg)
}

/// Render a page template with {% include %} support.
///
/// Sets up a MiniJinja environment with:
///   1. templates/{target}/ (target-specific, from active target)
///   2. templates/{base}/ (base-specific)
///   3. templates/common/ (format-agnostic fallback)
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
    let target = active_target.as_deref().unwrap_or(base);

    // Load templates from the sidecar's filesystem (always populated)
    {
        let tpl_dir = crate::paths::templates_dir(&root);
        let dir = tpl_dir.join(target);
        if dir.is_dir() {
            let pattern = dir.join("**").join("*.*");
            let pattern_str = pattern.display().to_string();
            for entry in crate::util::safe_glob(&pattern_str) {
                if let Ok(path) = entry {
                    if !path.is_file() { continue; }
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let rel = path.strip_prefix(&dir).unwrap_or(&path);
                        let name = rel.display().to_string();
                        templates.entry(name).or_insert(content);
                    }
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

// ---------------------------------------------------------------------------
// Body processing (Jinja evaluation of .qmd body text)
// ---------------------------------------------------------------------------

use protection::{protect_code_blocks, protect_inline_code, restore_code_blocks};

/// Result of Jinja body processing.
pub struct BodyResult {
    pub text: String,
}

/// Process a text block through MiniJinja, evaluating functions and variable references.
#[inline(never)]
pub fn process_body(
    text: &str,
    writer: &str,
    metadata: &Metadata,
) -> BodyResult {
    // 1. Protect fenced code blocks and inline code from Jinja
    let (protected, mut code_blocks) = protect_code_blocks(text);
    let protected = protect_inline_code(&protected, &mut code_blocks);

    // 1b. Escape heading attribute syntax {#id .class} which Jinja
    //     interprets as comment openers ({# ... #}).
    static RE_HEADING_ATTR: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{(#[a-zA-Z][a-zA-Z0-9_-]*(?:\s+\.[a-zA-Z][a-zA-Z0-9_-]*)*)\}").unwrap()
    });
    let protected = RE_HEADING_ATTR.replace_all(&protected, "\u{FDD2}$1\u{FDD3}").to_string();

    // Quick exit: if no Jinja syntax found, skip processing
    if !protected.contains("{{") && !protected.contains("{%") {
        return BodyResult {
            text: text.to_string(),
        };
    }

    // 2. Build MiniJinja environment
    let mut env = minijinja::Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);

    // 3. Build context with metadata, variables, and environment
    let context = variables::build_context(metadata, writer);

    // 4. Render through MiniJinja (on error, fall back to protected text so that
    //    restore_code_blocks can still recover code block placeholders)
    let rendered = match env.render_str(&protected, &context) {
        Ok(r) => r,
        Err(e) => {
            cwarn!("body template error: {}", e);
            protected.clone()
        }
    };

    // 5. Restore protected content
    let rendered = rendered.replace('\u{FDD2}', "{").replace('\u{FDD3}', "}");
    let text = restore_code_blocks(&rendered, &code_blocks);

    BodyResult { text }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_template() {
        let template = "<title>{{cfg.title}}</title>\n<body>{{clp.body}}</body>";
        let mut vars = TemplateVars::new();
        vars.cfg.insert("title".to_string(), minijinja::Value::from("Hello".to_string()));
        vars.clp.insert("body".to_string(), minijinja::Value::from("<p>World</p>".to_string()));
        let result = apply_template(template, &vars);
        assert_eq!(result, "<title>Hello</title>\n<body><p>World</p></body>");
    }

    #[test]
    fn test_missing_vars_become_empty() {
        let template = "{{cfg.title}}: {{cfg.missing}}";
        let mut vars = TemplateVars::new();
        vars.cfg.insert("title".to_string(), minijinja::Value::from("Hello".to_string()));
        let result = apply_template(template, &vars);
        assert_eq!(result, "Hello: ");
    }

    #[test]
    fn test_protect_restore_fenced_code() {
        let text = "before\n```python\nx = {{ hello }}\n```\nafter";
        let (protected, blocks) = protect_code_blocks(text);
        assert!(!protected.contains("{{ hello }}"));
        assert_eq!(blocks.len(), 1);
        let restored = restore_code_blocks(&protected, &blocks);
        assert_eq!(restored, text);
    }

    #[test]
    fn test_inline_code_protected() {
        let mut meta = Metadata::default();
        meta.title = Some("T".to_string());
        let result = process_body("version: `{{ config.title }}`", "html", &meta);
        assert!(result.text.contains("`{{ config.title }}`"));
    }

    #[test]
    fn test_no_jinja_syntax_passthrough() {
        let text = "plain text with no template syntax";
        let meta = Metadata::default();
        let result = process_body(text, "html", &meta);
        assert_eq!(result.text, text);
    }

    #[test]
    fn test_config_variable_access() {
        let mut meta = Metadata::default();
        meta.title = Some("My Title".to_string());
        let result = process_body("Title: {{ cfg.title }}", "html", &meta);
        assert_eq!(result.text, "Title: My Title");
    }

    #[test]
    fn test_env_context_variable() {
        std::env::set_var("CALEPIN_TEST_VAR", "hello_jinja");
        let meta = Metadata::default();
        let result = process_body("{{ env.CALEPIN_TEST_VAR }}", "html", &meta);
        assert_eq!(result.text, "hello_jinja");
        std::env::remove_var("CALEPIN_TEST_VAR");
    }

    #[test]
    fn test_code_blocks_preserved() {
        let text = "before {{ cfg.title }}\n```\n{{ not_a_var }}\n```\nafter";
        let mut meta = Metadata::default();
        meta.title = Some("T".to_string());
        let result = process_body(text, "html", &meta);
        assert!(result.text.contains("before T"));
        assert!(result.text.contains("{{ not_a_var }}"));
    }
}
