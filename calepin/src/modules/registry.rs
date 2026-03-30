//! Module registry: loading, indexing, and dispatch.
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

/// Span-level transform. Receives attributes and content, returns rendered output.
pub trait TransformSpan: Send + Sync {
    fn render(
        &self,
        attrs: &HashMap<String, String>,
        content: &str,
        format: &str,
        defaults: &crate::config::Metadata,
    ) -> Option<String>;
}

// ---------------------------------------------------------------------------
// Project-level transform (cross-page coordination)
// ---------------------------------------------------------------------------

/// A rendered page with its metadata, passed to project-level transforms.
#[allow(dead_code)]
pub struct RenderedPage {
    /// Source path relative to project root.
    pub source: PathBuf,
    /// Output path relative to output directory.
    pub output: PathBuf,
    /// Rendered body (HTML, LaTeX, etc.).
    pub body: String,
    /// Page title (from front matter or first heading).
    pub title: Option<String>,
    /// Page date (from front matter).
    pub date: Option<String>,
    /// Page subtitle.
    pub subtitle: Option<String>,
    /// Abstract text.
    pub abstract_text: Option<String>,
    /// URL path for this page (e.g., "/guides/intro.html").
    pub url: String,
    /// Table of contents HTML (if generated).
    pub toc: Option<String>,
    /// Page language (for multilingual sites).
    pub lang: Option<String>,
    /// Full page metadata.
    pub metadata: crate::config::Metadata,
}

impl RenderedPage {
    /// Convert to a DocumentInfo for collection functions that need it.
    pub fn to_document_info(&self) -> crate::collection::discover::DocumentInfo {
        crate::collection::discover::DocumentInfo {
            source: self.source.clone(),
            output: self.output.clone(),
            url: self.url.clone(),
            meta: crate::collection::discover::DocumentMeta {
                title: self.title.clone(),
                date: self.date.clone(),
                subtitle: self.subtitle.clone(),
                description: None,
                image: None,
                r#abstract: self.abstract_text.clone(),
                listing: None,
                lang: self.lang.clone(),
                translations: None,
                var: std::collections::HashMap::new(),
            },
            lang: self.lang.clone(),
        }
    }

    /// Convert to a CollectionRenderResult.
    pub fn to_render_result(&self) -> crate::collection::render::CollectionRenderResult {
        crate::collection::render::CollectionRenderResult {
            body: self.body.clone(),
            toc: self.toc.clone(),
            title: self.title.clone(),
            date: self.date.clone(),
            subtitle: self.subtitle.clone(),
            abstract_text: self.abstract_text.clone(),
        }
    }
}

/// Runtime context for project-level transforms.
pub struct ProjectTransformContext {
    /// Project root directory.
    pub base_dir: PathBuf,
    /// Output directory.
    pub output_dir: PathBuf,
    /// Active target name (e.g., "website", "book").
    pub target_name: String,
    /// Whether to use portable (relative) URLs.
    pub portable: bool,
    /// Whether running in serve mode.
    pub serve: bool,
}

/// Project-level transform. Operates on all rendered pages at once.
/// Used for cross-page coordination: navigation, cross-references, site wrapping.
pub trait TransformProject: Send + Sync {
    fn transform(
        &self,
        pages: &mut Vec<RenderedPage>,
        config: &crate::config::Metadata,
        writer: &str,
        ctx: &ProjectTransformContext,
    ) -> anyhow::Result<()>;
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
    Span(Box<dyn TransformSpan>),
    Document(Box<dyn TransformDocument>),
    Project(Box<dyn TransformProject>),
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
                None => {
                    // Don't warn for built-in modules or extension-declared modules
                    let builtin_names = builtin_module_names();
                    let is_builtin = builtin_names.iter().any(|b| b == name);
                    let is_extension = is_extension_module(name, project_root);
                    if !is_builtin && !is_extension {
                        eprintln!("Warning: module '{}' not found", name);
                    }
                }
            }
        }

        // Load external modules from installed extensions
        load_extension_modules(&mut modules, project_root);

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

    pub fn resolve_element_template(&self, name: &str, format: &str) -> Option<String> {
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

        crate::paths::resolve_template(&canonical, format)
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

    /// Resolve all project-level transforms from the active module list.
    pub fn resolve_project_transforms(&self, active: &[String]) -> Vec<&dyn TransformProject> {
        let mut result = Vec::new();
        for m in &self.modules {
            if active.contains(&m.manifest.name) {
                if let ModuleKind::Project(ref t) = m.kind {
                    result.push(t.as_ref());
                }
            }
        }
        result
    }

    /// Run all active project-level transforms on the given pages.
    /// Returns true if any transforms ran.
    pub fn run_project_transforms(
        &self,
        pages: &mut Vec<RenderedPage>,
        metadata: &crate::config::Metadata,
        writer: &str,
        ctx: &ProjectTransformContext,
        module_names: &[String],
    ) -> anyhow::Result<bool> {
        let transforms = self.resolve_project_transforms(module_names);
        if transforms.is_empty() {
            return Ok(false);
        }
        for transform in &transforms {
            transform.transform(pages, metadata, writer, ctx)?;
        }
        Ok(true)
    }

}

