//! Element rendering engine: thin dispatcher that delegates to filters.

use std::collections::HashMap;

use crate::types::Element;
use crate::filters::{Filter, FilterResult};
use crate::plugins::PluginHandle;
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
        "code-source"  => ["html", "latex", "typst", "markdown"],
        "code-output"  => ["html", "latex", "typst", "markdown"],
        "code-warning" => ["html", "latex", "typst", "markdown"],
        "code-message" => ["html", "latex", "typst", "markdown"],
        "code-error"   => ["html", "latex", "typst", "markdown"],
        "figure" => ["html", "latex", "typst", "markdown"],
        "div"    => ["html", "latex", "typst", "markdown"],
        "callout-note"      => ["html", "latex", "typst", "markdown"],
        "callout-warning"   => ["html", "latex", "typst", "markdown"],
        "callout-tip"       => ["html", "latex", "typst", "markdown"],
        "callout-caution"   => ["html", "latex", "typst", "markdown"],
        "callout-important" => ["html", "latex", "typst", "markdown"],
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
        "preamble" => ["html", "latex", "typst"],
        "appendix"           => ["html", "latex", "typst", "markdown"],
        "appendix-license"   => ["html", "latex", "typst", "markdown"],
        "appendix-copyright" => ["html", "latex", "typst", "markdown"],
        "appendix-funding"   => ["html", "latex", "typst", "markdown"],
        "appendix-citation"  => ["html", "latex", "typst", "markdown"],
        "author-block"     => ["html", "latex", "typst", "markdown"],
        "author-item"      => ["html", "latex", "typst", "markdown"],
        "affiliation-item" => ["html", "latex", "typst", "markdown"],
        "title-block"    => ["html", "latex"],
        "subtitle-block" => ["html", "latex", "typst"],
        "date-block"     => ["html", "latex"],
        "abstract-block" => ["html", "latex", "typst"],
        "keywords-block" => ["html", "latex", "typst"]
    )
}

// ---------------------------------------------------------------------------
// ElementRenderer
// ---------------------------------------------------------------------------

pub struct ElementRenderer {
    ext: String,
    templates: HashMap<String, String>,
    highlighter: Highlighter,
    builtin_filters: Vec<Box<dyn Filter>>,
    plugins: Vec<PluginHandle>,
    raw_fragments: std::cell::RefCell<Vec<String>>,
    sc_fragments: Vec<String>,
    escaped_sc_fragments: Vec<String>,
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
            "code-source", "code-output", "code-warning", "code-message", "code-error",
            "figure", "div",
            "callout-note", "callout-warning", "callout-tip", "callout-caution", "callout-important",
            "theorem", "lemma", "corollary", "proposition", "conjecture",
            "definition", "example", "exercise", "solution", "remark", "algorithm", "proof",
            "preamble",
        ];

        for name in &element_names {
            if let Some(tpl) = resolve_element_template(name, ext) {
                templates.insert(name.to_string(), tpl);
            }
        }

        let builtin_filters: Vec<Box<dyn Filter>> = vec![
            Box::new(crate::filters::TheoremFilter::new()),
            Box::new(crate::filters::CalloutFilter::new()),
        ];

        Self {
            ext: ext.to_string(),
            templates,
            highlighter: Highlighter::new(highlight_config),
            builtin_filters,
            plugins: Vec::new(),
            raw_fragments: std::cell::RefCell::new(Vec::new()),
            sc_fragments: Vec::new(),
            escaped_sc_fragments: Vec::new(),
            number_sections: false,
            shift_headings: false,
            default_fig_cap_location: None,
            theorem_numbers: std::cell::RefCell::new(HashMap::new()),
        }
    }

    pub fn set_plugins(&mut self, plugins: Vec<PluginHandle>) {
        self.plugins = plugins;
    }

    pub fn set_sc_fragments(&mut self, sc: Vec<String>, escaped: Vec<String>) {
        self.sc_fragments = sc;
        self.escaped_sc_fragments = escaped;
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
                let rendered = crate::filters::shortcodes::resolve_shortcode_raw(&rendered, &self.sc_fragments);
                crate::filters::shortcodes::resolve_escaped_shortcodes(&rendered, &self.escaped_sc_fragments)
            }
            Element::CodeAsis { text } => text.clone(),
            Element::Div { classes, id, attrs, children } => {
                crate::render::div::render(
                    classes, id, attrs, children, &self.ext,
                    &self.plugins, &self.builtin_filters,
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
            text, &self.ext, &self.plugins, &self.raw_fragments,
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
pub fn resolve_element_template(name: &str, ext: &str) -> Option<String> {
    let filename = format!("{}.{}", name, ext);
    if let Some(path) = crate::util::resolve_path("elements", &filename) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            return Some(content);
        }
    }
    builtin_template(name, ext).map(|s| s.to_string())
}
