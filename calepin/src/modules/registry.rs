//! Plugin registry: loading, indexing, and dispatch.
//!
//! Two transform traits at different scopes:
//!   - `TransformElementRaw` -- operates on raw Element children before rendering
//!   - `TransformElementRendered` -- operates on rendered children + template vars
//!   - `TransformBody` -- operates on the full rendered body string
//!
//! All are registered in the unified `ModuleRegistry`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::modules::transform_body::TransformBody;
use crate::module_manifest::{FilterMatch, FilterSpec, FormatSpec, ModuleManifest, ModuleProvides};
use crate::types::Element;

// ---------------------------------------------------------------------------
// Element transform traits
// ---------------------------------------------------------------------------

/// Result of element transform application.
pub enum ModuleResult {
    /// Transform produced final output. Stops further dispatch.
    Rendered(String),
    /// Transform enriched vars. Continue to next plugin, then template.
    Continue,
    /// Transform does not handle this element.
    Pass,
}

/// Context passed to element transforms during div/span rendering.
pub struct ModuleContext<'a> {
    pub classes: &'a [String],
    pub id: &'a Option<String>,
    pub attrs: &'a HashMap<String, String>,
    pub format: &'a str,
    pub defaults: &'a crate::metadata::Metadata,
    pub vars: HashMap<String, String>,

    children: &'a [Element],
    render_fn: &'a dyn Fn(&Element) -> String,
    #[allow(dead_code)]
    resolve_fn: &'a dyn Fn(&str) -> Option<String>,
    raw_fragments: &'a RefCell<Vec<String>>,
    rendered_cache: RefCell<Option<String>>,
}

impl<'a> ModuleContext<'a> {
    pub fn new(
        classes: &'a [String],
        id: &'a Option<String>,
        attrs: &'a HashMap<String, String>,
        children: &'a [Element],
        format: &'a str,
        defaults: &'a crate::metadata::Metadata,
        render_fn: &'a dyn Fn(&Element) -> String,
        resolve_fn: &'a dyn Fn(&str) -> Option<String>,
        raw_fragments: &'a RefCell<Vec<String>>,
    ) -> Self {
        Self {
            classes, id, attrs, format, defaults,
            vars: HashMap::new(),
            children, render_fn, resolve_fn, raw_fragments,
            rendered_cache: RefCell::new(None),
        }
    }

    pub fn children(&self) -> &[Element] { self.children }

    pub fn render_child(&self, element: &Element) -> String {
        (self.render_fn)(element)
    }

    /// All children rendered and joined (lazy, cached).
    pub fn render_children(&self) -> String {
        let mut cache = self.rendered_cache.borrow_mut();
        if cache.is_none() {
            let rendered = self.children.iter()
                .map(|el| (self.render_fn)(el))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
            *cache = Some(rendered);
        }
        cache.as_ref().unwrap().clone()
    }

    #[allow(dead_code)]
    pub fn resolve_partial(&self, name: &str) -> Option<String> {
        (self.resolve_fn)(name)
    }

    pub fn raw_fragments(&self) -> &RefCell<Vec<String>> {
        self.raw_fragments
    }
}

/// Operates on raw Element children before rendering.
pub trait TransformElementRaw: Send + Sync {
    fn apply(&self, ctx: &mut ModuleContext) -> ModuleResult;
}

/// Operates on rendered children + template vars.
pub trait TransformElementRendered: Send + Sync {
    fn apply(&self, ctx: &mut ModuleContext) -> ModuleResult;
}

// ---------------------------------------------------------------------------
// Plugin kind
// ---------------------------------------------------------------------------

pub enum ModuleKind {
    ElementRaw(Box<dyn TransformElementRaw>),
    ElementRendered(Box<dyn TransformElementRendered>),
    Body(Box<dyn TransformBody>),
    Noop,
}

// ---------------------------------------------------------------------------
// Loaded plugin
// ---------------------------------------------------------------------------

pub struct LoadedModule {
    pub manifest: ModuleManifest,
    pub kind: ModuleKind,
}

// ---------------------------------------------------------------------------
// Plugin registry
// ---------------------------------------------------------------------------

pub struct ModuleRegistry {
    modules: Vec<LoadedModule>,
}

