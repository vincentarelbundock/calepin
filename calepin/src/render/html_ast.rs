//! Convert a CommonMark AST (via comrak) to HTML by walking the node tree.
//!
//! This replaces comrak's high-level `markdown_to_html()` with a custom AST
//! traversal, giving structured access to headings, images, tables, and
//! footnotes during conversion. The approach mirrors `render/latex.rs`.

use comrak::nodes::NodeValue;
use comrak::{parse_document, Arena};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

use crate::render::markdown::comrak_options;
use crate::render::markers;
use crate::util::slugify;

/// Match explicit ID/class attribute in heading: `{#some-id}` or `{.unnumbered}`
static RE_EXPLICIT_ID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s*\{([^}]+)\}\s*$").unwrap()
});

/// Strip HTML tags for plain text extraction.
static STRIP_TAGS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());


/// Convert markdown text to HTML by walking comrak's AST.
/// Math expressions and raw span output are protected from HTML escaping.
pub fn markdown_to_html_ast(
    markdown: &str,
    raw_fragments: &[String],
    number_sections: bool,
    shift_headings: bool,
) -> String {
    let preprocessed = markers::preprocess(markdown);
    let (protected, math) = markers::protect_math(&preprocessed);
    let raw = markdown_to_html_raw(&protected, number_sections, shift_headings);
    let restored = markers::restore_math(&raw, &math);
    let restored = markers::resolve_equation_labels(&restored, "html");
    let restored = markers::resolve_escaped_dollars(&restored, "html");
    markers::resolve_raw(&restored, raw_fragments)
}

/// Inner HTML conversion (no math protection).
fn markdown_to_html_raw(markdown: &str, number_sections: bool, shift_headings: bool) -> String {
    let arena = Arena::new();
    let options = comrak_options();
    let root = parse_document(&arena, markdown, &options);

    // Pre-pass: assign numeric IDs to footnotes for sequential numbering
    let mut footnote_ids: HashMap<String, usize> = HashMap::new();
    let mut fn_counter = 0usize;
    for edge in root.traverse() {
        if let comrak::arena_tree::NodeEdge::Start(node) = edge {
            match &node.data.borrow().value {
                NodeValue::FootnoteDefinition(def) => {
                    if !footnote_ids.contains_key(&def.name) {
                        fn_counter += 1;
                        footnote_ids.insert(def.name.clone(), fn_counter);
                    }
                }
                NodeValue::FootnoteReference(r) => {
                    if !footnote_ids.contains_key(&r.name) {
                        fn_counter += 1;
                        footnote_ids.insert(r.name.clone(), fn_counter);
                    }
                }
                _ => {}
            }
        }
    }

    // Find minimum heading level for section numbering baseline
    let min_level = if number_sections {
        let mut min = 6u8;
        for edge in root.traverse() {
            if let comrak::arena_tree::NodeEdge::Start(node) = edge {
                if let NodeValue::Heading(h) = &node.data.borrow().value {
                    let level = if shift_headings { (h.level + 1).min(6) } else { h.level };
                    if level < min {
                        min = level;
                    }
                }
            }
        }
        min as usize
    } else {
        1
    };

    let mut out = String::new();
    let mut state = HtmlState {
        heading_content_start: None,
        heading_level: 0,
        heading_raw_text: String::new(),
        in_heading: false,
        table_alignments: Vec::new(),
        table_cell_index: 0,
        table_in_header: false,
        number_sections,
        shift_headings,
        section_counters: [0usize; 6],
        min_heading_level: min_level,
        footnote_ids,
        footnote_defs: Vec::new(),
        in_footnote_def: false,
        footnote_def_buf: String::new(),
        footnote_def_id: 0,
        skip_image_text: false,
        list_tight: false,
    };

    for edge in root.traverse() {
        match edge {
            comrak::arena_tree::NodeEdge::Start(node) => {
                let in_list_item = is_in_list_item(node);
                let val = node.data.borrow().value.clone();
                render_entering(&val, &mut out, &mut state, in_list_item);
            }
            comrak::arena_tree::NodeEdge::End(node) => {
                let val = node.data.borrow().value.clone();
                render_leaving(&val, &mut out, &mut state);
            }
        }
    }

    // Append footnote section if any definitions were collected
    if !state.footnote_defs.is_empty() {
        out.push_str("\n<section class=\"footnotes\" data-footnotes>\n<ol>\n");
        for (id, content) in &state.footnote_defs {
            out.push_str(&format!(
                "<li id=\"fn-{}\">\n{}<a href=\"#fnref-{}\" class=\"footnote-backref\" data-footnote-backref data-footnote-backref-idx=\"{}\" aria-label=\"Back to reference {}\">↩</a>\n</li>\n",
                id, content, id, id, id
            ));
        }
        out.push_str("</ol>\n</section>\n");
    }

    out
}

