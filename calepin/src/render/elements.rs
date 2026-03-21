//! Element rendering engine: thin dispatcher that delegates to filters.

use std::collections::HashMap;
use std::rc::Rc;

use crate::types::Element;
use crate::filters::{Filter, FilterResult};
use crate::registry::PluginRegistry;
use crate::filters::highlighting::{Highlighter, HighlightConfig, ColorScope};

// ---------------------------------------------------------------------------
// Built-in templates (embedded at compile time)
// ---------------------------------------------------------------------------
//
// Each template is a single .tera file with format conditionals
// ({% if format == "html" %} ... {% elif format == "latex" %} ... {% endif %}).

const THEOREM_ITALIC: &str = include_str!("../templates/elements/theorem_italic.tera");
const THEOREM_NORMAL: &str = include_str!("../templates/elements/theorem_normal.tera");
const CALLOUT: &str = include_str!("../templates/elements/callout.tera");
const CODE_DIAGNOSTIC: &str = include_str!("../templates/elements/code_diagnostic.tera");

fn builtin_template(name: &str) -> Option<&'static str> {
    match name {
        // Italic-body theorem environments
        "theorem" | "lemma" | "corollary" | "conjecture" | "proposition" => Some(THEOREM_ITALIC),
        // Normal-body theorem environments
        "definition" | "example" | "exercise" | "solution" | "remark" | "algorithm" => Some(THEOREM_NORMAL),
        // Callout environments (all types share one template)
        "callout_note" | "callout_tip" | "callout_warning" | "callout_caution" | "callout_important" => Some(CALLOUT),
        // Code diagnostics
        "code_error" | "code_warning" | "code_message" => Some(CODE_DIAGNOSTIC),
        // Single-file templates
        "code_source" => Some(include_str!("../templates/elements/code_source.tera")),
        "code_output" => Some(include_str!("../templates/elements/code_output.tera")),
        "figure" => Some(include_str!("../templates/elements/figure.tera")),
        "div" => Some(include_str!("../templates/elements/div.tera")),
        "proof" => Some(include_str!("../templates/elements/proof.tera")),
        "landscape" => Some(include_str!("../templates/elements/landscape.tera")),
        "preamble" => Some(include_str!("../templates/elements/preamble.tera")),
        "appendix" => Some(include_str!("../templates/elements/appendix.tera")),
        "appendix_license" => Some(include_str!("../templates/elements/appendix_license.tera")),
        "appendix_copyright" => Some(include_str!("../templates/elements/appendix_copyright.tera")),
        "appendix_funding" => Some(include_str!("../templates/elements/appendix_funding.tera")),
        "appendix_citation" => Some(include_str!("../templates/elements/appendix_citation.tera")),
        "author_block" => Some(include_str!("../templates/elements/author_block.tera")),
        "author_item" => Some(include_str!("../templates/elements/author_item.tera")),
        "affiliation_item" => Some(include_str!("../templates/elements/affiliation_item.tera")),
        "title_block" => Some(include_str!("../templates/elements/title_block.tera")),
        "subtitle_block" => Some(include_str!("../templates/elements/subtitle_block.tera")),
        "date_block" => Some(include_str!("../templates/elements/date_block.tera")),
        "abstract_block" => Some(include_str!("../templates/elements/abstract_block.tera")),
        "keywords_block" => Some(include_str!("../templates/elements/keywords_block.tera")),
        "bibliography_block" => Some(include_str!("../templates/elements/bibliography_block.tera")),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// ElementRenderer
// ---------------------------------------------------------------------------

pub struct ElementRenderer {
    ext: String,
    templates: HashMap<String, String>,
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
}

impl ElementRenderer {
    pub fn new(ext: &str, highlight_config: HighlightConfig) -> Self {
        let mut templates = HashMap::with_capacity(40);
        let element_names = [
            "code_source", "code_output", "code_warning", "code_message", "code_error",
            "figure", "div",
            "callout_note", "callout_warning", "callout_tip", "callout_caution", "callout_important",
            "theorem", "lemma", "corollary", "proposition", "conjecture",
            "definition", "example", "exercise", "solution", "remark", "algorithm", "proof",
            "preamble",
        ];

        for name in &element_names {
            if let Some(tpl) = resolve_element_template(name, ext) {
                templates.insert(name.to_string(), tpl);
            }
        }

        Self {
            ext: ext.to_string(),
            templates,
            highlighter: Highlighter::new(highlight_config),
            registry: Rc::new(PluginRegistry::empty()),
            raw_fragments: std::cell::RefCell::new(Vec::new()),
            sc_fragments: Vec::new(),
            number_sections: false,
            shift_headings: false,
            default_fig_cap_location: None,
            theorem_numbers: std::cell::RefCell::new(HashMap::new()),
        }
    }

    pub fn set_registry(&mut self, registry: Rc<PluginRegistry>) {
        self.registry = registry;
    }

    pub fn set_sc_fragments(&mut self, sc: Vec<String>) {
        self.sc_fragments = sc;
    }

    pub fn get_template(&self, name: &str) -> String {
        match self.templates.get(name) {
            Some(tpl) => {
                let mut vars = HashMap::new();
                vars.insert("format".to_string(), self.ext.clone());
                crate::render::template::apply_template(tpl, &vars)
            }
            None => String::new(),
        }
    }

    #[inline(never)]
    pub fn render(&self, element: &Element) -> String {
        match element {
            Element::Text { content } => {
                let processed = self.render_bracketed_spans(content);
                // Shift headings down one level (# → ##) only for HTML, where
                // <h1> is reserved for the document title. LaTeX and Typst have
                // no such constraint: \section and = are valid top-level headings.
                let processed = if self.shift_headings && self.ext == "html" {
                    crate::render::markdown::shift_headings(&processed)
                } else {
                    processed
                };
                let fragments = self.raw_fragments.borrow();
                let rendered = match self.ext.as_str() {
                    "html" => crate::render::markdown::render_html(&processed, &fragments),
                    "typst" => crate::render::markdown::render_typst(&processed, &fragments),
                    "latex" => crate::render::latex::markdown_to_latex(&processed, &fragments, self.number_sections),
                    _ => crate::render::markdown::resolve_raw(&processed, &fragments),
                };
                crate::render::markers::resolve_shortcode_raw(&rendered, &self.sc_fragments)
            }
            Element::CodeAsis { text } => text.clone(),
            Element::Div { classes, id, attrs, children } => {
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
                let tpl = match self.templates.get(name) {
                    Some(t) => t.clone(),
                    None => return String::new(),
                };
                self.build_template_output(&tpl, element)
            }
        }
    }

    fn render_bracketed_spans(&self, text: &str) -> String {
        crate::render::span::render(
            text, &self.ext, &self.registry, &self.raw_fragments,
            &|name| self.resolve_element_template(name),
        )
    }

    fn build_template_output(&self, template: &str, element: &Element) -> String {
        let mut vars = HashMap::new();
        vars.insert("format".to_string(), self.ext.clone());

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

        crate::render::template::apply_template(template, &vars)
    }

    fn resolve_element_template(&self, name: &str) -> Option<String> {
        // Check plugin-provided element templates first
        if let Some(tpl) = self.registry.resolve_element_template(name, &self.ext) {
            return Some(tpl);
        }
        resolve_element_template(name, &self.ext)
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
}

/// Resolve an element template: project → user → built-in.
/// Template names use underscores internally; hyphens are normalized.
/// User overrides are per-format ({name}.{ext}); built-in templates are
/// format-conditional single files.
pub fn resolve_element_template(name: &str, ext: &str) -> Option<String> {
    let canonical = name.replace('-', "_");
    // User override: per-format file
    let filename = format!("{}.{}", canonical, ext);
    if let Some(path) = crate::util::resolve_path("elements", &filename) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            return Some(content);
        }
    }
    // Built-in: format-conditional single template
    builtin_template(&canonical).map(|s| s.to_string())
}
