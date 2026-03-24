//! Unified AST walker for all output formats.
//!
//! A single `walk_ast()` function traverses comrak's node tree and calls
//! methods on a `FormatEmitter` trait to produce format-specific output.
//! Shared logic -- heading ID extraction, footnote pre-pass, section
//! numbering, table state, math/marker protection -- lives here.

use comrak::nodes::{NodeValue, TableAlignment, ListType};
use comrak::{parse_document, Arena};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

use crate::render::convert::{build_comrak_options, ImageAttrs};
use crate::render::markers;
use crate::util::slugify;

// ---------------------------------------------------------------------------
// Heading attribute regex (shared across all formats)
// ---------------------------------------------------------------------------

/// Match `{#id .class}` at end of heading raw text.
static RE_HEADING_ATTR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s*\{([^}]+)\}\s*$").unwrap()
});

// ---------------------------------------------------------------------------
// FormatEmitter trait
// ---------------------------------------------------------------------------

/// How a format handles footnotes.
pub enum FootnoteStrategy {
    /// Render `\footnotetext[N]{...}` at def site, `\footnotemark[N]` at ref (LaTeX).
    DefAtSite,
    /// Inline footnote content at the reference point (Typst).
    InlineAtRef,
    /// Collect defs, append a footnote section at end (HTML).
    CollectToSection,
}

/// Parsed heading attributes extracted from `{#id .class}` syntax.
pub struct HeadingAttrs {
    pub id: String,
    pub classes: Vec<String>,
}

// ---------------------------------------------------------------------------
// Structured metadata collected during the AST walk
// ---------------------------------------------------------------------------

/// A heading entry collected during the AST walk.
#[derive(Debug, Clone)]
pub struct TocEntry {
    pub level: u8,
    pub id: String,
    /// Plain text content (no formatting markup).
    pub text: String,
    /// Section number string (e.g. "1.2.3"), if numbering is enabled.
    pub number: Option<String>,
    pub classes: Vec<String>,
}

/// Metadata collected as a side-product of the AST walk.
/// Avoids the need to regex-scan rendered output for IDs.
#[derive(Debug, Clone, Default)]
pub struct WalkMetadata {
    /// All headings encountered during the walk.
    pub headings: Vec<TocEntry>,
    /// All IDs encountered (headings as sec-*, figures as fig-*, tables as tbl-*, etc.).
    /// Values are sequential numbers assigned during the walk.
    pub ids: HashMap<String, String>,
    /// Final footnote counter value (for chaining across multiple Text elements).
    pub footnote_counter_end: usize,
    /// Section counters at end of walk (for chaining across multiple Text elements).
    pub section_counters_end: [usize; 6],
    /// Minimum heading level seen (for chaining).
    pub min_heading_level: usize,
    /// Collected footnote definitions (for deferred rendering).
    pub footnote_defs: Vec<(usize, String)>,
}

/// Result of `walk_and_render`: the rendered string plus collected metadata.
pub struct WalkResult {
    pub output: String,
    pub metadata: WalkMetadata,
}

/// Format-specific string emission. Each method returns the string(s) to
/// write for a given AST node. The walker handles all shared state.
pub trait FormatEmitter {
    fn format_name(&self) -> &str;

    // -- Text escaping --
    fn escape_text(&self, text: &str) -> String;

    // -- Block containers --
    fn blockquote_open(&self) -> &str;
    fn blockquote_close(&self) -> &str;
    fn list_open(&self, ordered: bool, start: usize, tight: bool) -> String;
    fn list_close(&self, ordered: bool) -> String;
    fn item_open(&self, tight: bool) -> String;
    fn item_close(&self) -> &str;
    fn paragraph_open(&self, in_tight_list_item: bool) -> &str;
    fn paragraph_close(&self, in_tight_list_item: bool) -> &str;

    // -- Heading --
    // Receives pre-rendered inline content plus parsed attributes.
    fn heading(
        &self,
        level: u8,
        attrs: &HeadingAttrs,
        rendered_content: &str,
        section_number: Option<&str>,
    ) -> String;
    /// Called before heading content is rendered (e.g. to emit `\section{`).
    /// Content will be appended to the buffer, then `heading()` finalizes.
    fn heading_prefix(&self, level: u8) -> String;

    // -- Code --
    fn code_inline(&self, literal: &str) -> String;
    fn code_block(&self, info: &str, literal: &str) -> String;