struct HtmlState {
    heading_content_start: Option<usize>,
    heading_level: u8,
    heading_raw_text: String,
    in_heading: bool,
    table_alignments: Vec<comrak::nodes::TableAlignment>,
    table_cell_index: usize,
    table_in_header: bool,
    number_sections: bool,
    shift_headings: bool,
    section_counters: [usize; 6],
    min_heading_level: usize,
    footnote_ids: HashMap<String, usize>,
    footnote_defs: Vec<(usize, String)>,
    in_footnote_def: bool,
    footnote_def_buf: String,
    footnote_def_id: usize,
    skip_image_text: bool,
    list_tight: bool,
}

/// Get the target buffer: footnote def buffer if inside a footnote, else main output.
macro_rules! buf {
    ($out:expr, $state:expr) => {
        if $state.in_footnote_def { &mut $state.footnote_def_buf } else { $out }
    };
}

fn render_entering(
    val: &NodeValue,
    out: &mut String,
    state: &mut HtmlState,
    in_list_item: bool,
) {
    let buf = buf!(out, state);
    match val {
        NodeValue::Document => {}
        NodeValue::BlockQuote => {
            buf.push_str("<blockquote>\n");
        }
        NodeValue::List(nl) => {
            state.list_tight = nl.tight;
            match nl.list_type {
                comrak::nodes::ListType::Ordered => {
                    if nl.start == 1 {
                        buf.push_str("<ol>\n");
                    } else {
                        buf.push_str(&format!("<ol start=\"{}\">\n", nl.start));
                    }
                }
                _ => {
                    buf.push_str("<ul>\n");
                }
            }
        }
        NodeValue::Item(_) => {
            buf.push_str("<li>");
            if !state.list_tight {
                buf.push('\n');
            }
        }
        NodeValue::DescriptionList => {
            buf.push_str("<dl>\n");
        }
        NodeValue::DescriptionItem(_) => {}
        NodeValue::DescriptionTerm => {
            buf.push_str("<dt>");
        }
        NodeValue::DescriptionDetails => {
            buf.push_str("<dd>");
        }
        NodeValue::Heading(h) => {
            let level = if state.shift_headings { (h.level + 1).min(6) } else { h.level };
            state.heading_level = level;
            state.in_heading = true;
            state.heading_raw_text.clear();
            state.heading_content_start = Some(buf.len());
            buf.push_str(&format!("<h{}", level));
        }
        NodeValue::CodeBlock(cb) => {
            if cb.info.is_empty() {
                buf.push_str("<pre><code>");
            } else {
                let lang = cb.info.split_whitespace().next().unwrap_or("");
                buf.push_str(&format!("<pre><code class=\"language-{}\">", escape_html(lang)));
            }
            buf.push_str(&escape_html(&cb.literal));
            buf.push_str("</code></pre>\n");
        }
        NodeValue::HtmlBlock(hb) => {
            buf.push_str(&hb.literal);
        }
        NodeValue::Paragraph => {
            // In tight lists, don't wrap in <p>
            if !state.list_tight || !in_list_item {
                buf.push_str("<p>");
            }
        }
        NodeValue::ThematicBreak => {
            buf.push_str("<hr />\n");
        }
        NodeValue::Text(t) => {
            if !state.skip_image_text {
                if state.in_heading {
                    state.heading_raw_text.push_str(t);
                }
                buf.push_str(&escape_html(t));
            }
        }
        NodeValue::SoftBreak => {
            buf.push('\n');
        }
        NodeValue::LineBreak => {
            buf.push_str("<br />\n");
        }
        NodeValue::Code(c) => {
            buf.push_str("<code>");
            buf.push_str(&escape_html(&c.literal));
            buf.push_str("</code>");
        }
        NodeValue::Strong => {
            buf.push_str("<strong>");
        }
        NodeValue::Emph => {
            buf.push_str("<em>");
        }
        NodeValue::Strikethrough => {
            buf.push_str("<del>");
        }
        NodeValue::Superscript => {
            buf.push_str("<sup>");
        }
        NodeValue::Link(link) => {
            buf.push_str(&format!("<a href=\"{}\">", escape_attr(&link.url)));
        }
        NodeValue::Image(link) => {
            let resolved = crate::filters::figure::resolve_path(
                std::path::Path::new(&link.url), "html",
            );
            let url = resolved.display().to_string();
            buf.push_str(&format!("<img src=\"{}\"", escape_attr(&url)));
            buf.push_str(" alt=\"");
            // Alt text collected from child Text nodes; tag closed in render_leaving
            state.skip_image_text = true;
        }
        NodeValue::Table(table) => {
            state.table_alignments = table.alignments.clone();
            buf.push_str("<table>\n");
        }
        NodeValue::TableRow(header) => {
            state.table_cell_index = 0;
            state.table_in_header = *header;
            if *header {
                buf.push_str("<thead>\n");
            }
            buf.push_str("<tr>\n");
        }
        NodeValue::TableCell => {
            let tag = if state.table_in_header { "th" } else { "td" };
            let align = state.table_alignments
                .get(state.table_cell_index)
                .copied()
                .unwrap_or(comrak::nodes::TableAlignment::None);
            let align_attr = match align {
                comrak::nodes::TableAlignment::Left => " style=\"text-align: left\"",
                comrak::nodes::TableAlignment::Center => " style=\"text-align: center\"",
                comrak::nodes::TableAlignment::Right => " style=\"text-align: right\"",
                _ => "",
            };
            buf.push_str(&format!("<{}{}>", tag, align_attr));
            state.table_cell_index += 1;
        }
        NodeValue::FootnoteDefinition(def) => {
            let id = state.footnote_ids.get(&def.name).copied().unwrap_or(0);
            state.in_footnote_def = true;
            state.footnote_def_buf.clear();
            state.footnote_def_id = id;
        }
        NodeValue::FootnoteReference(r) => {
            let id = state.footnote_ids.get(&r.name).copied().unwrap_or(0);
            buf.push_str(&format!(
                "<sup class=\"footnote-ref\" id=\"fnref-{}\"><a href=\"#fn-{}\" data-footnote-ref>{}</a></sup>",
                id, id, id
            ));
        }
        NodeValue::HtmlInline(html) => {
            buf.push_str(html);
        }
        NodeValue::TaskItem(ti) => {
            if ti.symbol.is_some() {
                buf.push_str("<input type=\"checkbox\" checked=\"\" disabled=\"\" /> ");
            } else {
                buf.push_str("<input type=\"checkbox\" disabled=\"\" /> ");
            }
        }
        _ => {}
    }
}

