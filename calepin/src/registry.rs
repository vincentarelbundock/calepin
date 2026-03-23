//! Plugin registry: loading, indexing, and dispatch.
//!
//! The `PluginRegistry` is the single entry point for all plugin-related
//! operations. It replaces the previous scattered extension mechanisms:
//! WASM plugins, external filters, external shortcodes, element/page
//! templates, custom formats.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

use crate::filters::Filter;
use crate::plugin_manifest::{FilterMatch, FilterSpec, FormatSpec, PluginManifest, PluginProvides};
use crate::types::Element;

// ---------------------------------------------------------------------------
// Structural handler trait — for built-in plugins that need raw children
// ---------------------------------------------------------------------------

/// Structural handlers receive raw (un-rendered) children and a render closure.
/// They are built-in plugins that need per-child rendering control (tabsets,
/// layouts, figure divs).
pub trait StructuralHandler {
    /// Render a div. Returns `Some(output)` if handled, `None` to pass.
    fn render_div(
        &self,
        classes: &[String],
        id: &Option<String>,
        attrs: &HashMap<String, String>,
        children: &[Element],
        format: &str,
        render_element: &dyn Fn(&Element) -> String,
        resolve_template: &dyn Fn(&str) -> Option<String>,
        raw_fragments: &RefCell<Vec<String>>,
    ) -> Option<String>;
}

// ---------------------------------------------------------------------------
// Plugin kind — how a plugin is executed
// ---------------------------------------------------------------------------

pub enum PluginKind {
    /// Built-in structural plugin (tabset, layout, figure-div).
    /// Runs before child rendering, receives raw children.
    BuiltinStructural(Box<dyn StructuralHandler>),

    /// Built-in filter plugin (theorem, callout).
    /// Runs after child rendering, receives rendered content + vars.
    BuiltinFilter(Box<dyn Filter>),

    /// External subprocess plugin. Spawned per-call.
    Subprocess { path: PathBuf },

    /// Persistent subprocess plugin. Spawned once, communicates via JSON lines.
    PersistentSubprocess {
        path: PathBuf,
        process: RefCell<Option<PersistentProcess>>,
    },
}

// ---------------------------------------------------------------------------
// Loaded plugin — manifest + runtime handle
// ---------------------------------------------------------------------------

pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub kind: PluginKind,
}

// ---------------------------------------------------------------------------
// Persistent subprocess
// ---------------------------------------------------------------------------

pub struct PersistentProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl PersistentProcess {
    fn spawn(path: &Path) -> Option<Self> {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| eprintln!("Warning: failed to spawn persistent plugin {:?}: {}", path, e))
            .ok()?;

        let stdin = child.stdin.take()?;
        let stdout = BufReader::new(child.stdout.take()?);

        Some(PersistentProcess { child, stdin, stdout })
    }

    fn call(&mut self, request: &serde_json::Value) -> Option<serde_json::Value> {
        let mut line = serde_json::to_string(request).ok()?;
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).ok()?;
        self.stdin.flush().ok()?;

        let mut response_line = String::new();
        self.stdout.read_line(&mut response_line).ok()?;
        serde_json::from_str(&response_line).ok()
    }
}

impl Drop for PersistentProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ---------------------------------------------------------------------------
// Plugin registry
// ---------------------------------------------------------------------------

pub struct PluginRegistry {
    plugins: Vec<LoadedPlugin>,
}

impl PluginRegistry {
    /// Create a new registry from the front matter plugin list.
    /// Resolves plugins relative to `document_dir`.
    /// Loads user plugins first (in order), then appends built-in plugins.
    pub fn load(names: &[String], document_dir: &Path) -> Self {
        let mut plugins = Vec::new();

        // Load user plugins
        for name in names {
            match crate::paths::resolve_plugin_dir(name, document_dir) {
                Some(dir) => match PluginManifest::load(&dir) {
                    Ok(manifest) => {
                        // Check if any filter is persistent
                        let persistent_filter = manifest.provides.filters.iter()
                            .find(|f| f.persistent);
                        let kind = if let Some(pf) = persistent_filter {
                            let path = pf.run.clone()
                                .unwrap_or_else(|| dir.join("filter"));
                            PluginKind::PersistentSubprocess {
                                path,
                                process: RefCell::new(None),
                            }
                        } else {
                            PluginKind::Subprocess { path: dir }
                        };
                        plugins.push(LoadedPlugin { manifest, kind });
                    }
                    Err(e) => eprintln!("Warning: failed to load plugin '{}': {}", name, e),
                },
                None => eprintln!("Warning: plugin '{}' not found", name),
            }
        }

        // Register built-in plugins
        register_builtins(&mut plugins);

        PluginRegistry { plugins }
    }

