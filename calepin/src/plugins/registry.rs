//! Plugin registry: loading, indexing, and dispatch.
//!
//! The `PluginRegistry` is the single entry point for all plugin-related
//! operations: built-in structural handlers (tabset, layout, figure-div),
//! built-in filters (theorem, callout), element/page templates, CSL styles,
//! and custom format definitions.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

}

// ---------------------------------------------------------------------------
// Loaded plugin — manifest + runtime handle
// ---------------------------------------------------------------------------

pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub kind: PluginKind,
}

// ---------------------------------------------------------------------------
// Plugin registry
// ---------------------------------------------------------------------------

pub struct PluginRegistry {
    plugins: Vec<LoadedPlugin>,
}

impl PluginRegistry {
    /// Create a new registry from the front matter plugin list.
    /// Resolves plugins relative to `project_root`.
    /// Loads user plugins first (in order), then appends built-in plugins.
    pub fn load(names: &[String], project_root: &Path) -> Self {
        let mut plugins = Vec::new();

        // Load user plugins (templates, elements, CSL only -- no subprocess execution)
        for name in names {
            match crate::paths::resolve_plugin_dir(name, project_root) {
                Some(dir) => match PluginManifest::load(&dir) {
                    Ok(manifest) => {
                        // User plugins provide templates/elements/CSL but don't execute code.
                        // The PluginKind doesn't matter for template-only plugins; we use
                        // a dummy BuiltinFilter with a no-op filter that always passes.
                        plugins.push(LoadedPlugin {
                            manifest,
                            kind: PluginKind::BuiltinFilter(Box::new(NoopFilter)),
                        });
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

        // Three-layer resolution: project elements/ → theme elements/ → built-in
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

        // Three-layer resolution if filename has a dot (e.g., "calepin.html")
        if let Some(dot) = filename.rfind('.') {
            let name = &filename[..dot];
            let ext = &filename[dot + 1..];
            if let Some(path) = crate::paths::resolve_template(name, ext) {
                if let Ok(content) = std::fs::read_to_string(path) {
                    return Some(content);
                }
            }
        }

        None
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
                "layout_ncol".to_string(),
                "layout_nrow".to_string(),
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
                }],
                ..Default::default()
            },
            plugin_dir: PathBuf::new(),
        },
        kind: PluginKind::BuiltinFilter(filter),
    }
}

// ---------------------------------------------------------------------------
// Noop filter for user plugins (template/element providers only)
// ---------------------------------------------------------------------------

struct NoopFilter;

impl Filter for NoopFilter {
    fn apply(
        &self,
        _element: &Element,
        _format: &str,
        _vars: &mut HashMap<String, String>,
    ) -> crate::filters::FilterResult {
        crate::filters::FilterResult::Pass
    }
}