fn render_leaving(val: &NodeValue, out: &mut String, state: &mut HtmlState) {
    let buf = buf!(out, state);
    match val {
        NodeValue::BlockQuote => {
            buf.push_str("</blockquote>\n");
        }
        NodeValue::List(nl) => {
            match nl.list_type {
                comrak::nodes::ListType::Ordered => buf.push_str("</ol>\n"),
                _ => buf.push_str("</ul>\n"),
            }
            state.list_tight = false;
        }
        NodeValue::Item(_) => {
            buf.push_str("</li>\n");
        }
        NodeValue::DescriptionList => {
            buf.push_str("</dl>\n");
        }
        NodeValue::DescriptionItem(_) => {}
        NodeValue::DescriptionTerm => {
            buf.push_str("</dt>\n");
        }
        NodeValue::DescriptionDetails => {
            buf.push_str("</dd>\n");
        }
        NodeValue::Heading(_) => {
            state.in_heading = false;
            let level = state.heading_level;
            if let Some(start) = state.heading_content_start.take() {
                let full = buf[start..].to_string();
                let raw_text = state.heading_raw_text.clone();
                buf.truncate(start);

                // Extract the rendered content after "<hN"
                let prefix = format!("<h{}", level);
                let rendered_content = full.strip_prefix(&prefix).unwrap_or(&full);

                // Parse explicit {#id .class} from RAW (unescaped) text
                let (id, classes, clean_content) = if let Some(caps) = RE_EXPLICIT_ID.captures(&raw_text) {
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
                        let clean_raw = RE_EXPLICIT_ID.replace(&raw_text, "");
                        id = slugify(&clean_raw);
                    }

                    // Remove the escaped {#id} from the rendered HTML content
                    // It appears as HTML-escaped: {#id} or {.class}
                    let clean = remove_trailing_attr_block_html(rendered_content);
                    (id, classes, clean)
                } else {
                    let plain = STRIP_TAGS_RE.replace_all(rendered_content, "");
                    (slugify(&plain), Vec::new(), rendered_content.to_string())
                };

                let is_unnumbered = classes.iter().any(|c| c == "unnumbered" || c == "unlisted");

                // Build section number if needed
                let section_number = if state.number_sections && !is_unnumbered {
                    let depth = (level as usize).saturating_sub(state.min_heading_level);
                    if depth < 6 {
                        state.section_counters[depth] += 1;
                        for c in state.section_counters.iter_mut().skip(depth + 1) {
                            *c = 0;
                        }
                        let number: String = state.section_counters[..=depth]
                            .iter()
                            .map(|c| c.to_string())
                            .collect::<Vec<_>>()
                            .join(".");
                        Some(number)
                    } else {
                        None
                    }
                } else {
                    None
                };

                let class_attr = if classes.is_empty() {
                    String::new()
                } else {
                    format!(" class=\"{}\"", classes.join(" "))
                };

                buf.push_str(&format!("<h{}{} id=\"{}\">", level, class_attr, id));
                if let Some(num) = section_number {
                    buf.push_str(&format!("<span class=\"section-number\">{}</span> ", num));
                }
                buf.push_str(&clean_content);
                buf.push_str(&format!("</h{}>\n", level));
            }
        }
        NodeValue::Paragraph => {
            // Check if we're in a tight list context
            if !state.list_tight || !buf.ends_with(">") || buf.ends_with("<p>") {
                // Only close <p> if we opened one
                if buf.ends_with("<p>") {
                    // Empty paragraph case
                    buf.push_str("</p>\n");
                } else {
                    buf.push_str("</p>\n");
                }
            } else {
                buf.push('\n');
            }
        }
        NodeValue::Strong => buf.push_str("</strong>"),
        NodeValue::Emph => buf.push_str("</em>"),
        NodeValue::Strikethrough => buf.push_str("</del>"),
        NodeValue::Superscript => buf.push_str("</sup>"),
        NodeValue::Link(_) => buf.push_str("</a>"),
        NodeValue::Image(_) => {
            state.skip_image_text = false;
            buf.push_str("\" />");
        }
        NodeValue::Table(_) => {
            buf.push_str("</table>\n");
        }
        NodeValue::TableRow(header) => {
            buf.push_str("</tr>\n");
            if *header {
                buf.push_str("</thead>\n<tbody>\n");
            }
        }
        NodeValue::TableCell => {
            let tag = if state.table_in_header { "th" } else { "td" };
            buf.push_str(&format!("</{}>\n", tag));
        }
        NodeValue::FootnoteDefinition(_) => {
            let content = state.footnote_def_buf.clone();
            let id = state.footnote_def_id;
            state.footnote_defs.push((id, content));
            state.in_footnote_def = false;
            state.footnote_def_buf.clear();
        }
        // Close tbody before closing table
        NodeValue::Document => {
            // Check if we have an unclosed tbody
        }
        // Leaf nodes handled entirely in render_entering
        NodeValue::CodeBlock(_)
        | NodeValue::Code(_)
        | NodeValue::Text(_)
        | NodeValue::ThematicBreak
        | NodeValue::SoftBreak
        | NodeValue::LineBreak
        | NodeValue::FootnoteReference(_)
        | NodeValue::HtmlBlock(_)
        | NodeValue::HtmlInline(_)
        | NodeValue::TaskItem(_) => {}
        _ => {}
    }
}

