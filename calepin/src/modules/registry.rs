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
use crate::module_manifest::{MatchRule, MatchSpec, ModuleManifest, ModuleProvides};
use crate::emit::FormatEmitter;
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
    raw_fragments: &'a RefCell<Vec<String>>,
    module_ids: &'a RefCell<HashMap<String, String>>,
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
        raw_fragments: &'a RefCell<Vec<String>>,
        module_ids: &'a RefCell<HashMap<String, String>>,
    ) -> Self {
        Self {
            classes, id, attrs, format, defaults,
            vars: HashMap::new(),
            children, render_fn, raw_fragments, module_ids,
        }
    }

    pub fn children(&self) -> &[Element] { self.children }

    pub fn render_child(&self, element: &Element) -> String {
        (self.render_fn)(element)
    }

    pub fn raw_fragments(&self) -> &RefCell<Vec<String>> {
        self.raw_fragments
    }

    pub fn module_ids(&self) -> &RefCell<HashMap<String, String>> {
        self.module_ids
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

/// Factory that creates a configured FormatEmitter at render time.
/// Emitter configuration (embed_resources, number_sections, etc.) varies
/// per document, so the registry stores a factory rather than an instance.
pub type EmitterFactory = fn(&EmitterConfig) -> Box<dyn FormatEmitter>;

/// Per-render emitter configuration, derived from document metadata.
#[derive(Default)]
pub struct EmitterConfig {
    pub embed_resources: bool,
    pub number_sections: bool,
}

pub enum ModuleKind {
    Element(Box<dyn TransformElement>),
    ElementChildren(Box<dyn TransformElementChildren>),
    Document(Box<dyn TransformDocument>),
    Emitter(EmitterFactory),
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
                        let kind = if let Some(ref script) = manifest.provides.document_script {
                            ModuleKind::Document(Box::new(
                                crate::modules::transform_document::ScriptTransformDocument {
                                    script_path: script.clone(),
                                    module_dir: manifest.module_dir.clone(),
                                }
                            ))
                        } else {
                            ModuleKind::Noop
                        };
                        modules.push(LoadedModule { manifest, kind });
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

    /// Resolve the emitter (FormatEmitter) for the given format name.
    pub fn resolve_emitter(&self, name: &str, config: &EmitterConfig) -> Option<Box<dyn FormatEmitter>> {
        for m in &self.modules {
            if m.manifest.name == name {
                if let ModuleKind::Emitter(factory) = &m.kind {
                    return Some(factory(config));
                }
            }
        }
        None
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

}

// ---------------------------------------------------------------------------
// Built-in module config (parsed from embedded TOML)
// ---------------------------------------------------------------------------

const MODULES_TOML: &str = include_str!("../config/modules.toml");

/// Parsed entry from modules.toml.
struct BuiltinEntry {
    name: String,
    kind: String,
    matchers: Vec<MatchSpec>,
}

fn parse_builtin_entries() -> Vec<BuiltinEntry> {
    let root: toml::Value = toml::from_str(MODULES_TOML)
        .expect("Failed to parse built-in modules.toml");

    let modules = root.get("modules")
        .and_then(|v| v.as_array())
        .expect("modules.toml must contain [[modules]]");

    modules.iter().map(|entry| {
        let name = entry.get("name").and_then(|v| v.as_str())
            .expect("module entry missing 'name'").to_string();
        let kind = entry.get("kind").and_then(|v| v.as_str())
            .expect("module entry missing 'kind'").to_string();

        let matchers = parse_entry_matchers(entry);

        BuiltinEntry { name, kind, matchers }
    }).collect()
}

fn parse_entry_matchers(entry: &toml::Value) -> Vec<MatchSpec> {
    let match_rule = match entry.get("match") {
        Some(m) => MatchRule {
            classes: toml_str_vec(m, "classes"),
            attrs: toml_str_vec(m, "attrs"),
            id_prefix: m.get("id_prefix").and_then(|v| v.as_str()).map(String::from),
            formats: toml_str_vec(m, "formats"),
        },
        None => MatchRule::default(),
    };

    let contexts = {
        let v = toml_str_vec(entry, "contexts");
        if v.is_empty() { return Vec::new(); }
        v
    };

    vec![MatchSpec { run: None, match_rule, contexts }]
}

fn toml_str_vec(node: &toml::Value, key: &str) -> Vec<String> {
    node.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Built-in dispatch: name -> Rust implementation
// ---------------------------------------------------------------------------

/// Resolve a built-in module name to its Rust implementation.
fn resolve_builtin_kind(name: &str, kind_str: &str) -> ModuleKind {
    match (name, kind_str) {
        // Emitters (AST -> format output)
        ("html", "emitter") => ModuleKind::Emitter(|cfg| {
            Box::new(crate::emit::html::HtmlEmitter { embed_resources: cfg.embed_resources })
        }),
        ("latex", "emitter") => ModuleKind::Emitter(|cfg| {
            Box::new(crate::emit::latex::LatexEmitter { number_sections: cfg.number_sections })
        }),
        ("typst", "emitter") => ModuleKind::Emitter(|_| {
            Box::new(crate::emit::typst::TypstEmitter)
        }),
        ("markdown", "emitter") => ModuleKind::Emitter(|_| {
            Box::new(crate::emit::markdown::MarkdownEmitter)
        }),

        // Element children transforms
        ("tabset", "element_children") => ModuleKind::ElementChildren(
            Box::new(BuiltinElementChildren(builtin_element_children_fn::tabset))),
        ("layout", "element_children") => ModuleKind::ElementChildren(
            Box::new(BuiltinElementChildren(builtin_element_children_fn::layout))),
        ("figure", "element_children") => ModuleKind::ElementChildren(
            Box::new(BuiltinElementChildren(builtin_element_children_fn::figure))),
        ("table", "element_children") => ModuleKind::ElementChildren(
            Box::new(BuiltinElementChildren(builtin_element_children_fn::table))),
        ("theorem", "element_children") => ModuleKind::ElementChildren(
            Box::new(BuiltinElementChildren(builtin_element_children_fn::theorem))),
        ("callout", "element_children") => ModuleKind::ElementChildren(
            Box::new(BuiltinElementChildren(builtin_element_children_fn::callout))),

        // Element transforms
        ("convert_svg_pdf", "element") => ModuleKind::Element(
            Box::new(crate::modules::convert_svg_pdf::ConvertSvgPdf)),

        // Document transforms
        ("append_footnotes", "document") => ModuleKind::Document(
            Box::new(crate::modules::append_footnotes::AppendFootnotes)),
        ("split_slides", "document") => ModuleKind::Document(
            Box::new(crate::modules::split_slides::SplitSlides)),
        ("highlight", "document") => ModuleKind::Document(
            Box::new(crate::modules::highlight::transform_page::InjectHighlightMarkup)),
        ("embed_images", "document") => ModuleKind::Document(
            Box::new(crate::modules::embed_images::EmbedImagesHtml)),

        // Noop / partial-only
        (_, "noop") => ModuleKind::Noop,

        (name, kind) => {
            eprintln!("Warning: unknown built-in module '{name}' with kind '{kind}'");
            ModuleKind::Noop
        }
    }
}

// Generic wrapper for element children transforms via function pointer.
struct BuiltinElementChildren(fn(&mut ModuleContext) -> ModuleResult);

impl TransformElementChildren for BuiltinElementChildren {
    fn apply(&self, ctx: &mut ModuleContext) -> ModuleResult {
        (self.0)(ctx)
    }
}

mod builtin_element_children_fn {
    use super::*;

    pub fn tabset(ctx: &mut ModuleContext) -> ModuleResult {
        let output = crate::modules::tabset::render(
            ctx.format, ctx.attrs, ctx.children(), &|el| ctx.render_child(el),
        );
        ModuleResult::Rendered(output)
    }

    pub fn layout(ctx: &mut ModuleContext) -> ModuleResult {
        let output = crate::modules::layout::render(
            ctx.id, ctx.attrs, ctx.children(), ctx.format,
            &|el| ctx.render_child(el), ctx.raw_fragments(), ctx.defaults,
        );
        ModuleResult::Rendered(output)
    }

    pub fn figure(ctx: &mut ModuleContext) -> ModuleResult {
        let output = crate::modules::figure::render(
            ctx.id, ctx.attrs, ctx.children(), ctx.format,
            &|el| ctx.render_child(el), ctx.defaults, ctx.module_ids(),
        );
        ModuleResult::Rendered(output)
    }

    pub fn theorem(ctx: &mut ModuleContext) -> ModuleResult {
        let output = crate::modules::theorem::render(
            ctx.classes, ctx.id, ctx.attrs, ctx.children(), ctx.format,
            &|el| ctx.render_child(el), ctx.defaults, ctx.module_ids(),
        );
        ModuleResult::Rendered(output)
    }

    pub fn table(ctx: &mut ModuleContext) -> ModuleResult {
        let output = crate::modules::table::render(
            ctx.id, ctx.attrs, ctx.children(), ctx.format,
            &|el| ctx.render_child(el), ctx.module_ids(),
        );
        ModuleResult::Rendered(output)
    }

    pub fn callout(ctx: &mut ModuleContext) -> ModuleResult {
        let output = crate::modules::callout::render(
            ctx.classes, ctx.id, ctx.attrs, ctx.children(), ctx.format,
            &|el| ctx.render_child(el), ctx.module_ids(),
        );
        ModuleResult::Rendered(output)
    }
}

// ---------------------------------------------------------------------------
// Built-in registration
// ---------------------------------------------------------------------------

fn register_builtins(modules: &mut Vec<LoadedModule>) {
    for entry in parse_builtin_entries() {
        let kind = resolve_builtin_kind(&entry.name, &entry.kind);
        modules.push(LoadedModule {
            manifest: ModuleManifest {
                name: entry.name,
                version: None,
                description: None,
                provides: ModuleProvides {
                    matchers: entry.matchers,
                    ..Default::default()
                },
                module_dir: PathBuf::new(),
            },
            kind,
        });
    }
}
