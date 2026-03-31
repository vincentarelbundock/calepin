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
use crate::config::extension::ExtensionMatchRule;
use crate::emit::FormatEmitter;
use crate::types::Element;

/// A module's match rule paired with its contexts.
pub struct ModuleMatcher {
    pub match_rule: ExtensionMatchRule,
    pub contexts: Vec<String>,
}

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
    /// Per-page document metadata (from frontmatter discovery).
    pub doc_meta: crate::collection::discover::DocumentMeta,
    /// Full page metadata (collection-level config merged with per-page).
    pub metadata: crate::config::Metadata,
}

impl RenderedPage {
    /// Convert to a DocumentInfo for collection functions that need it.
    ///
    /// Uses the original per-page DocumentMeta, with title/date/subtitle
    /// updated from the render result (which may resolve them from headings, etc.).
    pub fn to_document_info(&self) -> crate::collection::discover::DocumentInfo {
        let mut meta = self.doc_meta.clone();
        // Render results may update these (e.g., title from first heading)
        if self.title.is_some() { meta.title = self.title.clone(); }
        if self.date.is_some() { meta.date = self.date.clone(); }
        if self.subtitle.is_some() { meta.subtitle = self.subtitle.clone(); }
        if self.abstract_text.is_some() { meta.r#abstract = self.abstract_text.clone(); }
        crate::collection::discover::DocumentInfo {
            source: self.source.clone(),
            output: self.output.clone(),
            url: self.url.clone(),
            lang: self.lang.clone(),
            meta,
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
/// Emitter configuration (standalone, number_sections, etc.) varies
/// per document, so the registry stores a factory rather than an instance.
pub type EmitterFactory = fn(&EmitterConfig) -> Box<dyn FormatEmitter>;

/// Per-render emitter configuration, derived from document metadata.
#[derive(Default)]
pub struct EmitterConfig {
    pub standalone: bool,
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
    pub name: String,
    pub matchers: Vec<ModuleMatcher>,
    pub kind: ModuleKind,
}

// ---------------------------------------------------------------------------
// Module registry
// ---------------------------------------------------------------------------

pub struct ModuleRegistry {
    modules: Vec<LoadedModule>,
}

impl ModuleRegistry {
    pub fn load(names: &[String], project_root: &Path) -> Self {
        let mut modules = register_builtins();

        // Load external modules from installed extensions
        load_extension_modules(&mut modules, project_root);

        // Warn about unknown module names
        for name in names {
            let known = modules.iter().any(|m| m.name == *name)
                || is_extension_module(name, project_root);
            if !known {
                eprintln!("Warning: module '{}' not found", name);
            }
        }

        ModuleRegistry { modules }
    }

    pub fn empty() -> Self {
        ModuleRegistry { modules: register_builtins() }
    }

    pub fn matching_modules<'a>(
        &'a self,
        classes: &[String],
        attrs: &HashMap<String, String>,
        id: Option<&str>,
        format: &str,
        context: &str,
    ) -> Vec<(&'a LoadedModule, &'a ModuleMatcher)> {
        let mut result = Vec::new();
        for module in &self.modules {
            for matcher in &module.matchers {
                if matcher.contexts.iter().any(|c| c == context)
                    && matcher.match_rule.matches(classes, attrs, id, format)
                {
                    result.push((module, matcher));
                }
            }
        }
        result
    }

    pub fn resolve_element_template(&self, name: &str, format: &str) -> Option<String> {
        let canonical = name.replace('-', "_");
        crate::paths::resolve_template(&canonical, format)
            .and_then(|p| std::fs::read_to_string(p).ok())
    }

    pub fn resolve_emitter(&self, name: &str, config: &EmitterConfig) -> Option<Box<dyn FormatEmitter>> {
        for m in &self.modules {
            if m.name == name {
                if let ModuleKind::Emitter(factory) = &m.kind {
                    return Some(factory(config));
                }
            }
        }
        None
    }

    pub fn resolve_transform_element(&self, active: &[String]) -> Vec<&dyn TransformElement> {
        self.modules.iter()
            .filter(|m| active.contains(&m.name))
            .filter_map(|m| match &m.kind { ModuleKind::Element(t) => Some(t.as_ref()), _ => None })
            .collect()
    }

    pub fn resolve_document_transforms(&self, active: &[String]) -> Vec<&dyn TransformDocument> {
        self.modules.iter()
            .filter(|m| active.contains(&m.name))
            .filter_map(|m| match &m.kind { ModuleKind::Document(t) => Some(t.as_ref()), _ => None })
            .collect()
    }

    pub fn run_project_transforms(
        &self,
        pages: &mut Vec<RenderedPage>,
        metadata: &crate::config::Metadata,
        writer: &str,
        ctx: &ProjectTransformContext,
        module_names: &[String],
    ) -> anyhow::Result<bool> {
        let transforms: Vec<_> = self.modules.iter()
            .filter(|m| module_names.contains(&m.name))
            .filter_map(|m| match &m.kind { ModuleKind::Project(t) => Some(t.as_ref()), _ => None })
            .collect();
        if transforms.is_empty() { return Ok(false); }
        for t in &transforms { t.transform(pages, metadata, writer, ctx)?; }
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
    prefixes
}

// ---------------------------------------------------------------------------
// Built-in module registration (from extension.toml manifest)
// ---------------------------------------------------------------------------

const MODULES_MANIFEST: &str = include_str!("extension.toml");

/// All built-in module names. Used for path validation.
pub fn builtin_module_names() -> Vec<String> {
    let manifest: crate::config::extension::ExtensionManifest =
        toml::from_str(MODULES_MANIFEST).expect("Failed to parse modules extension.toml");
    let mut names: Vec<String> = manifest.modules.into_iter().map(|m| m.name).collect();
    names.extend(["html", "latex", "typst", "markdown"].iter().map(|s| s.to_string()));
    names
}

// ---------------------------------------------------------------------------
// Built-in dispatch: name -> Rust implementation
// ---------------------------------------------------------------------------

/// Resolve a built-in module name to its Rust implementation.
fn resolve_builtin_kind(name: &str, kind_str: &str) -> ModuleKind {
    match (name, kind_str) {
        // Emitters (AST -> format output)
        ("html", "emitter") => ModuleKind::Emitter(|cfg| {
            Box::new(crate::emit::html::HtmlEmitter { standalone: cfg.standalone })
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

        // Span transforms
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

fn register_builtins() -> Vec<LoadedModule> {
    use crate::config::extension::ExtensionManifest;

    let manifest: ExtensionManifest = toml::from_str(MODULES_MANIFEST)
        .expect("Failed to parse built-in modules extension.toml");

    let mut modules: Vec<LoadedModule> = manifest.modules.into_iter().map(|m| {
        let matchers = if m.contexts.is_empty() {
            Vec::new()
        } else {
            vec![ModuleMatcher {
                match_rule: m.match_rule.unwrap_or_default(),
                contexts: m.contexts,
            }]
        };
        let kind = resolve_builtin_kind(&m.name, &m.kind);
        LoadedModule { name: m.name, matchers, kind }
    }).collect();

    // Emitters are format-specific, not user-facing modules.
    for name in ["html", "latex", "typst", "markdown"] {
        let kind = resolve_builtin_kind(name, "emitter");
        modules.push(LoadedModule { name: name.to_string(), matchers: Vec::new(), kind });
    }

    modules
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

            let matchers = if let Some(ref rule) = module_decl.match_rule {
                vec![ModuleMatcher {
                    match_rule: rule.clone(),
                    contexts: module_decl.contexts.clone(),
                }]
            } else {
                Vec::new()
            };
            modules.push(LoadedModule {
                name: module_decl.name.clone(),
                matchers,
                kind,
            });
        }
    }
}
