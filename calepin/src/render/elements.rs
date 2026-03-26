//! Element rendering engine: thin dispatcher that delegates to filters.

use std::collections::HashMap;
use std::rc::Rc;

use include_dir::{include_dir, Dir};

use crate::types::Element;
use crate::render::filter::{Filter, FilterResult};
use crate::registry::ModuleRegistry;
use crate::modules::highlight::{Highlighter, HighlightConfig, ColorScope};

// ---------------------------------------------------------------------------
// Built-in project tree (embedded at compile time)
// ---------------------------------------------------------------------------

/// Built-in partials (element/page templates), embedded at compile time.
pub static BUILTIN_PARTIALS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/partials");

/// Built-in assets (CSS, JS, scaffold files), embedded at compile time.
pub static BUILTIN_ASSETS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/assets");

/// Template name aliases: multiple names can map to the same template file.
fn resolve_partial_alias(name: &str) -> &str {
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
/// Checks `partials/{target}/{name}.{ext}` (active target, if different from base),
/// then `partials/{base}/{name}.{ext}`, then `templates/common/{name}.jinja`.
pub fn resolve_builtin_partial(name: &str, base: &str) -> Option<&'static str> {
    let resolved = resolve_partial_alias(name);
    let ext = crate::paths::engine_to_ext(base);

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
    sc_fragments: Vec<String>,
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
    /// Theorem numbers populated during rendering by TheoremFilter.
    /// Keyed by full id (e.g. "thm-cauchy"), value is the number string.
    pub theorem_numbers: std::cell::RefCell<HashMap<String, String>>,
    /// Accumulated walk metadata (headings, IDs) from all Text element renders.
    pub walk_metadata: std::cell::RefCell<crate::emit::WalkMetadata>,
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
    partial_cache: std::cell::RefCell<HashMap<String, Option<String>>>,
    /// Whether any code blocks were rendered (gates syntax CSS generation).
    has_code: std::cell::Cell<bool>,
    /// The resolved target.
    pub target: Option<crate::project::Target>,
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
            if let Some(tpl) = resolve_element_partial(name, ext) {
                template_env.add(name, tpl);
            }
        }

        Self {
            ext: ext.to_string(),
            template_env,
            highlighter: Highlighter::new(highlight_config),
            registry: Rc::new(ModuleRegistry::empty()),
            raw_fragments: std::cell::RefCell::new(Vec::new()),
            sc_fragments: Vec::new(),
            preamble: Vec::new(),
            metadata: crate::config::Metadata::default(),
            number_sections: false,
            shift_headings: false,
            convert_math: false,
            default_fig_cap_location: None,
            chapter_number: None,
            theorem_numbers: std::cell::RefCell::new(HashMap::new()),
            walk_metadata: std::cell::RefCell::new(crate::emit::WalkMetadata::default()),
            footnote_counter: std::cell::Cell::new(0),
            section_counters: std::cell::Cell::new(None),
            min_heading_level: std::cell::Cell::new(None),
            accumulated_footnote_defs: std::cell::RefCell::new(Vec::new()),
            global_footnote_defs: std::cell::RefCell::new(String::new()),
            partial_cache: std::cell::RefCell::new(HashMap::new()),
            has_code: std::cell::Cell::new(false),
            target: None,
        }
    }

    /// Create an ElementRenderer from document metadata and pipeline options.
    pub fn from_metadata(
        engine: &str,
        metadata: &crate::config::Metadata,
        options: &crate::pipeline::RenderCoreOptions,
    ) -> Self {
        let hl = metadata.highlight.as_ref();
        let builtin_hl = crate::project::builtin_metadata().highlight.as_ref();
        let highlight_config = metadata.var.get("highlight-style")
            .map(|v| crate::modules::highlight::parse_highlight_config(v))
            .unwrap_or_else(|| {
                crate::modules::highlight::HighlightConfig::LightDark {
                    light: hl.and_then(|h| h.light.clone())
                        .or_else(|| builtin_hl.and_then(|h| h.light.clone()))
                        .unwrap_or_else(|| "github".to_string()),
                    dark: hl.and_then(|h| h.dark.clone())
                        .or_else(|| builtin_hl.and_then(|h| h.dark.clone()))
                        .unwrap_or_else(|| "nord".to_string()),
                }
            });
        let mut er = Self::new(engine, highlight_config);
        er.metadata = metadata.clone();
        er.number_sections = metadata.number_sections;
        er.convert_math = metadata.convert_math;
        er.shift_headings = metadata.title.is_some();
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

    pub fn set_target(&mut self, target: Option<crate::project::Target>) {
        self.target = target;
    }

    pub fn set_registry(&mut self, registry: Rc<ModuleRegistry>) {
        self.registry = registry;
    }

    pub fn registry(&self) -> &ModuleRegistry {
        &self.registry
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
        crate::emit::html::render_footnote_section(&defs)
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
    /// conversion, metadata accumulation, shortcode marker resolution.
    fn render_text(&self, content: &str) -> String {
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
        let rendered = if self.ext == "html" {
            let options = crate::emit::WalkOptions {
                number_sections: self.number_sections,
                shift_headings: self.shift_headings,
                footnote_counter_start: self.footnote_counter.get(),
                section_counters_start: self.section_counters.get(),
                min_heading_level: self.min_heading_level.get(),
                suppress_footnote_section: true,
            };
            let embed = self.metadata.embed_resources.unwrap_or(true);
            let result = crate::render::convert::render_html_with_metadata(
                &processed, &fragments, &options, embed,
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
        } else {
            let fn_start = self.footnote_counter.get();
            let (output, fn_end) = match self.ext.as_str() {
                "typst" => crate::emit::typst::markdown_to_typst_with_counter(
                    &processed, &fragments, fn_start, self.convert_math,
                ),
                "latex" => crate::emit::latex::markdown_to_latex_with_counter(
                    &processed, &fragments, self.number_sections, fn_start,
                ),
                _ => crate::emit::markdown::markdown_to_markdown_with_counter(
                    &processed, &fragments, fn_start,
                ),
            };
            self.footnote_counter.set(fn_end);
            output
        };
        crate::render::markers::resolve_shortcode_raw(&rendered, &self.sc_fragments)
    }

    /// Render a fenced div: track cross-referenceable IDs, delegate to div pipeline.
    fn render_div(
        &self,
        classes: &[String],
        id: &Option<String>,
        attrs: &HashMap<String, String>,
        children: &[Element],
    ) -> String {
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
            &|name| self.resolve_element_partial(name),
            &self.raw_fragments,
            &self.theorem_numbers,
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
                // Track listing ID for cross-references
                let mut meta = self.walk_metadata.borrow_mut();
                let count = meta.ids.keys().filter(|k| k.starts_with("lst-")).count();
                meta.ids.insert(label.clone(), (count + 1).to_string());
                let num = count + 1;

                let label_defs = self.metadata.labels.clone();
                let mut listing_vars = HashMap::new();
                listing_vars.insert("base".to_string(), self.ext.clone());
                listing_vars.insert("engine".to_string(), self.ext.clone());
                listing_vars.insert("label".to_string(), label.clone());
                listing_vars.insert("number".to_string(), num.to_string());
                listing_vars.insert("content".to_string(), rendered);
                listing_vars.insert("label_listing".to_string(), label_defs.as_ref().and_then(|l| l.listing.clone()).unwrap_or_else(|| "Listing".to_string()));
                if let Some(cap) = lst_cap {
                    listing_vars.insert("lst_cap".to_string(), cap.clone());
                }
                let tpl = self.resolve_element_partial("code_listing")
                    .unwrap_or_else(|| crate::render::elements::resolve_builtin_partial("code_listing", &self.ext).unwrap_or("").to_string());
                return self.template_env.render_dynamic("code_listing", &tpl, &listing_vars);
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
        vars.insert("engine".to_string(), self.ext.clone());

        // Run element through pipeline filters
        let code_filter = crate::render::filter::code::CodeFilter::new(&self.highlighter);
        let fig_formats = self.target.as_ref()
            .map(|t| t.fig_formats.clone())
            .filter(|f| !f.is_empty())
            .unwrap_or_else(|| crate::render::filter::figure::default_fig_formats(&self.ext));
        let figure_filter = crate::render::filter::figure::FigureFilter::new(
            self.default_fig_cap_location.clone(),
            fig_formats,
        );

        for filter in [&code_filter as &dyn Filter, &figure_filter as &dyn Filter] {
            match filter.apply(element, &self.ext, &mut vars, &self.metadata) {
                FilterResult::Rendered(output) => return output,
                FilterResult::Continue | FilterResult::Pass => {}
            }
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

    pub fn theorem_numbers(&self) -> HashMap<String, String> {
        self.theorem_numbers.borrow().clone()
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
    pub fn walk_metadata(&self) -> crate::emit::WalkMetadata {
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
pub fn resolve_element_partial(name: &str, ext: &str) -> Option<String> {
    let canonical = name.replace('-', "_");
    // Project/user filesystem resolution
    if let Some(path) = crate::paths::resolve_partial(&canonical, ext) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            return Some(content);
        }
    }
    // Built-in: discovered from embedded project tree
    resolve_builtin_partial(&canonical, ext).map(|s| s.to_string())
}