impl ModuleRegistry {
    pub fn load(names: &[String], project_root: &Path) -> Self {
        let mut modules = Vec::new();

        for name in names {
            match crate::paths::resolve_module_dir(name, project_root) {
                Some(dir) => match ModuleManifest::load(&dir) {
                    Ok(manifest) => {
                        modules.push(LoadedModule { manifest, kind: ModuleKind::Noop });
                    }
                    Err(e) => eprintln!("Warning: failed to load module '{}': {}", name, e),
                },
                None => eprintln!("Warning: module '{}' not found", name),
            }
        }

        register_builtins(&mut modules);
        ModuleRegistry { modules }
    }

    pub fn empty() -> Self {
        let mut modules = Vec::new();
        register_builtins(&mut modules);
        ModuleRegistry { modules }
    }

    pub fn matching_filters<'a>(
        &'a self,
        classes: &[String],
        attrs: &HashMap<String, String>,
        id: Option<&str>,
        format: &str,
        context: &str,
    ) -> Vec<(&'a LoadedModule, &'a FilterSpec)> {
        let mut result = Vec::new();
        for plugin in &self.modules {
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

    pub fn resolve_element_partial(&self, name: &str, format: &str) -> Option<String> {
        let canonical = name.replace('-', "_");
        let filename = format!("{}.{}", canonical, format);

        for plugin in &self.modules {
            if let Some(ref spec) = plugin.manifest.provides.elements {
                let path = plugin.manifest.module_dir.join(&spec.dir).join(&filename);
                if path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        return Some(content);
                    }
                }
            }
        }

        crate::paths::resolve_partial(&canonical, format)
            .and_then(|p| std::fs::read_to_string(p).ok())
    }

    #[allow(dead_code)]
    pub fn resolve_page_partial(&self, filename: &str) -> Option<String> {
        for plugin in &self.modules {
            if let Some(ref spec) = plugin.manifest.provides.partials {
                let path = plugin.manifest.module_dir.join(&spec.dir).join(filename);
                if path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        return Some(content);
                    }
                }
            }
        }

        if let Some(dot) = filename.rfind('.') {
            let name = &filename[..dot];
            let ext = &filename[dot + 1..];
            if let Some(path) = crate::paths::resolve_partial(name, ext) {
                if let Ok(content) = std::fs::read_to_string(path) {
                    return Some(content);
                }
            }
        }

        None
    }

    #[allow(dead_code)]
    pub fn resolve_csl(&self) -> Option<PathBuf> {
        for plugin in &self.modules {
            if let Some(ref csl_file) = plugin.manifest.provides.csl {
                let path = plugin.manifest.module_dir.join(csl_file);
                if path.exists() {
                    return Some(path);
                }
            }
        }
        None
    }

    #[allow(dead_code)]
    pub fn resolve_format(&self, name: &str) -> Option<&FormatSpec> {
        self.modules.iter().find_map(|p| {
            p.manifest.provides.format.as_ref()
                .filter(|f| f.name == name)
        })
    }

    pub fn resolve_body_transform(&self, name: &str) -> Option<&dyn TransformBody> {
        for plugin in &self.modules {
            if plugin.manifest.name == name {
                if let ModuleKind::Body(ref t) = plugin.kind {
                    return Some(t.as_ref());
                }
            }
        }
        None
    }

    #[allow(dead_code)]
    pub fn plugins(&self) -> &[LoadedModule] { &self.modules }
}

// ---------------------------------------------------------------------------
// Built-in element transforms
// ---------------------------------------------------------------------------

struct TransformTabset;

impl TransformElementRaw for TransformTabset {
    fn apply(&self, ctx: &mut ModuleContext) -> ModuleResult {
        let output = crate::modules::tabset::render(
            ctx.format, ctx.attrs, ctx.children(), &|el| ctx.render_child(el),
        );
        ModuleResult::Rendered(output)
    }
}

struct TransformLayout;

impl TransformElementRaw for TransformLayout {
    fn apply(&self, ctx: &mut ModuleContext) -> ModuleResult {
        let output = crate::modules::layout::render(
            ctx.id, ctx.attrs, ctx.children(), ctx.format,
            &|el| ctx.render_child(el), ctx.raw_fragments(), ctx.defaults,
        );
        ModuleResult::Rendered(output)
    }
}

