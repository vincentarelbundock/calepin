//! Plugin registry: loading, indexing, and dispatch.
//!
//! Two transform traits at different scopes:
//!   - `TransformElement` -- operates on raw Element children before rendering
//!   - `TransformElementRendered` -- operates on rendered children + template vars
//!   - `TransformBody` -- operates on the full rendered body string
//!
//! All are registered in the unified `ModuleRegistry`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::modules::transform_document::TransformDocument;
use crate::module_manifest::{MatchRule, MatchSpec, FormatSpec, ModuleManifest, ModuleProvides};
use crate::types::Element;

// ---------------------------------------------------------------------------
// Element transform traits
// ---------------------------------------------------------------------------

/// Result of element transform application.
#[allow(dead_code)]
pub enum ModuleResult {
    /// Transform produced final output. Stops further dispatch.
    Rendered(String),
    /// Transform enriched vars. Continue to next plugin, then template.
    Continue,
    /// Transform does not handle this element.
    Pass,
}

/// Context passed to element transforms during div/span rendering.
#[allow(dead_code)]
pub struct ModuleContext<'a> {
    pub classes: &'a [String],
    pub id: &'a Option<String>,
    pub attrs: &'a HashMap<String, String>,
    pub format: &'a str,
    pub defaults: &'a crate::config::Metadata,
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
        defaults: &'a crate::config::Metadata,
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
    #[allow(dead_code)]
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

/// Pre-render mutation of individual elements. Called once per element
/// (including nested children) before rendering starts.
pub trait TransformElement: Send + Sync {
    fn transform(&self, element: &mut Element);
}

/// Per-div structural transform during rendering. Receives raw children.
pub trait TransformElementChildren: Send + Sync {
    fn apply(&self, ctx: &mut ModuleContext) -> ModuleResult;
}

// ---------------------------------------------------------------------------
// Module kind
// ---------------------------------------------------------------------------

pub enum ModuleKind {
    Element(Box<dyn TransformElement>),
    ElementChildren(Box<dyn TransformElementChildren>),
    Document(Box<dyn TransformDocument>),
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