    // -- Inline formatting --
    fn strong_open(&self) -> &str;
    fn strong_close(&self) -> &str;
    fn emph_open(&self) -> &str;
    fn emph_close(&self) -> &str;
    fn strikethrough_open(&self) -> &str;
    fn strikethrough_close(&self) -> &str;
    fn superscript_open(&self) -> &str;
    fn superscript_close(&self) -> &str;
    fn subscript_open(&self) -> &str;
    fn subscript_close(&self) -> &str;
    fn underline_open(&self) -> &str;
    fn underline_close(&self) -> &str;
    fn highlight_open(&self) -> &str;
    fn highlight_close(&self) -> &str;

    // -- Links & images --
    fn link_open(&self, url: &str) -> String;
    fn link_close(&self, url: &str) -> String;
    fn image(&self, url: &str, alt: &str, attrs: &ImageAttrs) -> String;

    // -- Table --
    fn table_open(&self, alignments: &[TableAlignment]) -> String;
    fn table_close(&self) -> &str;
    fn table_row_open(&self, is_header: bool) -> String;
    fn table_row_close(&self, is_header: bool) -> String;
    fn table_cell_open(&self, is_header: bool, align: TableAlignment, index: usize) -> String;
    fn table_cell_close(&self, is_header: bool) -> String;

    // -- Breaks --
    fn thematic_break(&self) -> &str;
    fn soft_break(&self) -> &str;
    fn line_break(&self) -> &str;

    // -- Footnotes --
    fn footnote_strategy(&self) -> FootnoteStrategy;
    /// Reference marker (HTML/LaTeX).
    fn footnote_ref(&self, id: usize) -> String;
    /// Inline footnote with content (Typst).
    fn footnote_ref_with_content(&self, _id: usize, content: &str) -> String {
        let _ = content;
        String::new()
    }
    /// Open a footnote definition block (LaTeX).
    fn footnote_def_open(&self, id: usize) -> String { let _ = id; String::new() }
    fn footnote_def_close(&self) -> &str { "" }
    /// Render the collected footnote section (HTML).
    fn footnote_section(&self, _defs: &[(usize, String)]) -> String { String::new() }

    // -- Raw HTML --
    fn html_block(&self, literal: &str) -> String;
    fn html_inline(&self, literal: &str) -> String;

    // -- Tasks --
    fn task_item(&self, checked: bool) -> String;

    // -- Description lists --
    fn description_list_open(&self) -> &str { "<dl>\n" }
    fn description_list_close(&self) -> &str { "</dl>\n" }
    fn description_term_open(&self) -> &str { "<dt>" }
    fn description_term_close(&self) -> &str { "</dt>\n" }
    fn description_details_open(&self) -> &str { "<dd>" }
    fn description_details_close(&self) -> &str { "</dd>\n" }
}

// ---------------------------------------------------------------------------
// Walk options
// ---------------------------------------------------------------------------

pub struct WalkOptions {
    pub number_sections: bool,
    pub shift_headings: bool,
    /// Starting footnote counter (for documents with multiple Text elements).
    pub footnote_counter_start: usize,
    /// Section counters carried from a previous Text element. None = start fresh.
    pub section_counters_start: Option<[usize; 6]>,
    /// Minimum heading level carried from a previous Text element. None = compute via pre-pass.
    pub min_heading_level: Option<usize>,
    /// When true, don't append the footnote section inline; defs are returned in metadata.
    pub suppress_footnote_section: bool,
}

