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

/// Built-in syntax highlighting themes (`.tmTheme` files), embedded at compile time.
pub static BUILTIN_HIGHLIGHTING: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/assets/highlighting");

/// Template name aliases: multiple names can map to the same template file.
fn resolve_template_alias(name: &str) -> &str {
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

/// Look up a built-in template by name and base.
/// Checks `templates/{base}/{name}.{ext}` then `templates/common/{name}.jinja`.
pub fn resolve_builtin_template(name: &str, base: &str) -> Option<&'static str> {
    let resolved = resolve_template_alias(name);
    let ext = crate::paths::engine_to_ext(base);

    // Base-specific
    let base_path = format!("templates/{}/{}.{}", base, resolved, ext);
    if let Some(file) = BUILTIN_PROJECT.get_file(&base_path) {
        return file.contents_utf8();
    }

    // Generic .jinja
    let common_path = format!("templates/common/{}.jinja", resolved);
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
    preamble: Vec<String>,
    pub number_sections: bool,
    pub shift_headings: bool,
    pub default_fig_cap_location: Option<String>,
    /// Chapter number for collection pages. When set, section counters start
    /// at [chapter, 0, 0, 0, 0, 0] so sections get chapter-prefixed numbers.
    pub chapter_number: Option<usize>,
    /// Theorem numbers populated during rendering by TheoremFilter.
    /// Keyed by full id (e.g. "thm-cauchy"), value is the number string.
    pub theorem_numbers: std::cell::RefCell<HashMap<String, String>>,
    /// Accumulated walk metadata (headings, IDs) from all Text element renders.
    pub walk_metadata: std::cell::RefCell<crate::render::ast::WalkMetadata>,
    /// Running footnote counter across Text elements.
    footnote_counter: std::cell::Cell<usize>,
    /// Section counters chained across Text elements.
    section_counters: std::cell::Cell<Option<[usize; 6]>>,
    /// Minimum heading level chained across Text elements.
    min_heading_level: std::cell::Cell<Option<usize>>,
    /// Accumulated footnote defs from all Text elements (for combined section at end).
    accumulated_footnote_defs: std::cell::RefCell<Vec<(usize, String)>>,
    /// Footnote definitions collected from all Text elements (for cross-block resolution).
    /// Each entry is the full `[^name]: content` line(s).
    global_footnote_defs: std::cell::RefCell<String>,
    /// Cache for resolved element templates (avoids repeated filesystem lookups).
    template_cache: std::cell::RefCell<HashMap<String, Option<String>>>,
    /// Whether any code blocks were rendered (gates syntax CSS generation).
    has_code: std::cell::Cell<bool>,
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
            preamble: Vec::new(),
            number_sections: false,
            shift_headings: false,
            default_fig_cap_location: None,
            chapter_number: None,
            theorem_numbers: std::cell::RefCell::new(HashMap::new()),
            walk_metadata: std::cell::RefCell::new(crate::render::ast::WalkMetadata::default()),
            footnote_counter: std::cell::Cell::new(0),
            section_counters: std::cell::Cell::new(None),
            min_heading_level: std::cell::Cell::new(None),
            accumulated_footnote_defs: std::cell::RefCell::new(Vec::new()),
            global_footnote_defs: std::cell::RefCell::new(String::new()),
            template_cache: std::cell::RefCell::new(HashMap::new()),
            has_code: std::cell::Cell::new(false),
        }
    }

    pub fn set_registry(&mut self, registry: Rc<PluginRegistry>) {
        self.registry = registry;
    }

    pub fn set_sc_fragments(&mut self, sc: Vec<String>) {
        self.sc_fragments = sc;
    }

    pub fn set_preamble(&mut self, preamble: Vec<String>) {
        self.preamble = preamble;
    }

    pub fn preamble(&self) -> &[String] {
        &self.preamble
    }

    /// Render the combined footnote section from all accumulated Text elements.
    /// Returns empty string if no footnotes or if format is not HTML.
    pub fn render_footnote_section(&self) -> String {
        let defs = self.accumulated_footnote_defs.borrow();
        if defs.is_empty() || self.ext != "html" {
            return String::new();
        }
        crate::render::html_ast::render_footnote_section(&defs)
    }

    pub fn render_template(&self, name: &str) -> String {
        let mut vars = HashMap::new();
        vars.insert("base".to_string(), self.ext.clone());
        vars.insert("engine".to_string(), self.ext.clone());
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
                        let options = crate::render::ast::WalkOptions {
                            number_sections: self.number_sections,
                            shift_headings: self.shift_headings,
                            footnote_counter_start: self.footnote_counter.get(),
                            section_counters_start: self.section_counters.get(),
                            min_heading_level: self.min_heading_level.get(),
                            suppress_footnote_section: true,
                        };
                        let result = crate::render::markdown::render_html_with_metadata(
                            &processed, &fragments, &options,
                        );
                        self.footnote_counter.set(result.metadata.footnote_counter_end);
                        self.section_counters.set(Some(result.metadata.section_counters_end));
                        self.min_heading_level.set(Some(result.metadata.min_heading_level));
                        if !result.metadata.footnote_defs.is_empty() {
                            let mut acc = self.accumulated_footnote_defs.borrow_mut();
                            for def in result.metadata.footnote_defs {
                                if !acc.iter().any(|(id, _)| *id == def.0) {
                                    acc.push(def);
                                }
                            }
                        }
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
                        let (output, fn_end) = crate::render::latex::markdown_to_latex_with_counter(
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
                // Track cross-referenceable div IDs
                if let Some(ref div_id) = id {
                    let trackable_prefix = if div_id.starts_with("fig-") || div_id.starts_with("tbl-") {
                        Some(&div_id[..4])
                    } else if div_id.starts_with("tip-") || div_id.starts_with("nte-")
                        || div_id.starts_with("wrn-") || div_id.starts_with("imp-")
                        || div_id.starts_with("cau-") || div_id.starts_with("lst-")
                    {
                        Some(&div_id[..4])
                    } else {
                        None
                    };
                    if let Some(prefix) = trackable_prefix {
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
                if matches!(element, Element::CodeSource { .. }) {
                    self.has_code.set(true);
                }
                let name = element.template_name();
                let rendered = self.build_template_output(name, element);

                // Wrap code source in a listing div when the label has a lst- prefix
                if let Element::CodeSource { label, lst_cap, .. } = element {
                    if label.starts_with("lst-") {
                        // Track listing ID for cross-references
                        let mut meta = self.walk_metadata.borrow_mut();
                        let count = meta.ids.keys().filter(|k| k.starts_with("lst-")).count();
                        meta.ids.insert(label.clone(), (count + 1).to_string());
                        let num = count + 1;

                        let mut listing_vars = HashMap::new();
                        listing_vars.insert("base".to_string(), self.ext.clone());
                        listing_vars.insert("engine".to_string(), self.ext.clone());
                        listing_vars.insert("label".to_string(), label.clone());
                        listing_vars.insert("number".to_string(), num.to_string());
                        listing_vars.insert("content".to_string(), rendered);
                        if let Some(cap) = lst_cap {
                            listing_vars.insert("lst_cap".to_string(), cap.clone());
                        }
                        let tpl = self.resolve_element_template("code_listing")
                            .unwrap_or_else(|| include_str!("../project/templates/common/code_listing.jinja").to_string());
                        return crate::render::template::apply_template(&tpl, &listing_vars);
                    }
                }

                rendered
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
        vars.insert("engine".to_string(), self.ext.clone());

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

    /// Set initial section counters (e.g., for chapter-prefixed numbering).
    pub fn set_section_counters(&self, counters: [usize; 6]) {
        self.section_counters.set(Some(counters));
    }

    pub fn theorem_numbers(&self) -> HashMap<String, String> {
        self.theorem_numbers.borrow().clone()
    }

    pub fn syntax_css(&self) -> String {
        if self.ext != "html" || !self.has_code.get() { return String::new(); }
        self.highlighter.syntax_css()
    }

    pub fn syntax_css_with_scope(&self, scope: ColorScope) -> String {
        if self.ext != "html" || !self.has_code.get() { return String::new(); }
        self.highlighter.syntax_css_with_scope(scope)
    }

    pub fn latex_color_definitions(&self) -> String {
        if !self.has_code.get() { return String::new(); }
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
        let mut defs = String::new();
        Self::collect_footnote_defs_recursive(elements, &mut defs);
        *self.global_footnote_defs.borrow_mut() = defs;
    }

    fn collect_footnote_defs_recursive(elements: &[Element], defs: &mut String) {
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
                Self::collect_footnote_defs_recursive(children, defs);
            }
        }
    }
}

/// Resolve an element template: project → user → built-in.
/// Template names use underscores internally; hyphens are normalized.
pub fn resolve_element_template(name: &str, ext: &str) -> Option<String> {
    let canonical = name.replace('-', "_");
    // Project/user filesystem resolution
    if let Some(path) = crate::paths::resolve_template(&canonical, ext) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            return Some(content);
        }
    }
    // Built-in: discovered from embedded project tree
    resolve_builtin_template(&canonical, ext).map(|s| s.to_string())
}