// ---------------------------------------------------------------------------
// Built-in module names and cross-reference prefixes
// ---------------------------------------------------------------------------

/// All cross-reference prefix-to-label mappings, collected from modules.
/// Used by bibliography (to exclude from citation lookup), crossref (to
/// resolve/renumber), and div validation (to check prefix ownership).
pub fn all_crossref_prefixes() -> Vec<(&'static str, &'static str)> {
    let mut prefixes = vec![
        // Core (not module-owned)
        ("fig", "Figure"),
        ("tbl", "Table"),
        ("eq", "Equation"),
        ("sec", "Section"),
        ("lst", "Listing"),
    ];
    // Theorem module
    for &(_, prefix) in crate::modules::theorem::THEOREM_PREFIXES {
        let label = match prefix {
            "thm" => "Theorem", "lem" => "Lemma", "cor" => "Corollary",
            "prp" => "Proposition", "cnj" => "Conjecture", "def" => "Definition",
            "exm" => "Example", "exr" => "Exercise", "sol" => "Solution",
            "rem" => "Remark", "alg" => "Algorithm",
            _ => "",
        };
        prefixes.push((prefix, label));
    }
    // Callout module
    for &(_, prefix) in crate::modules::callout::CALLOUT_PREFIXES {
        let label = match prefix {
            "tip" => "Tip", "nte" => "Note", "wrn" => "Warning",
            "imp" => "Important", "cau" => "Caution",
            _ => "",
        };
        prefixes.push((prefix, label));
    }
    prefixes
}

