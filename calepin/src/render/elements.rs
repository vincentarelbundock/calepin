//! Element rendering engine: thin dispatcher that delegates to filters.

use std::collections::HashMap;
use std::rc::Rc;

use include_dir::{include_dir, Dir};

use crate::types::Element;
use crate::filters::{Filter, FilterResult};
use crate::registry::PluginRegistry;
use crate::filters::highlighting::{Highlighter, HighlightConfig, ColorScope};

// ---------------------------------------------------------------------------
// Built-in project tree (embedded at compile time)
// ---------------------------------------------------------------------------

/// The entire built-in project directory, embedded in the binary.
/// Files are discovered by path at runtime -- no hardcoded file list.
pub static BUILTIN_PROJECT: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/project");

/// Component name aliases: multiple names can map to the same template file.
fn resolve_component_alias(name: &str) -> &str {
    match name {
        // Italic-body theorem environments share one template
        "theorem" | "lemma" | "corollary" | "conjecture" | "proposition" => "theorem_italic",
        // Normal-body theorem environments share one template
        "definition" | "example" | "exercise" | "solution" | "remark" | "algorithm" => "theorem_normal",
        // Callout types share one template
        "callout_note" | "callout_tip" | "callout_warning" | "callout_caution" | "callout_important" => "callout",
        // Code diagnostics share one template
        "code_error" | "code_warning" | "code_message" => "code_diagnostic",
        // Everything else: name matches file directly
        other => other,
    }
}

/// Look up a built-in component template by name.
/// Checks `components/{base}/{name}.{ext}` then `components/common/{name}.jinja`.
fn builtin_component(name: &str, base: &str) -> Option<&'static str> {
    let resolved = resolve_component_alias(name);
    let ext = crate::paths::base_to_ext(base);

    // Base-specific override
    let base_path = format!("components/{}/{}.{}", base, resolved, ext);
    if let Some(file) = BUILTIN_PROJECT.get_file(&base_path) {
        return file.contents_utf8();
    }

    // Generic .jinja
    let common_path = format!("components/common/{}.jinja", resolved);
    BUILTIN_PROJECT.get_file(&common_path).and_then(|f| f.contents_utf8())
}

/// Look up a built-in page template by name and base.
/// Checks `templates/{base}/{name}.{ext}` then `templates/common/{name}.jinja`.
pub fn builtin_template(name: &str, base: &str) -> Option<&'static str> {
    let ext = crate::paths::base_to_ext(base);

    let base_path = format!("templates/{}/{}.{}", base, name, ext);
    if let Some(file) = BUILTIN_PROJECT.get_file(&base_path) {
        return file.contents_utf8();
    }

    let common_path = format!("templates/common/{}.jinja", name);
    BUILTIN_PROJECT.get_file(&common_path).and_then(|f| f.contents_utf8())
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
    registry: Rc<PluginRegistry>,
    raw_fragments: std::cell::RefCell<Vec<String>>,
    sc_fragments: Vec<String>,
    pub number_sections: bool,
    pub shift_headings: bool,
    pub default_fig_cap_location: Option<String>,
    /// Theorem numbers populated during rendering by TheoremFilter.
    /// Keyed by full id (e.g. "thm-cauchy"), value is the number string.
    pub theorem_numbers: std::cell::RefCell<HashMap<String, String>>,
    /// Accumulated walk metadata (headings, IDs) from all Text element renders.
    pub walk_metadata: std::cell::RefCell<crate::render::ast::WalkMetadata>,
    /// Running footnote counter across Text elements.
    footnote_counter: std::cell::Cell<usize>,
    /// Footnote definitions collected from all Text elements (for cross-block resolution).
    /// Each entry is the full `[^name]: content` line(s).
    global_footnote_defs: std::cell::RefCell<String>,
    /// Cache for resolved element templates (avoids repeated filesystem lookups).
    template_cache: std::cell::RefCell<HashMap<String, Option<String>>>,
}

