//! Element renderer: thin dispatcher that delegates to filters.

use std::collections::HashMap;
use std::rc::Rc;

use include_dir::{include_dir, Dir};

use crate::types::Element;
use crate::render::vars::BuildElementVars;
use crate::registry::ModuleRegistry;
use crate::modules::Highlighter;

// ---------------------------------------------------------------------------
// Built-in project tree (embedded at compile time)
// ---------------------------------------------------------------------------

/// Built-in partials (element/page templates), embedded at compile time.
pub static BUILTIN_PARTIALS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/partials");

/// Built-in assets (CSS, JS, scaffold files), embedded at compile time.
pub static BUILTIN_ASSETS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/scaffold");

/// Template name aliases: multiple names can map to the same template file.
fn resolve_partial_alias(name: &str) -> &str {
    match name {
        // Code diagnostics share one template
        "code_error" | "code_warning" | "code_message" => "code_diagnostic",
        // Everything else: name matches file directly
        other => other,
    }
}

/// Look up a built-in template by name and base.
/// Checks `partials/{target}/{name}.{ext}` (active target, if different from base),
/// then `partials/{base}/{name}.{ext}`, then `templates/common/{name}.jinja`.
pub fn resolve_builtin_partial(name: &str, base: &str) -> Option<&'static str> {
    let resolved = resolve_partial_alias(name);
    let ext = crate::paths::resolve_extension(base);

    // Target-specific (e.g., book/page.typ)
    if let Some(target) = crate::paths::get_active_target() {
        if target != base {
            let target_path = format!("{}/{}.{}", target, resolved, ext);
            if let Some(file) = BUILTIN_PARTIALS.get_file(&target_path) {
                return file.contents_utf8();
            }
        }
    }

    // Base-specific (e.g., html/figure.html)
    let base_path = format!("{}/{}.{}", base, resolved, ext);
    if let Some(file) = BUILTIN_PARTIALS.get_file(&base_path) {
        return file.contents_utf8();
    }

    // Generic .jinja
    let common_path = format!("common/{}.jinja", resolved);
    BUILTIN_PARTIALS.get_file(&common_path).and_then(|f| f.contents_utf8())
}

// ---------------------------------------------------------------------------
// ElementRenderer
// ---------------------------------------------------------------------------

pub struct ElementRenderer {
    ext: String,
    /// Pre-compiled template environment. Element templates are parsed and
    /// compiled once at construction time rather than on every render() call.
    /// This avoids repeated parsing overhead (~3 us/call) for templates that
    /// may be rendered hundreds of times per document (code chunks, divs, etc.).
    template_env: crate::render::template::TemplateEnv,
    highlighter: Highlighter,
    registry: Rc<ModuleRegistry>,
    raw_fragments: std::cell::RefCell<Vec<String>>,
    preamble: Vec<String>,
    /// Resolved rendering metadata (highlight, figure, callout, labels, etc.).
    pub metadata: crate::config::Metadata,
    pub number_sections: bool,
    pub shift_headings: bool,
    pub convert_math: bool,
    pub default_fig_cap_location: Option<String>,
    /// Chapter number for collection pages. When set, section counters start
    /// at [chapter, 0, 0, 0, 0, 0] so sections get chapter-prefixed numbers.
    pub chapter_number: Option<usize>,
    /// IDs registered by modules during rendering (e.g. "thm-cauchy" -> "1").
    /// Used by the cross-ref system to resolve `@id` references.
    pub module_ids: std::cell::RefCell<HashMap<String, String>>,
    /// Accumulated walk metadata (headings, IDs) from all Text element renders.
    pub walk_metadata: std::cell::RefCell<crate::emit::WalkMetadata>,
    /// Footnote state: counter, accumulated defs, cross-block defs.
    pub footnotes: crate::modules::FootnoteState,
    /// Section counters chained across Text elements.
    section_counters: std::cell::Cell<Option<[usize; 6]>>,
    /// Minimum heading level chained across Text elements.
    min_heading_level: std::cell::Cell<Option<usize>>,
    /// Cache for resolved element templates (avoids repeated filesystem lookups).
    partial_cache: std::cell::RefCell<HashMap<String, Option<String>>>,
    /// Whether any code blocks were rendered (gates syntax CSS generation).
    has_code: std::cell::Cell<bool>,
    /// The resolved target.
    pub target: Option<crate::config::Target>,
}