/// All built-in module names (from modules.toml). Used for path validation
/// to skip filesystem checks for built-in modules.
pub fn builtin_module_names() -> Vec<String> {
    parse_builtin_entries().into_iter().map(|e| e.name).collect()
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
        ("document_listing", "element_children") => ModuleKind::ElementChildren(
            Box::new(BuiltinElementChildren(builtin_element_children_fn::document_listing))),
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

        // Span transforms
        ("pagebreak", "span") => ModuleKind::Span(
            Box::new(BuiltinSpan(builtin_span_fn::pagebreak))),
        ("video", "span") => ModuleKind::Span(
            Box::new(BuiltinSpan(builtin_span_fn::video))),
        ("placeholder", "span") => ModuleKind::Span(
            Box::new(BuiltinSpan(builtin_span_fn::placeholder))),
        ("lorem", "span") => ModuleKind::Span(
            Box::new(BuiltinSpan(builtin_span_fn::lorem))),

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

        // Project-level transforms (cross-page coordination)
        (name, "project") => {
            if let Some(module) = crate::modules::project_modules::resolve_builtin_project(name) {
                ModuleKind::Project(module)
            } else {
                eprintln!("Warning: unknown project module '{name}'");
                ModuleKind::Noop
            }
        }

        // Noop / template-only
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

    pub fn document_listing(ctx: &mut ModuleContext) -> ModuleResult {
        let output = crate::modules::document_listing::render(ctx);
        ModuleResult::Rendered(output)
    }

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
            &|el| ctx.render_child(el), ctx.module_ids(),
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

// Generic wrapper for span transforms via function pointer.
struct BuiltinSpan(fn(&HashMap<String, String>, &str, &str, &crate::config::Metadata) -> Option<String>);

impl TransformSpan for BuiltinSpan {
    fn render(&self, attrs: &HashMap<String, String>, content: &str, format: &str,
              defaults: &crate::config::Metadata) -> Option<String> {
        (self.0)(attrs, content, format, defaults)
    }
}

mod builtin_span_fn {
    use std::collections::HashMap;

    pub fn pagebreak(_attrs: &HashMap<String, String>, _content: &str, format: &str,
                     _defaults: &crate::config::Metadata) -> Option<String> {
        Some(crate::modules::pagebreak::render(format))
    }

    pub fn video(attrs: &HashMap<String, String>, _content: &str, format: &str,
                 defaults: &crate::config::Metadata) -> Option<String> {
        Some(crate::modules::video::render(attrs, format, defaults))
    }

    pub fn placeholder(attrs: &HashMap<String, String>, _content: &str, format: &str,
                       defaults: &crate::config::Metadata) -> Option<String> {
        Some(crate::modules::placeholder::render(attrs, format, defaults))
    }

    pub fn lorem(attrs: &HashMap<String, String>, _content: &str, _format: &str,
                 defaults: &crate::config::Metadata) -> Option<String> {
        Some(crate::modules::lorem::render(attrs, defaults))
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

/// Check if a module name is declared in any installed extension.
fn is_extension_module(name: &str, project_root: &Path) -> bool {
    let extensions_dir = crate::paths::extensions_dir(project_root);
    if !extensions_dir.is_dir() { return false; }
    if let Ok(entries) = std::fs::read_dir(&extensions_dir) {
        for entry in entries.flatten() {
            let ext_dir = entry.path();
            if let Some(manifest) = crate::config::extension::load_cached(&ext_dir) {
                if manifest.modules.iter().any(|m| m.name == name) {
                    return true;
                }
            }
        }
    }
    false
}

/// Load external modules from installed extensions.
///
/// Scans the active target's extension chain and any side-loaded extensions
/// for `[[modules]]` entries with `run` fields. Creates the appropriate
/// external transform (script or WASM) for each.
fn load_extension_modules(modules: &mut Vec<LoadedModule>, project_root: &Path) {
    let extensions_dir = crate::paths::extensions_dir(project_root);
    if !extensions_dir.is_dir() {
        return;
    }

    // Collect extension names: active target chain + side-loaded
    let target = crate::paths::get_active_target().unwrap_or_default();
    let mut ext_names = crate::config::extension::chain_names(&project_root, &target);
    let mut seen: std::collections::HashSet<String> = ext_names.iter().cloned().collect();
    for name in crate::paths::get_sideloaded_extensions() {
        if seen.insert(name.clone()) {
            ext_names.push(name);
        }
    }

    // Check each extension for modules with `run` fields
    for ext_name in &ext_names {
        let ext_dir = extensions_dir.join(ext_name);
        let ext_dir_ref = &ext_dir;
        let manifest = match crate::config::extension::load_cached(ext_dir_ref) {
            Some(m) => m,
            None => {
                let manifest_path = ext_dir.join("extension.toml");
                if manifest_path.exists() {
                    eprintln!("Warning: failed to read or parse {}", manifest_path.display());
                }
                continue;
            }
        };

        // Build vars from extension manifest
        let vars_json = if manifest.vars.is_empty() {
            serde_json::Value::Object(serde_json::Map::new())
        } else {
            let mut map = serde_json::Map::new();
            for (k, v) in &manifest.vars {
                map.insert(k.clone(), serde_json::to_value(v).unwrap_or_default());
            }
            serde_json::Value::Object(map)
        };

        for module_decl in &manifest.modules {
            let Some(ref run_path) = module_decl.run else { continue };
            let script = ext_dir.join("scripts").join(run_path);
            if !script.exists() {
                // Also check directly in the extension dir
                let alt = ext_dir.join(run_path);
                if !alt.exists() {
                    continue;
                }
            }
            let script_path = if ext_dir.join("scripts").join(run_path).exists() {
                ext_dir.join("scripts").join(run_path)
            } else {
                ext_dir.join(run_path)
            };
            // Ensure absolute path for script execution
            let script_path = if script_path.is_relative() {
                std::env::current_dir().unwrap_or_default().join(&script_path)
            } else {
                script_path
            };
            let ext_dir_abs = if ext_dir.is_relative() {
                std::env::current_dir().unwrap_or_default().join(&ext_dir)
            } else {
                ext_dir.clone()
            };

            let kind = match module_decl.kind.as_str() {
                "document" => {
                    if module_decl.protocol == "text" {
                        ModuleKind::Document(Box::new(
                            crate::modules::transform_document::ScriptTransformDocument {
                                script_path: script_path.clone(),
                                module_dir: ext_dir_abs.clone(),
                            }
                        ))
                    } else {
                        ModuleKind::Document(Box::new(
                            crate::modules::external::JsonDocumentTransform {
                                name: module_decl.name.clone(),
                                script_path: script_path.clone(),
                                module_dir: ext_dir_abs.clone(),
                                vars: vars_json.clone(),
                            }
                        ))
                    }
                }
                "project" => {
                    ModuleKind::Project(Box::new(
                        crate::modules::external::ExternalProjectTransform {
                            name: module_decl.name.clone(),
                            script_path: script_path.clone(),
                            module_dir: ext_dir_abs.clone(),
                            protocol: module_decl.protocol.clone(),
                            vars: vars_json.clone(),
                        }
                    ))
                }
                "element_children" => {
                    ModuleKind::ElementChildren(Box::new(
                        crate::modules::external::ExternalElementChildrenTransform {
                            name: module_decl.name.clone(),
                            script_path: script_path.clone(),
                            module_dir: ext_dir_abs.clone(),
                            vars: vars_json.clone(),
                        }
                    ))
                }
                _ => continue, // span/element not yet supported as external
            };

            // Create manifest with match rules from extension declaration
            let matchers = if let Some(ref rule) = module_decl.match_rule {
                vec![crate::module_manifest::MatchSpec {
                    run: None,
                    match_rule: crate::module_manifest::MatchRule {
                        classes: rule.classes.clone(),
                        attrs: rule.attrs.clone(),
                        id_prefix: rule.id_prefix.clone(),
                        formats: rule.writers.clone(),
                    },
                    contexts: module_decl.contexts.clone(),
                }]
            } else {
                Vec::new()
            };
            let mod_manifest = ModuleManifest {
                name: module_decl.name.clone(),
                version: None,
                description: Some(module_decl.description.clone()),
                provides: crate::module_manifest::ModuleProvides {
                    matchers,
                    ..Default::default()
                },
                module_dir: ext_dir_abs.clone(),
            };
            modules.push(LoadedModule { manifest: mod_manifest, kind });
        }
    }
}