impl Default for WalkOptions {
    fn default() -> Self {
        Self {
            number_sections: false,
            shift_headings: false,
            footnote_counter_start: 0,
            section_counters_start: None,
            min_heading_level: None,
            suppress_footnote_section: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Convert markdown and return both rendered output and collected metadata
/// (headings for TOC, IDs for cross-references).
pub fn walk_and_render_with_metadata(
    emitter: &dyn FormatEmitter,
    markdown: &str,
    raw_fragments: &[String],
    options: &WalkOptions,
) -> WalkResult {
    let preprocessed = markers::preprocess_markdown(markdown);
    let (protected, math) = markers::protect_math(&preprocessed);
    let (raw, metadata) = walk_ast(emitter, &protected, options);
    let restored = markers::restore_math(&raw, &math);
    let fmt = emitter.format_name();
    let restored = markers::resolve_equation_labels(&restored, fmt);
    let restored = markers::resolve_escaped_dollars(&restored, fmt);
    let output = markers::resolve_raw(&restored, raw_fragments);
    WalkResult { output, metadata }
}

// ---------------------------------------------------------------------------
// Walker state
// ---------------------------------------------------------------------------

struct WalkState {
    heading_content_start: Option<usize>,
    heading_level: u8,
    heading_raw_text: String,
    in_heading: bool,

    table_alignments: Vec<TableAlignment>,
    table_cell_index: usize,
    table_in_header: bool,

    number_sections: bool,
    shift_headings: bool,
    section_counters: [usize; 6],
    min_heading_level: usize,

    footnote_ids: HashMap<String, usize>,
    /// Names of footnotes that have actual references (not just global defs).
    footnote_ref_names: std::collections::HashSet<String>,
    /// Pre-collected footnote text (for InlineAtRef strategy).
    footnote_text: HashMap<String, String>,
    /// Collected rendered footnote defs (for CollectToSection strategy).
    footnote_defs: Vec<(usize, String)>,
    in_footnote_def: bool,
    footnote_def_buf: String,
    footnote_def_id: usize,
    footnote_def_name: String,

    skip_image_text: bool,
    image_alt: String,
    /// Buffered image waiting for possible `{attrs}` in the next text node.
    pending_image: Option<PendingImage>,
    list_tight: bool,
    in_tight_list_item: bool,

    /// Collected metadata (headings, IDs).
    meta: WalkMetadata,
}

struct PendingImage {
    url: String,
    alt: String,
}

// ---------------------------------------------------------------------------
// Core AST walk
// ---------------------------------------------------------------------------

fn walk_ast(emitter: &dyn FormatEmitter, markdown: &str, options: &WalkOptions) -> (String, WalkMetadata) {
    let arena = Arena::new();
    let comrak_opts = build_comrak_options();
    let root = parse_document(&arena, markdown, &comrak_opts);

    // Pre-pass 1: assign sequential footnote IDs (continuing from previous Text elements).
    // First pass: collect ref names and assign IDs to refs only (so refs get
    // sequential numbers without gaps from globally-appended defs).
    let mut footnote_ids: HashMap<String, usize> = HashMap::new();
    let mut footnote_ref_names = std::collections::HashSet::new();
    let mut fn_counter = options.footnote_counter_start;
    for edge in root.traverse() {
        if let comrak::arena_tree::NodeEdge::Start(node) = edge {
            if let NodeValue::FootnoteReference(r) = &node.data.borrow().value {
                footnote_ref_names.insert(r.name.clone());
                if !footnote_ids.contains_key(&r.name) {
                    fn_counter += 1;
                    footnote_ids.insert(r.name.clone(), fn_counter);
                }
            }
        }
    }
    // Record the ref-only counter for chaining to the next walk.
    let fn_counter_refs = fn_counter;
    // Second: assign IDs to def-only footnotes (needed for comrak to render them,
    // but these won't produce visible output since they have no refs).
    // Use a separate counter so these don't create gaps in ref numbering.
    let mut fn_counter_defs = fn_counter;
    for edge in root.traverse() {
        if let comrak::arena_tree::NodeEdge::Start(node) = edge {
            if let NodeValue::FootnoteDefinition(def) = &node.data.borrow().value {
                if !footnote_ids.contains_key(&def.name) {
                    fn_counter_defs += 1;
                    footnote_ids.insert(def.name.clone(), fn_counter_defs);
                }
            }
        }
    }

    // Pre-pass 2: for InlineAtRef strategy, collect footnote plain text
    let footnote_text = match emitter.footnote_strategy() {
        FootnoteStrategy::InlineAtRef => {
            let mut map = HashMap::new();
            for edge in root.traverse() {
                if let comrak::arena_tree::NodeEdge::Start(node) = edge {
                    if let NodeValue::FootnoteDefinition(def) = &node.data.borrow().value {
                        let mut text = String::new();
                        collect_text_content(node, &mut text);
                        map.insert(def.name.clone(), text.trim().to_string());
                    }
                }
            }
            map
        }
        _ => HashMap::new(),
    };

    // Pre-pass 3: find minimum heading level for numbering baseline
    let min_level = if let Some(ml) = options.min_heading_level {
        ml
    } else if options.number_sections {
        let mut min = 6u8;
        for edge in root.traverse() {
            if let comrak::arena_tree::NodeEdge::Start(node) = edge {
                if let NodeValue::Heading(h) = &node.data.borrow().value {
                    let level = if options.shift_headings { (h.level + 1).min(6) } else { h.level };
                    if level < min { min = level; }
                }
            }
        }
        min as usize
    } else {
        1
    };

    let mut out = String::new();
    let mut state = WalkState {
        heading_content_start: None,
        heading_level: 0,
        heading_raw_text: String::new(),
        in_heading: false,
        table_alignments: Vec::new(),
        table_cell_index: 0,
        table_in_header: false,
        number_sections: options.number_sections,
        shift_headings: options.shift_headings,
        section_counters: options.section_counters_start.unwrap_or([0; 6]),
        min_heading_level: min_level,
        footnote_ids,
        footnote_ref_names,
        footnote_text,
        footnote_defs: Vec::new(),
        in_footnote_def: false,
        footnote_def_buf: String::new(),
        footnote_def_id: 0,
        footnote_def_name: String::new(),
        skip_image_text: false,
        image_alt: String::new(),
        pending_image: None,
        list_tight: false,
        in_tight_list_item: false,
        meta: WalkMetadata::default(),
    };

    // Main traversal
    for edge in root.traverse() {
        match edge {
            comrak::arena_tree::NodeEdge::Start(node) => {
                let in_list_item = is_in_list_item(node);
                let val = node.data.borrow().value.clone();
                emit_entering(emitter, &val, &mut out, &mut state, in_list_item);
            }
            comrak::arena_tree::NodeEdge::End(node) => {
                let val = node.data.borrow().value.clone();
                emit_leaving(emitter, &val, &mut out, &mut state);
            }
        }
    }

    // Flush any remaining pending image
    if let Some(img) = state.pending_image.take() {
        out.push_str(&emitter.image(&img.url, &img.alt, &ImageAttrs::empty()));
    }

    // Append footnote section if strategy is CollectToSection (unless suppressed)
    if let FootnoteStrategy::CollectToSection = emitter.footnote_strategy() {
        if !state.footnote_defs.is_empty() {
            if options.suppress_footnote_section {
                state.meta.footnote_defs = std::mem::take(&mut state.footnote_defs);
            } else {
                out.push_str(&emitter.footnote_section(&state.footnote_defs));
            }
        }
    }

    state.meta.footnote_counter_end = fn_counter_refs;
    state.meta.section_counters_end = state.section_counters;
    state.meta.min_heading_level = state.min_heading_level;
    (out, state.meta)
}

// ---------------------------------------------------------------------------
// Enter/leave dispatch
// ---------------------------------------------------------------------------

/// Regex for detecting `{key=value ...}` at the start of text following an image.
static RE_IMG_ATTR_BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\{([^}]+)\}").unwrap()
});