impl ElementRenderer {
    pub fn new(ext: &str, highlight_config: HighlightConfig) -> Self {
        // Pre-compile all known element templates into a single minijinja
        // environment. This pays the parse cost once; each subsequent
        // render() call just executes the compiled template.
        let mut template_env = crate::render::template::TemplateEnv::new();
        let element_names: &[&'static str] = &[
            "code_source", "code_output", "code_warning", "code_message", "code_error",
            "figure", "div",
            "callout_note", "callout_warning", "callout_tip", "callout_caution", "callout_important",
            "theorem", "lemma", "corollary", "proposition", "conjecture",
            "definition", "example", "exercise", "solution", "remark", "algorithm", "proof",
            "preamble",
        ];

        for name in element_names {
            if let Some(tpl) = resolve_element_template(name, ext) {
                template_env.add(name, tpl);
            }
        }

        Self {
            ext: ext.to_string(),
            template_env,
            highlighter: Highlighter::new(highlight_config),
            registry: Rc::new(PluginRegistry::empty()),
            raw_fragments: std::cell::RefCell::new(Vec::new()),
            sc_fragments: Vec::new(),
            number_sections: false,
            shift_headings: false,
            default_fig_cap_location: None,
            theorem_numbers: std::cell::RefCell::new(HashMap::new()),
            walk_metadata: std::cell::RefCell::new(crate::render::ast::WalkMetadata::default()),
            footnote_counter: std::cell::Cell::new(0),
            global_footnote_defs: std::cell::RefCell::new(String::new()),
            template_cache: std::cell::RefCell::new(HashMap::new()),
        }
    }

    pub fn set_registry(&mut self, registry: Rc<PluginRegistry>) {
        self.registry = registry;
    }

    pub fn set_sc_fragments(&mut self, sc: Vec<String>) {
        self.sc_fragments = sc;
    }

    pub fn get_template(&self, name: &str) -> String {
        let mut vars = HashMap::new();
        vars.insert("base".to_string(), self.ext.clone());
        vars.insert("base".to_string(), self.ext.clone());
        self.template_env.render(name, &vars)
    }

    #[inline(never)]
    pub fn render(&self, element: &Element) -> String {
        match element {
            Element::Text { content } => {
                let processed = self.render_bracketed_spans(content);
                // Append global footnote definitions if this text has footnote refs
                // so comrak can resolve them within a single parse.
                let processed = {
                    let defs = self.global_footnote_defs.borrow();
                    if !defs.is_empty() && processed.contains("[^") {
                        format!("{}{}", processed, defs)
                    } else {
                        processed
                    }
                };
                let fragments = self.raw_fragments.borrow();
                let rendered = match self.ext.as_str() {
                    "html" => {
                        let fn_start = self.footnote_counter.get();
                        let result = crate::render::markdown::render_html_full_with_metadata(
                            &processed, &fragments, self.number_sections, self.shift_headings, fn_start,
                        );
                        self.footnote_counter.set(result.metadata.footnote_counter_end);
                        let mut meta = self.walk_metadata.borrow_mut();
                        meta.headings.extend(result.metadata.headings);
                        meta.ids.extend(result.metadata.ids);
                        result.output
                    }
                    "typst" => {
                        let fn_start = self.footnote_counter.get();
                        let (output, fn_end) = crate::render::typst_ast::markdown_to_typst_with_counter(
                            &processed, &fragments, fn_start,
                        );
                        self.footnote_counter.set(fn_end);
                        output
                    }
                    "latex" => {
                        let fn_start = self.footnote_counter.get();
                        let (output, fn_end) = crate::render::latex_emit::markdown_to_latex_with_counter(
                            &processed, &fragments, self.number_sections, fn_start,
                        );
                        self.footnote_counter.set(fn_end);
                        output
                    }
                    _ => crate::render::markdown::resolve_raw(&processed, &fragments),
                };
                crate::render::markers::resolve_shortcode_raw(&rendered, &self.sc_fragments)
            }
            Element::CodeAsis { text } => text.clone(),
            Element::Div { classes, id, attrs, children } => {
                // Track fig-/tbl- IDs for cross-reference resolution
                if let Some(ref div_id) = id {
                    if div_id.starts_with("fig-") || div_id.starts_with("tbl-") {
                        let prefix = &div_id[..4]; // "fig-" or "tbl-"
                        let mut meta = self.walk_metadata.borrow_mut();
                        let count = meta.ids.keys().filter(|k| k.starts_with(prefix)).count();
                        meta.ids.insert(div_id.clone(), (count + 1).to_string());
                    }
                }
                crate::render::div::render(
                    classes, id, attrs, children, &self.ext,
                    &self.registry,
                    &|e| self.render(e),
                    &|name| self.resolve_element_template(name),
                    &self.raw_fragments,
                    &self.theorem_numbers,
                )
            }
            _ => {
                let name = element.template_name();
                self.build_template_output(name, element)
            }
        }
    }