struct TransformTheorem {
    counters: std::sync::Mutex<HashMap<String, usize>>,
}

impl TransformTheorem {
    fn new() -> Self {
        Self { counters: std::sync::Mutex::new(HashMap::new()) }
    }
}

impl TransformElementRendered for TransformTheorem {
    fn apply(&self, ctx: &mut ModuleContext) -> ModuleResult {
        for cls in ctx.classes {
            if crate::render::filter::theorem::theorem_prefix(cls).is_some() {
                let mut counters = self.counters.lock().unwrap();
                let count = counters.entry(cls.clone()).or_insert(0);
                *count += 1;
                ctx.vars.insert("number".to_string(), count.to_string());
                ctx.vars.insert("type_class".to_string(), cls.clone());
                return ModuleResult::Continue;
            }
        }
        ModuleResult::Pass
    }
}

// ---------------------------------------------------------------------------
// Built-in registration
// ---------------------------------------------------------------------------

fn register_builtins(modules: &mut Vec<LoadedModule>) {
    // Element transforms (raw)
    modules.push(builtin_element_raw(
        "tabset",
        FilterMatch {
            classes: vec!["panel-tabset".to_string()],
            formats: vec!["html".to_string()],
            ..Default::default()
        },
        vec!["div".to_string()],
        Box::new(TransformTabset),
    ));

    modules.push(builtin_element_raw(
        "layout",
        FilterMatch {
            attrs: vec!["layout_ncol".into(), "layout_nrow".into(), "layout".into()],
            ..Default::default()
        },
        vec!["div".to_string()],
        Box::new(TransformLayout),
    ));

    // Element transforms (rendered)
    modules.push(builtin_element_rendered(
        "theorem",
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
        Box::new(TransformTheorem::new()),
    ));

    // Body transforms
    modules.push(builtin_body_transform("append_footnotes_html",
        Box::new(crate::modules::append_footnotes_html::AppendFootnotesHtml)));
    modules.push(builtin_body_transform("split_slides_html",
        Box::new(crate::modules::split_slides_html::SplitSlidesHtml)));
    modules.push(builtin_body_transform("inject_syntax_css_html",
        Box::new(crate::modules::inject_syntax_css_html::InjectSyntaxCssHtml)));
    modules.push(builtin_body_transform("embed_images_html",
        Box::new(crate::modules::embed_images_html::EmbedImagesHtml)));
    modules.push(builtin_body_transform("inject_color_defs_latex",
        Box::new(crate::modules::inject_color_defs_latex::InjectColorDefsLatex)));
}

fn builtin_element_raw(
    name: &str, match_rule: FilterMatch, contexts: Vec<String>,
    plugin: Box<dyn TransformElementRaw>,
) -> LoadedModule {
    LoadedModule {
        manifest: ModuleManifest {
            name: name.to_string(), version: None, description: None,
            provides: ModuleProvides {
                filters: vec![FilterSpec { run: None, match_rule, contexts }],
                ..Default::default()
            },
            module_dir: PathBuf::new(),
        },
        kind: ModuleKind::ElementRaw(plugin),
    }
}

fn builtin_element_rendered(
    name: &str, match_rule: FilterMatch, contexts: Vec<String>,
    plugin: Box<dyn TransformElementRendered>,
) -> LoadedModule {
    LoadedModule {
        manifest: ModuleManifest {
            name: name.to_string(), version: None, description: None,
            provides: ModuleProvides {
                filters: vec![FilterSpec { run: None, match_rule, contexts }],
                ..Default::default()
            },
            module_dir: PathBuf::new(),
        },
        kind: ModuleKind::ElementRendered(plugin),
    }
}

fn builtin_body_transform(name: &str, transform: Box<dyn TransformBody>) -> LoadedModule {
    LoadedModule {
        manifest: ModuleManifest {
            name: name.to_string(), version: None, description: None,
            provides: ModuleProvides::default(),
            module_dir: PathBuf::new(),
        },
        kind: ModuleKind::Body(transform),
    }
}