fn emit_entering(
    e: &dyn FormatEmitter,
    val: &NodeValue,
    out: &mut String,
    s: &mut WalkState,
    in_list_item: bool,
) {
    // Flush pending image before any non-text node (except Image itself).
    if !matches!(val, NodeValue::Text(_) | NodeValue::Image(_)) {
        if let Some(img) = s.pending_image.take() {
            let empty_attrs = ImageAttrs::empty();
            let rendered = e.image(&img.url, &img.alt, &empty_attrs);
            if s.in_footnote_def { s.footnote_def_buf.push_str(&rendered); }
            else { out.push_str(&rendered); }
        }
    }

    // Route output to footnote buffer if collecting a def
    let buf = if s.in_footnote_def { &mut s.footnote_def_buf } else { out };

    match val {
        NodeValue::Document => {}
        NodeValue::BlockQuote => buf.push_str(e.blockquote_open()),
        NodeValue::List(nl) => {
            s.list_tight = nl.tight;
            let ordered = nl.list_type == ListType::Ordered;
            buf.push_str(&e.list_open(ordered, nl.start, nl.tight));
        }
        NodeValue::Item(_) => {
            s.in_tight_list_item = s.list_tight;
            buf.push_str(&e.item_open(s.list_tight));
        }
        NodeValue::DescriptionList => buf.push_str(e.description_list_open()),
        NodeValue::DescriptionItem(_) => {}
        NodeValue::DescriptionTerm => buf.push_str(e.description_term_open()),
        NodeValue::DescriptionDetails => buf.push_str(e.description_details_open()),
        NodeValue::Heading(h) => {
            let level = if s.shift_headings { (h.level + 1).min(6) } else { h.level };
            s.heading_level = level;
            s.in_heading = true;
            s.heading_raw_text.clear();
            buf.push_str(&e.heading_prefix(level));
            s.heading_content_start = Some(buf.len());
        }
        NodeValue::CodeBlock(cb) => {
            buf.push_str(&e.code_block(&cb.info, &cb.literal));
        }
        NodeValue::HtmlBlock(hb) => {
            buf.push_str(&e.html_block(&hb.literal));
        }
        NodeValue::Paragraph => {
            let tight = s.list_tight && in_list_item;
            buf.push_str(e.paragraph_open(tight));
        }
        NodeValue::ThematicBreak => buf.push_str(e.thematic_break()),
        NodeValue::Text(t) => {
            if s.skip_image_text {
                s.image_alt.push_str(t);
            } else if let Some(img) = s.pending_image.take() {
                // Check if this text starts with {key=value} image attributes
                if let Some(caps) = RE_IMG_ATTR_BLOCK.captures(t) {
                    let attrs_str = &caps[1];
                    let attrs = ImageAttrs::parse(attrs_str);
                    buf.push_str(&e.image(&img.url, &img.alt, &attrs));
                    // Emit any remaining text after the {attrs} block
                    let remainder = &t[caps.get(0).unwrap().end()..];
                    if !remainder.is_empty() {
                        if s.in_heading {
                            s.heading_raw_text.push_str(remainder);
                        }
                        buf.push_str(&e.escape_text(remainder));
                    }
                } else {
                    // Not an attr block -- emit image without attrs, then the text
                    let empty_attrs = ImageAttrs::empty();
                    buf.push_str(&e.image(&img.url, &img.alt, &empty_attrs));
                    if s.in_heading {
                        s.heading_raw_text.push_str(t);
                    }
                    buf.push_str(&e.escape_text(t));
                }
            } else {
                if s.in_heading {
                    s.heading_raw_text.push_str(t);
                }
                buf.push_str(&e.escape_text(t));
            }
        }
        NodeValue::SoftBreak => buf.push_str(e.soft_break()),
        NodeValue::LineBreak => buf.push_str(e.line_break()),
        NodeValue::Code(c) => buf.push_str(&e.code_inline(&c.literal)),
        NodeValue::Strong => buf.push_str(e.strong_open()),
        NodeValue::Emph => buf.push_str(e.emph_open()),
        NodeValue::Strikethrough => buf.push_str(e.strikethrough_open()),
        NodeValue::Superscript => buf.push_str(e.superscript_open()),
        NodeValue::Subscript => buf.push_str(e.subscript_open()),
        NodeValue::Underline => buf.push_str(e.underline_open()),
        NodeValue::Highlight => buf.push_str(e.highlight_open()),
        NodeValue::Link(link) => buf.push_str(&e.link_open(&link.url)),
        NodeValue::Image(_) => {
            s.skip_image_text = true;
            s.image_alt.clear();
            // Image tag is emitted in leave (after collecting alt text)
        }
        NodeValue::Table(table) => {
            s.table_alignments = table.alignments.clone();
            buf.push_str(&e.table_open(&table.alignments));
        }
        NodeValue::TableRow(header) => {
            s.table_cell_index = 0;
            s.table_in_header = *header;
            buf.push_str(&e.table_row_open(*header));
        }
        NodeValue::TableCell => {
            let align = s.table_alignments
                .get(s.table_cell_index)
                .copied()
                .unwrap_or(TableAlignment::None);
            buf.push_str(&e.table_cell_open(s.table_in_header, align, s.table_cell_index));
            s.table_cell_index += 1;
        }
        NodeValue::FootnoteDefinition(def) => {
            match e.footnote_strategy() {
                FootnoteStrategy::DefAtSite => {
                    let id = s.footnote_ids.get(&def.name).copied().unwrap_or(0);
                    buf.push_str(&e.footnote_def_open(id));
                }
                FootnoteStrategy::CollectToSection => {
                    let id = s.footnote_ids.get(&def.name).copied().unwrap_or(0);
                    s.in_footnote_def = true;
                    s.footnote_def_buf.clear();
                    s.footnote_def_id = id;
                    s.footnote_def_name = def.name.clone();
                }
                FootnoteStrategy::InlineAtRef => {
                    // Skip def entirely; content was pre-collected
                    s.in_footnote_def = true;
                    s.footnote_def_buf.clear();
                }
            }
        }
        NodeValue::FootnoteReference(r) => {
            let id = s.footnote_ids.get(&r.name).copied().unwrap_or(0);
            match e.footnote_strategy() {
                FootnoteStrategy::InlineAtRef => {
                    let content = s.footnote_text.get(&r.name)
                        .cloned()
                        .unwrap_or_default();
                    buf.push_str(&e.footnote_ref_with_content(id, &content));
                }
                _ => {
                    buf.push_str(&e.footnote_ref(id));
                }
            }
        }
        NodeValue::HtmlInline(html) => buf.push_str(&e.html_inline(html)),
        NodeValue::TaskItem(ti) => buf.push_str(&e.task_item(ti.symbol.is_some())),
        _ => {}
    }
}

