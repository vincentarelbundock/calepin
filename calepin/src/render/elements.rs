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

macro_rules! element_templates {
    ( $name:expr, $ext:expr; $( $tpl:literal => [ $( $fmt:literal ),+ ] ),+ $(,)? ) => {
        match ($name, $ext) {
            $($(
                ($tpl, $fmt) => Some(include_str!(concat!("../templates/elements/", $tpl, ".", $fmt))),
            )+)+
            _ => None,
        }
    };
}

fn builtin_template(name: &str, ext: &str) -> Option<&'static str> {
    element_templates!(name, ext;
        "code_source"  => ["html", "latex", "typst", "markdown"],
        "code_output"  => ["html", "latex", "typst", "markdown"],
        "code_warning" => ["html", "latex", "typst", "markdown"],
        "code_message" => ["html", "latex", "typst", "markdown"],
        "code_error"   => ["html", "latex", "typst", "markdown"],
        "figure" => ["html", "latex", "typst", "markdown"],
        "div"    => ["html", "latex", "typst", "markdown"],
        "callout_note"      => ["html", "latex", "typst", "markdown"],
        "callout_warning"   => ["html", "latex", "typst", "markdown"],
        "callout_tip"       => ["html", "latex", "typst", "markdown"],
        "callout_caution"   => ["html", "latex", "typst", "markdown"],
        "callout_important" => ["html", "latex", "typst", "markdown"],
        "theorem"     => ["html", "latex", "typst", "markdown"],
        "lemma"       => ["html", "latex", "typst", "markdown"],
        "corollary"   => ["html", "latex", "typst", "markdown"],
        "proposition" => ["html", "latex", "typst", "markdown"],
        "conjecture"  => ["html", "latex", "typst", "markdown"],
        "definition"  => ["html", "latex", "typst", "markdown"],
        "example"     => ["html", "latex", "typst", "markdown"],
        "exercise"    => ["html", "latex", "typst", "markdown"],
        "solution"    => ["html", "latex", "typst", "markdown"],
        "remark"      => ["html", "latex", "typst", "markdown"],
        "algorithm"   => ["html", "latex", "typst", "markdown"],
        "proof"       => ["html", "latex", "typst", "markdown"],
        "landscape"   => ["html", "latex", "typst", "markdown"],
        "preamble" => ["html", "latex", "typst"],
        "appendix"           => ["html", "latex", "typst", "markdown"],
        "appendix_license"   => ["html", "latex", "typst", "markdown"],
        "appendix_copyright" => ["html", "latex", "typst", "markdown"],
        "appendix_funding"   => ["html", "latex", "typst", "markdown"],
        "appendix_citation"  => ["html", "latex", "typst", "markdown"],
        "author_block"     => ["html", "latex", "typst", "markdown"],
        "author_item"      => ["html", "latex", "typst", "markdown"],
        "affiliation_item" => ["html", "latex", "typst", "markdown"],
        "title_block"    => ["html", "latex"],
        "subtitle_block" => ["html", "latex", "typst"],
        "date_block"     => ["html", "latex"],
        "abstract_block" => ["html", "latex", "typst"],
        "keywords_block" => ["html", "latex", "typst"]
    )
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
        self.templates.get(name).cloned().unwrap_or_default()
    }

    pub fn render(&self, element: &Element) -> String {
        match element {
            Element::Text { content } => {
                let processed = self.render_bracketed_spans(content);
                let processed = if self.shift_headings {
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
                crate::filters::shortcodes::resolve_shortcode_raw(&rendered, &self.sc_fragments)
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
pub fn resolve_element_template(name: &str, ext: &str) -> Option<String> {
    let canonical = name.replace('-', "_");
    let filename = format!("{}.{}", canonical, ext);
    if let Some(path) = crate::util::resolve_path("elements", &filename) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            return Some(content);
        }
    }
    builtin_template(&canonical, ext).map(|s| s.to_string())
}