impl ElementRenderer {
    pub fn new(ext: &str, highlighter: Highlighter) -> Self {
        // Pre-compile all known element templates into a single minijinja
        // environment. This pays the parse cost once; each subsequent
        // render() call just executes the compiled template.
        let mut template_env = crate::render::template::TemplateEnv::new();
        let element_names: &[&'static str] = &[
            "code_source", "code_output", "code_warning", "code_message", "code_error",
            "figure", "div", "preamble",
        ];

        for name in element_names {
            if let Some(tpl) = resolve_element_partial(name, ext) {
                template_env.add(name, tpl);
            }
        }

        Self {
            ext: ext.to_string(),
            template_env,
            highlighter,
            registry: Rc::new(ModuleRegistry::empty()),
            raw_fragments: std::cell::RefCell::new(Vec::new()),
            preamble: Vec::new(),
            metadata: crate::config::Metadata::default(),
            number_sections: false,
            shift_headings: false,
            convert_math: false,
            default_fig_cap_location: None,
            chapter_number: None,
            module_ids: std::cell::RefCell::new(HashMap::new()),
            walk_metadata: std::cell::RefCell::new(crate::emit::WalkMetadata::default()),
            footnotes: crate::modules::FootnoteState::new(),
            section_counters: std::cell::Cell::new(None),
            min_heading_level: std::cell::Cell::new(None),
            partial_cache: std::cell::RefCell::new(HashMap::new()),
            has_code: std::cell::Cell::new(false),
            target: None,
        }
    }

    /// Create an ElementRenderer from document metadata and pipeline options.
    pub fn from_metadata(
        writer: &str,
        metadata: &crate::config::Metadata,
        options: &crate::render::pipeline::RenderCoreOptions,
    ) -> Self {
        let mut er = Self::new(writer, Highlighter::from_metadata(metadata));
        er.metadata = metadata.clone();
        er.number_sections = metadata.number_sections;
        er.convert_math = metadata.convert_math;
        // Only shift headings when the document has a title AND uses h1 (#) headers.
        // If the document starts at h2 (##), don't shift -- let h2 render as <h2>.
        er.shift_headings = false;
        er.chapter_number = options.chapter_number;
        if let Some(ch) = options.chapter_number {
            let mut counters = [0usize; 6];
            counters[0] = ch;
            er.set_section_counters(counters);
        }
        er.default_fig_cap_location = metadata.var.get("fig_cap_location")
            .and_then(|v| v.as_str()).map(|s| s.to_string());
        er
    }

    pub fn set_target(&mut self, target: Option<crate::config::Target>) {
        self.target = target;
    }

    pub fn set_shift_headings(&mut self, shift: bool) {
        self.shift_headings = shift;
    }

    pub fn set_registry(&mut self, registry: Rc<ModuleRegistry>) {
        self.registry = registry;
    }

    pub fn registry(&self) -> &ModuleRegistry {
        &self.registry
    }

    pub fn set_preamble(&mut self, preamble: Vec<String>) {
        self.preamble = preamble;
    }

    pub fn preamble(&self) -> &[String] {
        &self.preamble
    }

    #[inline(never)]
    pub fn render(&self, element: &Element) -> String {
        match element {
            Element::Text { content } => self.render_text(content),
            Element::CodeAsis { text } => text.clone(),
            Element::Div { classes, id, attrs, children } => self.render_div(classes, id, attrs, children),
            _ => self.render_templated(element),
        }
    }

    /// Render a text element: span dispatch, footnote injection, markdown
    /// conversion, metadata accumulation.
    fn render_text(&self, content: &str) -> String {
        let processed = self.render_bracketed_spans(content);
        // Inject cross-block footnote definitions so comrak can resolve them
        let processed = self.footnotes.inject_defs(&processed);
        let fragments = self.raw_fragments.borrow();
        let config = crate::registry::EmitterConfig {
            embed_resources: self.metadata.embed_resources.unwrap_or(true),
            number_sections: self.number_sections,
        };
        let emitter = self.registry.resolve_emitter(&self.ext, &config)
            .expect("no writer registered for format");
        let options = crate::emit::WalkOptions {
            number_sections: self.number_sections,
            shift_headings: self.shift_headings,
            footnote_counter_start: self.footnotes.counter(),
            section_counters_start: self.section_counters.get(),
            min_heading_level: self.min_heading_level.get(),
            suppress_footnote_section: true,
        };
        let result = crate::emit::walk_and_render_with_metadata(
            emitter.as_ref(), &processed, &fragments, &options,
        );
        self.footnotes.set_counter(result.metadata.footnote_counter_end);
        // Typst math conversion post-pass
        let output = if self.ext == "typst" {
            if self.convert_math {
                crate::modules::convert_math_for_typst(&result.output)
            } else {
                crate::modules::strip_math_for_typst(&result.output)
            }
        } else {
            result.output
        };
        // Accumulate walk metadata (headings, IDs, footnote defs)
        if !result.metadata.headings.is_empty() || !result.metadata.ids.is_empty() {
            let mut meta = self.walk_metadata.borrow_mut();
            meta.headings.extend(result.metadata.headings);
            meta.ids.extend(result.metadata.ids);
        }
        if self.section_counters.get().is_some() || options.number_sections {
            self.section_counters.set(Some(result.metadata.section_counters_end));
            self.min_heading_level.set(Some(result.metadata.min_heading_level));
        }
        self.footnotes.accumulate(result.metadata.footnote_defs);
        output
    }