fn emit_leaving(
    e: &dyn FormatEmitter,
    val: &NodeValue,
    out: &mut String,
    s: &mut WalkState,
) {
    let buf = if s.in_footnote_def { &mut s.footnote_def_buf } else { out };

    match val {
        NodeValue::BlockQuote => buf.push_str(e.blockquote_close()),
        NodeValue::List(nl) => {
            let ordered = nl.list_type == ListType::Ordered;
            buf.push_str(&e.list_close(ordered));
            s.list_tight = false;
        }
        NodeValue::Item(_) => {
            buf.push_str(e.item_close());
            s.in_tight_list_item = false;
        }
        NodeValue::DescriptionList => buf.push_str(e.description_list_close()),
        NodeValue::DescriptionItem(_) => {}
        NodeValue::DescriptionTerm => buf.push_str(e.description_term_close()),
        NodeValue::DescriptionDetails => buf.push_str(e.description_details_close()),
        NodeValue::Heading(_) => {
            s.in_heading = false;
            let level = s.heading_level;
            if let Some(start) = s.heading_content_start.take() {
                let rendered = buf[start..].to_string();
                buf.truncate(start);

                // Parse {#id .class} from raw (unescaped) text
                let raw = &s.heading_raw_text;
                let (attrs, clean_content) = parse_heading_attrs(raw, &rendered);

                // Section numbering
                let section_number = if s.number_sections
                    && !attrs.classes.iter().any(|c| c == "unnumbered" || c == "unlisted")
                {
                    let depth = (level as usize).saturating_sub(s.min_heading_level);
                    if depth < 6 {
                        s.section_counters[depth] += 1;
                        for c in s.section_counters.iter_mut().skip(depth + 1) {
                            *c = 0;
                        }
                        Some(
                            s.section_counters[..=depth]
                                .iter()
                                .map(|c| c.to_string())
                                .collect::<Vec<_>>()
                                .join("."),
                        )
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Collect metadata: TOC entry
                let is_unlisted = attrs.classes.iter().any(|c| c == "unlisted");
                if !is_unlisted {
                    s.meta.headings.push(TocEntry {
                        level,
                        id: attrs.id.clone(),
                        text: RE_HEADING_ATTR.replace(&s.heading_raw_text, "").trim().to_string(),
                        number: section_number.clone(),
                        classes: attrs.classes.clone(),
                    });
                }

                // Collect metadata: section ID
                if !attrs.id.is_empty() {
                    let sec_key = if attrs.id.starts_with("sec-") {
                        attrs.id.clone()
                    } else {
                        format!("sec-{}", &attrs.id)
                    };
                    if let Some(ref num) = section_number {
                        s.meta.ids.insert(sec_key, num.clone());
                    }
                }

                buf.push_str(&e.heading(
                    level,
                    &attrs,
                    &clean_content,
                    section_number.as_deref(),
                ));
            }
        }
        NodeValue::Paragraph => {
            let tight = s.list_tight && s.in_tight_list_item;
            buf.push_str(e.paragraph_close(tight));
        }
        NodeValue::Strong => buf.push_str(e.strong_close()),
        NodeValue::Emph => buf.push_str(e.emph_close()),
        NodeValue::Strikethrough => buf.push_str(e.strikethrough_close()),
        NodeValue::Superscript => buf.push_str(e.superscript_close()),
        NodeValue::Subscript => buf.push_str(e.subscript_close()),
        NodeValue::Underline => buf.push_str(e.underline_close()),
        NodeValue::Highlight => buf.push_str(e.highlight_close()),
        NodeValue::Link(link) => buf.push_str(&e.link_close(&link.url)),
        NodeValue::Image(link) => {
            s.skip_image_text = false;
            let alt = s.image_alt.clone();
            s.image_alt.clear();
            // Buffer the image; the next Text node may have {attrs}
            s.pending_image = Some(PendingImage {
                url: link.url.clone(),
                alt,
            });
        }
        NodeValue::Table(_) => buf.push_str(e.table_close()),
        NodeValue::TableRow(header) => buf.push_str(&e.table_row_close(*header)),
        NodeValue::TableCell => buf.push_str(&e.table_cell_close(s.table_in_header)),
        NodeValue::FootnoteDefinition(_) => {
            match e.footnote_strategy() {
                FootnoteStrategy::DefAtSite => {
                    buf.push_str(e.footnote_def_close());
                }
                FootnoteStrategy::CollectToSection => {
                    // Only collect defs for footnotes that were actually referenced
                    // in this Text block (not globally-appended defs from other blocks).
                    // Deduplicate by ID to avoid double-collecting when global defs
                    // duplicate a def already present in this block.
                    let id = s.footnote_def_id;
                    if s.footnote_ref_names.contains(&s.footnote_def_name)
                        && !s.footnote_defs.iter().any(|(existing_id, _)| *existing_id == id)
                    {
                        let content = s.footnote_def_buf.clone();
                        s.footnote_defs.push((id, content));
                    }
                    s.in_footnote_def = false;
                    s.footnote_def_buf.clear();
                }
                FootnoteStrategy::InlineAtRef => {
                    s.in_footnote_def = false;
                    s.footnote_def_buf.clear();
                }
            }
        }
        NodeValue::Document => {}
        // Leaf nodes handled entirely in emit_entering
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn is_in_list_item(
    node: &comrak::arena_tree::Node<'_, std::cell::RefCell<comrak::nodes::Ast>>,
) -> bool {
    if let Some(parent) = node.parent() {
        matches!(parent.data.borrow().value, NodeValue::Item(_) | NodeValue::TaskItem(_))
    } else {
        false
    }
}

/// Recursively collect plain text content from a node's descendants.
fn collect_text_content<'a>(
    node: &'a comrak::arena_tree::Node<'a, std::cell::RefCell<comrak::nodes::Ast>>,
    out: &mut String,
) {
    for child in node.children() {
        let val = child.data.borrow().value.clone();
        match val {
            NodeValue::Text(t) => out.push_str(&t),
            NodeValue::Code(c) => {
                out.push('`');
                out.push_str(&c.literal);
                out.push('`');
            }
            NodeValue::SoftBreak | NodeValue::LineBreak => out.push(' '),
            _ => collect_text_content(child, out),
        }
    }
}

/// Parse `{#id .class1 .class2}` from raw heading text.
/// Returns parsed attrs and the cleaned rendered content (with the attr block removed).
fn parse_heading_attrs(raw_text: &str, rendered_content: &str) -> (HeadingAttrs, String) {
    if let Some(caps) = RE_HEADING_ATTR.captures(raw_text) {
        let attr_str = &caps[1];
        let mut id = String::new();
        let mut classes = Vec::new();
        for token in attr_str.split_whitespace() {
            if let Some(stripped) = token.strip_prefix('#') {
                id = stripped.to_string();
            } else if let Some(stripped) = token.strip_prefix('.') {
                classes.push(stripped.to_string());
            }
        }
        if id.is_empty() {
            let clean_raw = RE_HEADING_ATTR.replace(raw_text, "");
            id = slugify(&clean_raw);
        }

        // Strip the trailing {#id .class} from the rendered content.
        // It may be escaped differently per format, so find the last `{` or `\{`.
        let clean = strip_trailing_attr_block(rendered_content);
        (HeadingAttrs { id, classes }, clean)
    } else {
        let id = slugify(raw_text);
        (
            HeadingAttrs { id, classes: Vec::new() },
            rendered_content.trim().to_string(),
        )
    }
}

/// Remove a trailing `{...}` or `\{...\}` attribute block from rendered content.
fn strip_trailing_attr_block(content: &str) -> String {
    // Try escaped form first: \{...\}
    if let Some(pos) = content.rfind("\\{") {
        return content[..pos].trim().to_string();
    }
    // Then plain form: {...}
    if let Some(pos) = content.rfind('{') {
        if content[pos..].contains('}') {
            return content[..pos].trim().to_string();
        }
    }
    content.trim().to_string()
}