/// Check if a node is directly inside a list item (for tight list handling).
fn is_in_list_item(
    node: &comrak::arena_tree::Node<'_, std::cell::RefCell<comrak::nodes::Ast>>,
) -> bool {
    if let Some(parent) = node.parent() {
        matches!(parent.data.borrow().value, NodeValue::Item(_) | NodeValue::TaskItem(_))
    } else {
        false
    }
}


/// Remove trailing `{...}` attribute block from rendered heading content.
/// The block appears as HTML-escaped text (e.g., `{#sec-intro}` or `{.unnumbered}`).
fn remove_trailing_attr_block_html(content: &str) -> String {
    // In HTML, the { and } are not escaped, they appear literally
    if let Some(pos) = content.rfind('{') {
        if content[pos..].contains('}') {
            return content[..pos].trim().to_string();
        }
    }
    content.trim().to_string()
}

/// Escape HTML special characters in text content.
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// Escape for use in HTML attribute values.
fn escape_attr(s: &str) -> String {
    escape_html(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_with_id() {
        let html = markdown_to_html_ast("# Hello World", &[], false, false);
        assert!(html.contains("id=\"hello-world\""), "html: {}", html);
        assert!(html.contains("<h1"), "html: {}", html);
        assert!(html.contains("Hello World"), "html: {}", html);
    }

    #[test]
    fn test_heading_explicit_id() {
        let html = markdown_to_html_ast("## Methods {#sec-methods}", &[], false, false);
        assert!(html.contains("id=\"sec-methods\""), "html: {}", html);
        assert!(!html.contains("{#sec-methods}"), "should strip attr: {}", html);
        assert!(html.contains("Methods"), "html: {}", html);
    }

    #[test]
    fn test_heading_unnumbered() {
        let html = markdown_to_html_ast("# Appendix {.unnumbered}", &[], true, false);
        assert!(html.contains("class=\"unnumbered\""), "html: {}", html);
        assert!(!html.contains("section-number"), "should not number: {}", html);
    }

    #[test]
    fn test_section_numbering() {
        let md = "# First\n\n## Sub\n\n# Second";
        let html = markdown_to_html_ast(md, &[], true, false);
        assert!(html.contains("<span class=\"section-number\">1</span> First"), "html: {}", html);
        assert!(html.contains("<span class=\"section-number\">1.1</span> Sub"), "html: {}", html);
        assert!(html.contains("<span class=\"section-number\">2</span> Second"), "html: {}", html);
    }

    #[test]
    fn test_section_numbering_shifted() {
        let md = "# First\n\n## Sub\n\n# Second";
        let html = markdown_to_html_ast(md, &[], true, true);
        assert!(html.contains("<h2"), "should shift: {}", html);
        assert!(html.contains("<span class=\"section-number\">1</span> First"), "html: {}", html);
    }

    #[test]
    fn test_shift_headings() {
        let html = markdown_to_html_ast("# Title", &[], false, true);
        assert!(html.contains("<h2"), "should shift to h2: {}", html);
    }

    #[test]
    fn test_emphasis() {
        let html = markdown_to_html_ast("*italic* and **bold**", &[], false, false);
        assert!(html.contains("<em>italic</em>"), "html: {}", html);
        assert!(html.contains("<strong>bold</strong>"), "html: {}", html);
    }

    #[test]
    fn test_link() {
        let html = markdown_to_html_ast("[click](https://example.com)", &[], false, false);
        assert!(html.contains("<a href=\"https://example.com\">click</a>"), "html: {}", html);
    }

    #[test]
    fn test_code_block() {
        let html = markdown_to_html_ast("```python\nx = 1\n```", &[], false, false);
        assert!(html.contains("language-python"), "html: {}", html);
        assert!(html.contains("x = 1"), "html: {}", html);
    }

    #[test]
    fn test_table() {
        let md = "| A | B |\n|:--|--:|\n| 1 | 2 |";
        let html = markdown_to_html_ast(md, &[], false, false);
        assert!(html.contains("<table>"), "html: {}", html);
        assert!(html.contains("<th"), "html: {}", html);
        assert!(html.contains("text-align: left"), "html: {}", html);
        assert!(html.contains("text-align: right"), "html: {}", html);
    }

    #[test]
    fn test_footnotes() {
        let md = "Text[^1].\n\n[^1]: A note.";
        let html = markdown_to_html_ast(md, &[], false, false);
        assert!(html.contains("footnote-ref"), "html: {}", html);
        assert!(html.contains("class=\"footnotes\""), "html: {}", html);
        assert!(html.contains("A note."), "html: {}", html);
    }

    #[test]
    fn test_math_preserved() {
        let html = markdown_to_html_ast("The formula $a^2 + b^2 = c^2$ is important.", &[], false, false);
        assert!(html.contains("$a^2 + b^2 = c^2$"), "html: {}", html);
    }

    #[test]
    fn test_list() {
        let html = markdown_to_html_ast("- one\n- two", &[], false, false);
        assert!(html.contains("<ul>"), "html: {}", html);
        assert!(html.contains("<li>"), "html: {}", html);
        assert!(html.contains("one"), "html: {}", html);
    }

    #[test]
    fn test_ordered_list() {
        let html = markdown_to_html_ast("1. first\n2. second", &[], false, false);
        assert!(html.contains("<ol>"), "html: {}", html);
        assert!(html.contains("first"), "html: {}", html);
    }

    #[test]
    fn test_blockquote() {
        let html = markdown_to_html_ast("> quoted text", &[], false, false);
        assert!(html.contains("<blockquote>"), "html: {}", html);
        assert!(html.contains("quoted text"), "html: {}", html);
    }

    #[test]
    fn test_image() {
        let html = markdown_to_html_ast("![alt text](image.png)", &[], false, false);
        assert!(html.contains("<img"), "html: {}", html);
        assert!(html.contains("image.png"), "html: {}", html);
    }

    #[test]
    fn test_task_list() {
        let html = markdown_to_html_ast("- [ ] todo\n- [x] done", &[], false, false);
        assert!(html.contains("type=\"checkbox\""), "html: {}", html);
        assert!(html.contains("checked"), "html: {}", html);
    }

    #[test]
    fn test_strikethrough() {
        let html = markdown_to_html_ast("~~deleted~~", &[], false, false);
        assert!(html.contains("<del>deleted</del>"), "html: {}", html);
    }

    #[test]
    fn test_hr() {
        let html = markdown_to_html_ast("---", &[], false, false);
        assert!(html.contains("<hr"), "html: {}", html);
    }
}