    /// Return an empty registry (no plugins, just built-ins).
    pub fn empty() -> Self {
        let mut plugins = Vec::new();
        register_builtins(&mut plugins);
        PluginRegistry { plugins }
    }

    // -----------------------------------------------------------------------
    // Filter dispatch
    // -----------------------------------------------------------------------

    /// Iterate over plugins whose filter matches the given element properties.
    /// Returns matching (plugin, filter_spec) pairs in registry order.
    /// A single plugin may appear multiple times if it has multiple matching filters.
    pub fn matching_filters<'a>(
        &'a self,
        classes: &[String],
        attrs: &HashMap<String, String>,
        id: Option<&str>,
        format: &str,
        context: &str,
    ) -> Vec<(&'a LoadedPlugin, &'a FilterSpec)> {
        let mut result = Vec::new();
        for plugin in &self.plugins {
            for spec in &plugin.manifest.provides.filters {
                if spec.contexts.iter().any(|c| c == context)
                    && spec.match_rule.matches(classes, attrs, id, format)
                {
                    result.push((plugin, spec));
                }
            }
        }
        result
    }

    /// Call a subprocess filter (one-shot or persistent).
    /// Returns `Some(output)` if the plugin rendered, `None` to pass.
    pub fn call_subprocess_filter(
        &self,
        plugin: &LoadedPlugin,
        filter_spec: &FilterSpec,
        context: &str,
        content: &str,
        classes: &[String],
        id: &str,
        format: &str,
        attrs: &HashMap<String, String>,
    ) -> Option<String> {
        let input = build_filter_json(context, content, classes, id, format, attrs);

        match &plugin.kind {
            PluginKind::Subprocess { path: _ } => {
                let run_path = filter_spec.run.as_ref()?;
                crate::util::run_json_process(run_path, &input)
            }
            PluginKind::PersistentSubprocess { path, process } => {
                let mut proc = process.borrow_mut();
                if proc.is_none() {
                    *proc = PersistentProcess::spawn(path);
                }
                let proc = proc.as_mut()?;

                let mut request = serde_json::json!({ "type": "filter" });
                if let serde_json::Value::Object(ref mut map) = request {
                    if let serde_json::Value::Object(input_map) = input {
                        map.extend(input_map);
                    }
                }

                let response = proc.call(&request)?;
                match response["result"].as_str()? {
                    "rendered" => response["output"].as_str().map(|s| s.to_string()),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Shortcode dispatch
    // -----------------------------------------------------------------------

    /// Find the first plugin that handles a shortcode name.
    pub fn matching_shortcode(&self, name: &str) -> Option<&LoadedPlugin> {
        self.plugins.iter().find(|p| {
            p.manifest.provides.shortcode.as_ref()
                .map_or(false, |s| s.names.iter().any(|n| n == name))
        })
    }

    /// Return all shortcode names and their plugin index, for Jinja function registration.
    pub fn shortcode_names(&self) -> Vec<(String, usize)> {
        let mut result = Vec::new();
        for (idx, plugin) in self.plugins.iter().enumerate() {
            if let Some(ref sc) = plugin.manifest.provides.shortcode {
                for name in &sc.names {
                    result.push((name.clone(), idx));
                }
            }
        }
        result
    }

    /// Get a plugin by its index in the plugins vector.
    pub fn plugin_by_index(&self, idx: usize) -> Option<&LoadedPlugin> {
        self.plugins.get(idx)
    }

    /// Call a subprocess shortcode.
    pub fn call_subprocess_shortcode(
        &self,
        plugin: &LoadedPlugin,
        name: &str,
        args: &[String],
        kwargs: &HashMap<String, String>,
        format: &str,
        meta: &serde_json::Value,
    ) -> Option<String> {
        let input = serde_json::json!({
            "name": name,
            "args": args,
            "kwargs": kwargs,
            "format": format,
            "meta": meta,
        });

        match &plugin.kind {
            PluginKind::Subprocess { path: _ } => {
                let run_path = plugin.manifest.provides.shortcode.as_ref()?.run.as_ref()?;
                crate::util::run_json_process(run_path, &input)
            }
            PluginKind::PersistentSubprocess { path, process } => {
                let mut proc = process.borrow_mut();
                if proc.is_none() {
                    *proc = PersistentProcess::spawn(path);
                }
                let proc = proc.as_mut()?;

                let mut request = serde_json::json!({ "type": "shortcode" });
                if let serde_json::Value::Object(ref mut map) = request {
                    if let serde_json::Value::Object(input_map) = input {
                        map.extend(input_map);
                    }
                }

                let response = proc.call(&request)?;
                response["output"].as_str().map(|s| s.to_string())
            }
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Postprocess dispatch
    // -----------------------------------------------------------------------

    /// Return plugins that have a postprocess handler for the given format.
    pub fn postprocessors(&self, format: &str) -> Vec<&LoadedPlugin> {
        self.plugins.iter().filter(|p| {
            p.manifest.provides.postprocess.as_ref().map_or(false, |pp| {
                pp.formats.is_empty() || pp.formats.iter().any(|f| f == format)
            })
        }).collect()
    }

    /// Call a subprocess postprocessor.
    pub fn call_subprocess_postprocess(
        &self,
        plugin: &LoadedPlugin,
        body: &str,
        format: &str,
        title: &str,
        css: &str,
    ) -> Option<String> {
        let input = serde_json::json!({
            "body": body,
            "format": format,
            "title": title,
            "css": css,
        });

        match &plugin.kind {
            PluginKind::Subprocess { path: _ } => {
                let run_path = plugin.manifest.provides.postprocess.as_ref()?.run.as_ref()?;
                crate::util::run_json_process(run_path, &input)
            }
            PluginKind::PersistentSubprocess { path, process } => {
                let mut proc = process.borrow_mut();
                if proc.is_none() {
                    *proc = PersistentProcess::spawn(path);
                }
                let proc = proc.as_mut()?;

                let mut request = serde_json::json!({ "type": "postprocess" });
                if let serde_json::Value::Object(ref mut map) = request {
                    if let serde_json::Value::Object(input_map) = input {
                        map.extend(input_map);
                    }
                }

                let response = proc.call(&request)?;
                response["output"].as_str().map(|s| s.to_string())
            }
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Template resolution
    // -----------------------------------------------------------------------

    /// Resolve an element template (component) by checking plugin element dirs (in order),
    /// then falling back to the three-layer component resolution.
    pub fn resolve_element_template(&self, name: &str, format: &str) -> Option<String> {
        let canonical = name.replace('-', "_");
        let filename = format!("{}.{}", canonical, format);

        // Check plugin-provided element/component dirs
        for plugin in &self.plugins {
            if let Some(ref spec) = plugin.manifest.provides.elements {
                let path = plugin.manifest.plugin_dir.join(&spec.dir).join(&filename);
                if path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        return Some(content);
                    }
                }
            }
        }

        // Three-layer resolution: project templates/ → user templates/ → legacy
        crate::paths::resolve_template(&canonical, format)
            .and_then(|p| std::fs::read_to_string(p).ok())
    }

    /// Resolve a page template by checking plugin template dirs (in order),
    /// then falling back to the three-layer template resolution.
    pub fn resolve_page_template(&self, filename: &str) -> Option<String> {
        // Check plugin-provided template dirs
        for plugin in &self.plugins {
            if let Some(ref spec) = plugin.manifest.provides.templates {
                let path = plugin.manifest.plugin_dir.join(&spec.dir).join(filename);
                if path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        return Some(content);
                    }
                }
            }
        }

        // Try new three-layer resolution if filename has a dot (e.g., "calepin.html")
        if let Some(dot) = filename.rfind('.') {
            let name = &filename[..dot];
            let ext = &filename[dot + 1..];
            let base = match ext {
                "tex" => "latex",
                "typ" => "typst",
                "md" => "markdown",
                other => other,
            };
            if let Some(path) = crate::paths::resolve_template(name, base) {
                if let Ok(content) = std::fs::read_to_string(path) {
                    return Some(content);
                }
            }
        }

        // Legacy fallback
        crate::paths::resolve_path_cwd("templates", filename)
            .and_then(|p| std::fs::read_to_string(p).ok())
    }

    /// Resolve a CSL file from plugins.
    pub fn resolve_csl(&self) -> Option<PathBuf> {
        for plugin in &self.plugins {
            if let Some(ref csl_file) = plugin.manifest.provides.csl {
                let path = plugin.manifest.plugin_dir.join(csl_file);
                if path.exists() {
                    return Some(path);
                }
            }
        }
        None
    }

    // -----------------------------------------------------------------------
    // Custom format resolution
    // -----------------------------------------------------------------------

    /// Find a plugin that provides a custom format with the given name.
    pub fn resolve_format(&self, name: &str) -> Option<&FormatSpec> {
        self.plugins.iter().find_map(|p| {
            p.manifest.provides.format.as_ref()
                .filter(|f| f.name == name)
        })
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    pub fn plugins(&self) -> &[LoadedPlugin] {
        &self.plugins
    }
}

// ---------------------------------------------------------------------------
// Built-in structural handlers
// ---------------------------------------------------------------------------

struct TabsetHandler;

impl StructuralHandler for TabsetHandler {
    fn render_div(
        &self,
        _classes: &[String],
        _id: &Option<String>,
        attrs: &HashMap<String, String>,
        children: &[Element],
        format: &str,
        render_element: &dyn Fn(&Element) -> String,
        _resolve_template: &dyn Fn(&str) -> Option<String>,
        _raw_fragments: &RefCell<Vec<String>>,
    ) -> Option<String> {
        Some(crate::structures::tabset::render(format, attrs, children, render_element))
    }
}

struct LayoutHandler;

impl StructuralHandler for LayoutHandler {
    fn render_div(
        &self,
        _classes: &[String],
        id: &Option<String>,
        attrs: &HashMap<String, String>,
        children: &[Element],
        format: &str,
        render_element: &dyn Fn(&Element) -> String,
        _resolve_template: &dyn Fn(&str) -> Option<String>,
        raw_fragments: &RefCell<Vec<String>>,
    ) -> Option<String> {
        Some(crate::structures::layout::render(id, attrs, children, format, render_element, raw_fragments))
    }
}

struct FigureDivHandler;

impl StructuralHandler for FigureDivHandler {
    fn render_div(
        &self,
        classes: &[String],
        id: &Option<String>,
        attrs: &HashMap<String, String>,
        children: &[Element],
        format: &str,
        render_element: &dyn Fn(&Element) -> String,
        resolve_template: &dyn Fn(&str) -> Option<String>,
        _raw_fragments: &RefCell<Vec<String>>,
    ) -> Option<String> {
        // Guard: don't handle figure divs that are also callouts
        if classes.iter().any(|c| c.starts_with("callout-")) {
            return None;
        }
        let id_val = id.as_ref()?;
        Some(crate::structures::figure::render_div(id_val, attrs, children, format, render_element, resolve_template))
    }
}

struct TableDivHandler;

impl StructuralHandler for TableDivHandler {
    fn render_div(
        &self,
        _classes: &[String],
        id: &Option<String>,
        attrs: &HashMap<String, String>,
        children: &[Element],
        format: &str,
        render_element: &dyn Fn(&Element) -> String,
        resolve_template: &dyn Fn(&str) -> Option<String>,
        _raw_fragments: &RefCell<Vec<String>>,
    ) -> Option<String> {
        let id_val = id.as_ref()?;
        Some(crate::structures::table::render_div(id_val, attrs, children, format, render_element, resolve_template))
    }
}

// ---------------------------------------------------------------------------
// Built-in plugin registration
// ---------------------------------------------------------------------------

fn register_builtins(plugins: &mut Vec<LoadedPlugin>) {
    // Tabset
    plugins.push(builtin_structural(
        "tabset",
        "Panel tabset rendering",
        FilterMatch {
            classes: vec!["panel-tabset".to_string()],
            ..Default::default()
        },
        vec!["div".to_string()],
        Box::new(TabsetHandler),
    ));

    // Layout
    plugins.push(builtin_structural(
        "layout",
        "Layout grid rendering (ncol/nrow/custom)",
        FilterMatch {
            attrs: vec![
                "layout-ncol".to_string(),
                "layout-nrow".to_string(),
                "layout".to_string(),
            ],
            ..Default::default()
        },
        vec!["div".to_string()],
        Box::new(LayoutHandler),
    ));

    // Figure div
    plugins.push(builtin_structural(
        "figure-div",
        "Figure div rendering",
        FilterMatch {
            id_prefix: Some("fig-".to_string()),
            ..Default::default()
        },
        vec!["div".to_string()],
        Box::new(FigureDivHandler),
    ));

    // Table div
    plugins.push(builtin_structural(
        "table-div",
        "Table div rendering",
        FilterMatch {
            id_prefix: Some("tbl-".to_string()),
            ..Default::default()
        },
        vec!["div".to_string()],
        Box::new(TableDivHandler),
    ));

    // Theorem
    plugins.push(builtin_filter(
        "theorem",
        "Theorem auto-numbering",
        FilterMatch {
            classes: vec![
                "theorem".into(), "lemma".into(), "corollary".into(),
                "proposition".into(), "conjecture".into(), "definition".into(),
                "example".into(), "exercise".into(), "solution".into(),
                "remark".into(), "algorithm".into(), "proof".into(),
            ],
            ..Default::default()
        },
        vec!["div".to_string()],
        Box::new(crate::filters::TheoremFilter::new()),
    ));

    // Callout
    plugins.push(builtin_filter(
        "callout",
        "Callout enrichment (title, icon, collapse)",
        FilterMatch {
            classes: vec![
                "callout-note".into(), "callout-warning".into(),
                "callout-tip".into(), "callout-caution".into(),
                "callout-important".into(),
            ],
            ..Default::default()
        },
        vec!["div".to_string()],
        Box::new(crate::filters::CalloutFilter::new()),
    ));
}

fn builtin_structural(
    name: &str,
    description: &str,
    match_rule: FilterMatch,
    contexts: Vec<String>,
    handler: Box<dyn StructuralHandler>,
) -> LoadedPlugin {
    LoadedPlugin {
        manifest: PluginManifest {
            name: name.to_string(),
            version: None,
            description: Some(description.to_string()),
            provides: PluginProvides {
                filters: vec![crate::plugin_manifest::FilterSpec {
                    run: None,
                    match_rule,
                    contexts,
                    persistent: false,
                }],
                ..Default::default()
            },
            plugin_dir: PathBuf::new(),
        },
        kind: PluginKind::BuiltinStructural(handler),
    }
}

fn builtin_filter(
    name: &str,
    description: &str,
    match_rule: FilterMatch,
    contexts: Vec<String>,
    filter: Box<dyn Filter>,
) -> LoadedPlugin {
    LoadedPlugin {
        manifest: PluginManifest {
            name: name.to_string(),
            version: None,
            description: Some(description.to_string()),
            provides: PluginProvides {
                filters: vec![crate::plugin_manifest::FilterSpec {
                    run: None,
                    match_rule,
                    contexts,
                    persistent: false,
                }],
                ..Default::default()
            },
            plugin_dir: PathBuf::new(),
        },
        kind: PluginKind::BuiltinFilter(filter),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------


/// Build the JSON payload for a filter subprocess call.
fn build_filter_json(
    context: &str,
    content: &str,
    classes: &[String],
    id: &str,
    format: &str,
    attrs: &HashMap<String, String>,
) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("context".into(), serde_json::Value::String(context.into()));
    map.insert("content".into(), serde_json::Value::String(content.into()));
    map.insert("classes".into(), serde_json::json!(classes));
    map.insert("id".into(), serde_json::Value::String(id.into()));
    map.insert("format".into(), serde_json::Value::String(format.into()));

    // Flatten attrs
    for (k, v) in attrs {
        if !map.contains_key(k) {
            map.insert(k.clone(), serde_json::Value::String(v.clone()));
        }
    }

    serde_json::Value::Object(map)
}