    fn render_bracketed_spans(&self, text: &str) -> String {
        crate::render::span::render(
            text, &self.ext, &self.registry, &self.raw_fragments,
            &|name| self.resolve_element_template(name),
        )
    }

    fn build_template_output(&self, template_name: &str, element: &Element) -> String {
        let mut vars = HashMap::new();
        vars.insert("base".to_string(), self.ext.clone());
        vars.insert("base".to_string(), self.ext.clone());

        // Run element through pipeline filters
        let code_filter = crate::filters::code::CodeFilter::new(&self.highlighter);
        let figure_filter = crate::filters::figure::FigureFilter::new(
            self.default_fig_cap_location.clone(),
        );

        for filter in [&code_filter as &dyn Filter, &figure_filter as &dyn Filter] {
            match filter.apply(element, &self.ext, &mut vars) {
                FilterResult::Rendered(output) => return output,
                FilterResult::Continue | FilterResult::Pass => {}
            }
        }

        self.template_env.render(template_name, &vars)
    }

    fn resolve_element_template(&self, name: &str) -> Option<String> {
        // Check cache first to avoid repeated filesystem lookups
        if let Some(cached) = self.template_cache.borrow().get(name) {
            return cached.clone();
        }
        let result = self.registry.resolve_element_template(name, &self.ext)
            .or_else(|| resolve_element_template(name, &self.ext));
        self.template_cache.borrow_mut().insert(name.to_string(), result.clone());
        result
    }

    pub fn theorem_numbers(&self) -> HashMap<String, String> {
        self.theorem_numbers.borrow().clone()
    }

    pub fn syntax_css(&self) -> String {
        if self.ext != "html" { return String::new(); }
        self.highlighter.syntax_css()
    }

    pub fn syntax_css_with_scope(&self, scope: ColorScope) -> String {
        if self.ext != "html" { return String::new(); }
        self.highlighter.syntax_css_with_scope(scope)
    }

    pub fn latex_color_definitions(&self) -> String {
        self.highlighter.latex_color_definitions()
    }

    /// Return the accumulated walk metadata (headings for TOC, IDs for cross-refs).
    pub fn walk_metadata(&self) -> crate::render::ast::WalkMetadata {
        self.walk_metadata.borrow().clone()
    }

    /// Pre-scan all Text elements for footnote definitions (`[^name]: ...`).
    /// These are collected so they can be appended to Text elements that contain
    /// footnote references (`[^name]`), enabling cross-block footnote resolution.
    pub fn collect_footnote_defs(&self, elements: &[Element]) {
        use regex::Regex;
        use std::sync::LazyLock;
        static RE_FN_DEF: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"(?m)^\[\^[^\]]+\]:.*(?:\n(?:    |\t).*)*").unwrap()
        });

        let mut defs = String::new();
        for el in elements {
            if let Element::Text { content } = el {
                for m in RE_FN_DEF.find_iter(content) {
                    defs.push_str("\n\n");
                    defs.push_str(m.as_str());
                }
            }
            // Also recurse into divs
            if let Element::Div { children, .. } = el {
                self.collect_footnote_defs_recursive(children, &mut defs);
            }
        }
        *self.global_footnote_defs.borrow_mut() = defs;
    }

    fn collect_footnote_defs_recursive(&self, elements: &[Element], defs: &mut String) {
        use regex::Regex;
        use std::sync::LazyLock;
        static RE_FN_DEF: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"(?m)^\[\^[^\]]+\]:.*(?:\n(?:    |\t).*)*").unwrap()
        });

        for el in elements {
            if let Element::Text { content } = el {
                for m in RE_FN_DEF.find_iter(content) {
                    defs.push_str("\n\n");
                    defs.push_str(m.as_str());
                }
            }
            if let Element::Div { children, .. } = el {
                self.collect_footnote_defs_recursive(children, defs);
            }
        }
    }
}

/// Resolve an element template (component): project → user → built-in.
/// Template names use underscores internally; hyphens are normalized.
pub fn resolve_element_template(name: &str, ext: &str) -> Option<String> {
    let canonical = name.replace('-', "_");
    // Project/user filesystem resolution
    if let Some(path) = crate::paths::resolve_component(&canonical, ext) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            return Some(content);
        }
    }
    // Built-in: discovered from embedded project tree
    builtin_component(&canonical, ext).map(|s| s.to_string())
}