    /// Render a fenced div: delegate to div pipeline (modules handle ID registration).
    fn render_div(
        &self,
        classes: &[String],
        id: &Option<String>,
        attrs: &HashMap<String, String>,
        children: &[Element],
    ) -> String {
        crate::render::div::render(
            classes, id, attrs, children, &self.ext,
            &self.registry,
            &|e| self.render(e),
            &|name| self.resolve_element_partial(name),
            &self.raw_fragments,
            &self.module_ids,
            &self.template_env,
            &self.metadata,
        )
    }

    /// Render a templated element (code, figure, diagnostic): build template
    /// vars, apply element template, handle code listing wrapping.
    fn render_templated(&self, element: &Element) -> String {
        if matches!(element, Element::CodeSource { .. }) {
            self.has_code.set(true);
        }
        let name = element.template_name();
        let rendered = self.build_template_output(name, element);

        // Wrap code source in a listing div when the label has a lst- prefix
        if let Element::CodeSource { label, lst_cap, .. } = element {
            if label.starts_with("lst-") {
                return crate::modules::wrap_listing(
                    label, lst_cap.as_deref(), &rendered, &self.ext,
                    &self.module_ids, &self.metadata, &self.template_env,
                    &|name| self.resolve_element_partial(name),
                );
            }
        }

        rendered
    }

    fn render_bracketed_spans(&self, text: &str) -> String {
        crate::render::span::render(
            text, &self.ext, &self.registry, &self.raw_fragments,
            &self.metadata,
            &|name| self.resolve_element_partial(name),
            &self.template_env,
        )
    }

    fn build_template_output(&self, template_name: &str, element: &Element) -> String {
        let mut vars = HashMap::new();
        vars.insert("base".to_string(), self.ext.clone());
        vars.insert("writer".to_string(), self.ext.clone());

        // Run element through pipeline filters
        let code_filter = crate::render::vars::BuildCodeVars::new(&self.highlighter);
        let figure_filter = crate::modules::BuildFigureVars::new(
            &self.ext,
            self.target.as_ref(),
            self.default_fig_cap_location.clone(),
        );

        for builder in [&code_filter as &dyn BuildElementVars, &figure_filter as &dyn BuildElementVars] {
            builder.apply(element, &self.ext, &mut vars, &self.metadata);
        }

        self.template_env.render(template_name, &vars)
    }

    fn resolve_element_partial(&self, name: &str) -> Option<String> {
        // Check cache first to avoid repeated filesystem lookups
        if let Some(cached) = self.partial_cache.borrow().get(name) {
            return cached.clone();
        }
        let result = self.registry.resolve_element_partial(name, &self.ext)
            .or_else(|| resolve_element_partial(name, &self.ext));
        self.partial_cache.borrow_mut().insert(name.to_string(), result.clone());
        result
    }

    /// Set initial section counters (e.g., for chapter-prefixed numbering).
    pub fn set_section_counters(&self, counters: [usize; 6]) {
        self.section_counters.set(Some(counters));
    }

    pub fn module_ids(&self) -> HashMap<String, String> {
        self.module_ids.borrow().clone()
    }

    pub fn syntax_css(&self) -> String {
        if self.ext != "html" || !self.has_code.get() { return String::new(); }
        self.highlighter.syntax_css()
    }

    pub fn latex_color_definitions(&self) -> String {
        if !self.has_code.get() { return String::new(); }
        self.highlighter.latex_color_definitions()
    }

    /// Return the accumulated walk metadata (headings for TOC, IDs for cross-refs).
    pub fn walk_metadata(&self) -> crate::emit::WalkMetadata {
        self.walk_metadata.borrow().clone()
    }

}

/// Resolve an element template: project → user → built-in.
/// Template names use underscores internally; hyphens are normalized.
pub fn resolve_element_partial(name: &str, ext: &str) -> Option<String> {
    let canonical = name.replace('-', "_");
    if crate::paths::has_user_partials() {
        // User partials exist: use only filesystem, no built-in fallback
        crate::paths::resolve_partial(&canonical, ext)
            .and_then(|path| std::fs::read_to_string(&path).ok())
    } else {
        // No user partials: use only built-in
        resolve_builtin_partial(&canonical, ext).map(|s| s.to_string())
    }
}