    pub fn matching_modules<'a>(
        &'a self,
        classes: &[String],
        attrs: &HashMap<String, String>,
        id: Option<&str>,
        format: &str,
        context: &str,
    ) -> Vec<(&'a LoadedModule, &'a MatchSpec)> {
        let mut result = Vec::new();
        for plugin in &self.modules {
            for spec in &plugin.manifest.provides.matchers {
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

    /// Collect all element preparers from active modules.
    pub fn resolve_transform_element(&self, active: &[String]) -> Vec<&dyn TransformElement> {
        let mut result = Vec::new();
        for m in &self.modules {
            if active.contains(&m.manifest.name) {
                if let ModuleKind::Element(ref t) = m.kind {
                    result.push(t.as_ref());
                }
            }
        }
        result
    }

    /// Collect all document transforms from active modules.
    pub fn resolve_document_transforms(&self, active: &[String]) -> Vec<&dyn TransformDocument> {
        let mut result = Vec::new();
        for m in &self.modules {
            if active.contains(&m.manifest.name) {
                if let ModuleKind::Document(ref t) = m.kind {
                    result.push(t.as_ref());
                }
            }
        }
        result
    }

    #[allow(dead_code)]
    pub fn modules(&self) -> &[LoadedModule] { &self.modules }
}

// ---------------------------------------------------------------------------
// Built-in element transforms
// ---------------------------------------------------------------------------

struct TransformTabset;

impl TransformElementChildren for TransformTabset {
    fn apply(&self, ctx: &mut ModuleContext) -> ModuleResult {
        let output = crate::modules::tabset::render(
            ctx.format, ctx.attrs, ctx.children(), &|el| ctx.render_child(el),
        );
        ModuleResult::Rendered(output)
    }
}

struct TransformLayout;

impl TransformElementChildren for TransformLayout {
    fn apply(&self, ctx: &mut ModuleContext) -> ModuleResult {
        let output = crate::modules::layout::render(
            ctx.id, ctx.attrs, ctx.children(), ctx.format,
            &|el| ctx.render_child(el), ctx.raw_fragments(), ctx.defaults,
        );
        ModuleResult::Rendered(output)
    }
}

struct NoopElementChildren;
impl TransformElementChildren for NoopElementChildren {
    fn apply(&self, _ctx: &mut ModuleContext) -> ModuleResult {
        ModuleResult::Pass
    }
}

// ---------------------------------------------------------------------------
// Built-in registration
// ---------------------------------------------------------------------------

fn register_builtins(modules: &mut Vec<LoadedModule>) {
    // Element children transforms (per-div structural rewriting)
    modules.push(builtin_element_children(
        "tabset",
        MatchRule {
            classes: vec!["panel-tabset".to_string()],
            formats: vec!["html".to_string()],
            ..Default::default()
        },
        vec!["div".to_string()],
        Box::new(TransformTabset),
    ));

    modules.push(builtin_element_children(
        "layout",
        MatchRule {
            attrs: vec!["layout_ncol".into(), "layout_nrow".into(), "layout".into()],
            ..Default::default()
        },
        vec!["div".to_string()],
        Box::new(TransformLayout),
    ));

    // Auto-numbered elements (number=true in match rule, no transform code)
    modules.push(builtin_element_children(
        "theorem",
        MatchRule {
            classes: vec![
                "theorem".into(), "lemma".into(), "corollary".into(),
                "proposition".into(), "conjecture".into(), "definition".into(),
                "example".into(), "exercise".into(), "solution".into(),
                "remark".into(), "algorithm".into(), "proof".into(),
            ],
            number: true,
            ..Default::default()
        },
        vec!["div".to_string()],
        Box::new(NoopElementChildren),
    ));

    // Prepare elements (pre-render mutations)
    modules.push(builtin_transform_element("convert_svg_pdf",
        Box::new(crate::modules::convert_svg_pdf::ConvertSvgPdf)));

    // Document transforms
    modules.push(builtin_document_transform("append_footnotes",
        Box::new(crate::modules::append_footnotes::AppendFootnotes)));
    modules.push(builtin_document_transform("split_slides",
        Box::new(crate::modules::split_slides::SplitSlides)));
    modules.push(builtin_document_transform("highlight",
        Box::new(crate::modules::highlight::transform_page::InjectHighlightMarkup)));
    modules.push(builtin_document_transform("embed_images",
        Box::new(crate::modules::embed_images::EmbedImagesHtml)));
}

fn builtin_element_children(
    name: &str, match_rule: MatchRule, contexts: Vec<String>,
    plugin: Box<dyn TransformElementChildren>,
) -> LoadedModule {
    LoadedModule {
        manifest: ModuleManifest {
            name: name.to_string(), version: None, description: None,
            provides: ModuleProvides {
                matchers: vec![MatchSpec { run: None, match_rule, contexts }],
                ..Default::default()
            },
            module_dir: PathBuf::new(),
        },
        kind: ModuleKind::ElementChildren(plugin),
    }
}

fn builtin_transform_element(name: &str, transform: Box<dyn TransformElement>) -> LoadedModule {
    LoadedModule {
        manifest: ModuleManifest {
            name: name.to_string(), version: None, description: None,
            provides: ModuleProvides::default(),
            module_dir: PathBuf::new(),
        },
        kind: ModuleKind::Element(transform),
    }
}

fn builtin_document_transform(name: &str, transform: Box<dyn TransformDocument>) -> LoadedModule {
    LoadedModule {
        manifest: ModuleManifest {
            name: name.to_string(), version: None, description: None,
            provides: ModuleProvides::default(),
            module_dir: PathBuf::new(),
        },
        kind: ModuleKind::Document(transform),
    }
}
