//! Convert a CommonMark AST (via comrak) to Typst by walking the node tree.
//!
//! This replaces comrak's high-level `markdown_to_typst()` with a custom AST
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

/// Match explicit ID/class attribute in heading: `{#some-id}` or `{.class}`
static RE_EXPLICIT_ID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s*\{([^}]+)\}\s*$").unwrap()
});


/// Convert markdown text to Typst by walking comrak's AST.
/// Math expressions and raw span output are protected from escaping.
pub fn markdown_to_typst_ast(markdown: &str, raw_fragments: &[String]) -> String {
    let preprocessed = markers::preprocess(markdown);
    let (protected, math) = markers::protect_math(&preprocessed);
    let raw = markdown_to_typst_raw(&protected);
    let restored = markers::restore_math(&raw, &math);
    let restored = markers::resolve_equation_labels(&restored, "typst");
    let restored = markers::resolve_escaped_dollars(&restored, "typst");
    let restored = crate::filters::math::strip_math_for_typst(&restored);
    markers::resolve_raw(&restored, raw_fragments)
}

/// Inner Typst conversion (no math protection).
fn markdown_to_typst_raw(markdown: &str) -> String {
    let arena = Arena::new();
    let options = comrak_options();
    let root = parse_document(&arena, markdown, &options);

    // Pre-pass: collect footnote definitions by extracting text from child nodes
    let mut footnote_defs: HashMap<String, String> = HashMap::new();
    for edge in root.traverse() {
        if let comrak::arena_tree::NodeEdge::Start(node) = edge {
            if let NodeValue::FootnoteDefinition(def) = &node.data.borrow().value {
                let mut text = String::new();
                collect_text_content(node, &mut text);
                footnote_defs.insert(def.name.clone(), text.trim().to_string());
            }
        }
    }

    let mut out = String::new();
    let mut state = TypstState {
        heading_content_start: None,
        heading_level: 0,
        heading_raw_text: String::new(),
        in_heading: false,
        table_alignments: Vec::new(),
        table_cell_index: 0,
        table_in_header: false,
        table_num_columns: 0,
        in_footnote_def: false,
        footnote_defs,
        skip_image_text: false,
        list_tight: false,
        in_image: false,
        image_alt: String::new(),
    };

    // Main pass: render everything except footnote definitions
    for edge in root.traverse() {
        match edge {
            comrak::arena_tree::NodeEdge::Start(node) => {
                let val = node.data.borrow().value.clone();
                if matches!(val, NodeValue::FootnoteDefinition(_)) {
                    state.in_footnote_def = true;
                    continue;
                }
                if state.in_footnote_def { continue; }
                render_entering_to_buf(&val, &mut out, &mut state);
            }
            comrak::arena_tree::NodeEdge::End(node) => {
                let val = node.data.borrow().value.clone();
                if matches!(val, NodeValue::FootnoteDefinition(_)) {
                    state.in_footnote_def = false;
                    continue;
                }
                if state.in_footnote_def { continue; }
                render_leaving_to_buf(&val, &mut out, &mut state);
            }
        }
    }

    out
}

struct TypstState {
    heading_content_start: Option<usize>,
    heading_level: u8,
    /// Raw (unescaped) heading text for ID extraction
    heading_raw_text: String,
    in_heading: bool,
    table_alignments: Vec<comrak::nodes::TableAlignment>,
    table_cell_index: usize,
    table_in_header: bool,
    table_num_columns: usize,
    in_footnote_def: bool,
    footnote_defs: HashMap<String, String>,
    skip_image_text: bool,
    list_tight: bool,
    in_image: bool,
    image_alt: String,
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

fn render_entering_to_buf(val: &NodeValue, buf: &mut String, state: &mut TypstState) {
    match val {
        NodeValue::Document => {}
        NodeValue::BlockQuote => {
            buf.push_str("#quote(block: true)[\n");
        }
        NodeValue::List(nl) => {
            state.list_tight = nl.tight;
            buf.push('\n');
        }
        NodeValue::Item(_) => {
            buf.push_str("- ");
        }
        NodeValue::DescriptionList => {}
        NodeValue::DescriptionItem(_) => {}
        NodeValue::DescriptionTerm => {
            buf.push_str("/ ");
        }
        NodeValue::DescriptionDetails => {
            buf.push_str(": ");
        }
        NodeValue::Heading(h) => {
            state.heading_level = h.level;
            state.in_heading = true;
            state.heading_raw_text.clear();
            let equals = "=".repeat(h.level as usize);
            buf.push_str(&format!("{} ", equals));
            state.heading_content_start = Some(buf.len());
        }
        NodeValue::CodeBlock(cb) => {
            if cb.info.is_empty() {
                buf.push_str("```\n");
            } else {
                let lang = cb.info.split_whitespace().next().unwrap_or("");
                buf.push_str(&format!("```{}\n", lang));
            }
            buf.push_str(&cb.literal);
            if !cb.literal.ends_with('\n') {
                buf.push('\n');
            }
            buf.push_str("```\n\n");
        }
        NodeValue::HtmlBlock(hb) => {
            // Raw HTML has limited support in Typst; pass through as-is
            buf.push_str(&hb.literal);
        }
        NodeValue::Paragraph => {}
        NodeValue::ThematicBreak => {
            buf.push_str("#line(length: 100%)\n\n");
        }
        NodeValue::Text(t) => {
            if state.in_image {
                state.image_alt.push_str(t);
            } else if !state.skip_image_text {
                if state.in_heading {
                    state.heading_raw_text.push_str(t);
                }
                buf.push_str(&escape_typst(t));
            }
        }
        NodeValue::SoftBreak => {
            if !state.in_image {
                buf.push('\n');
            }
        }
        NodeValue::LineBreak => {
            buf.push_str("\\\n");
        }
        NodeValue::Code(c) => {
            // Use raw inline code
            let backticks = if c.literal.contains('`') { "``" } else { "`" };
            buf.push_str(backticks);
            buf.push_str(&c.literal);
            buf.push_str(backticks);
        }
        NodeValue::Strong => {
            buf.push('*');
        }
        NodeValue::Emph => {
            buf.push('_');
        }
        NodeValue::Strikethrough => {
            buf.push_str("#strike[");
        }
        NodeValue::Superscript => {
            buf.push_str("#super[");
        }
        NodeValue::Link(link) => {
            buf.push_str(&format!("#link(\"{}\")[", link.url));
        }
        NodeValue::Image(_) => {
            state.in_image = true;
            state.image_alt.clear();
        }
        NodeValue::Table(table) => {
            state.table_alignments = table.alignments.clone();
            state.table_num_columns = table.num_columns as usize;
            let align_strs: Vec<&str> = table.alignments.iter().map(|a| match a {
                comrak::nodes::TableAlignment::Left => "left",
                comrak::nodes::TableAlignment::Center => "center",
                comrak::nodes::TableAlignment::Right => "right",
                _ => "auto",
            }).collect();
            buf.push_str(&format!(
                "#table(\n  columns: {},\n  align: ({}),\n",
                table.num_columns,
                align_strs.join(", ")
            ));
        }
        NodeValue::TableRow(header) => {
            state.table_cell_index = 0;
            state.table_in_header = *header;
        }
        NodeValue::TableCell => {
            buf.push_str("  [");
        }
        NodeValue::FootnoteDefinition(_) => {
            // Handled in pre-pass
        }
        NodeValue::FootnoteReference(r) => {
            let content = state.footnote_defs.get(&r.name)
                .cloned()
                .unwrap_or_default();
            buf.push_str(&format!("#footnote[{}]", content.trim()));
        }
        NodeValue::HtmlInline(html) => {
            buf.push_str(html);
        }
        NodeValue::TaskItem(ti) => {
            if ti.symbol.is_some() {
                buf.push_str("- [x] ");
            } else {
                buf.push_str("- [ ] ");
            }
        }
        _ => {}
    }
}

fn render_leaving_to_buf(val: &NodeValue, buf: &mut String, state: &mut TypstState) {
    match val {
        NodeValue::BlockQuote => {
            buf.push_str("]\n\n");
        }
        NodeValue::List(_) => {
            buf.push('\n');
            state.list_tight = false;
        }
        NodeValue::Item(_) => {
            buf.push('\n');
        }
        NodeValue::DescriptionList => {}
        NodeValue::DescriptionItem(_) => {}
        NodeValue::DescriptionTerm => {
            // The colon separator is part of the term/details syntax
        }
        NodeValue::DescriptionDetails => {
            buf.push('\n');
        }
        NodeValue::Heading(_) => {
            state.in_heading = false;
            if let Some(start) = state.heading_content_start.take() {
                let rendered_content = buf[start..].to_string();
                let raw_text = state.heading_raw_text.clone();
                buf.truncate(start);

                // Parse explicit {#id .class} from RAW (unescaped) text
                let (label, clean_rendered) = if let Some(caps) = RE_EXPLICIT_ID.captures(&raw_text) {
                    let attr_str = &caps[1];
                    let mut id = String::new();
                    for token in attr_str.split_whitespace() {
                        if let Some(stripped) = token.strip_prefix('#') {
                            id = stripped.to_string();
                        }
                    }
                    if id.is_empty() {
                        id = slugify(&RE_EXPLICIT_ID.replace(&raw_text, ""));
                    }
                    // Remove the escaped {#id} from rendered content
                    // The escaped form in Typst is \{...\} or similar
                    let clean = remove_trailing_attr_block(&rendered_content);
                    (id, clean)
                } else {
                    (slugify(&raw_text), rendered_content.trim().to_string())
                };

                buf.push_str(&clean_rendered);
                buf.push_str(&format!(" <{}>\n", label));
            } else {
                buf.push('\n');
            }
        }
        NodeValue::Paragraph => {
            buf.push_str("\n\n");
        }
        NodeValue::Strong => buf.push('*'),
        NodeValue::Emph => buf.push('_'),
        NodeValue::Strikethrough => buf.push(']'),
        NodeValue::Superscript => buf.push(']'),
        NodeValue::Link(_) => buf.push(']'),
        NodeValue::Image(link) => {
            state.in_image = false;
            let resolved = crate::filters::figure::resolve_path(
                std::path::Path::new(&link.url), "typst",
            );
            let url = resolved.display().to_string();
            buf.push_str(&format!("#box(image(\"{}\"))", url));
            state.image_alt.clear();
        }
        NodeValue::Table(_) => {
            buf.push_str(")\n\n");
        }
        NodeValue::TableRow(_) => {}
        NodeValue::TableCell => {
            buf.push_str("],\n");
            state.table_cell_index += 1;
        }
        NodeValue::FootnoteDefinition(_) => {}
        // Leaf nodes
        NodeValue::CodeBlock(_)
        | NodeValue::Code(_)
        | NodeValue::Text(_)
        | NodeValue::ThematicBreak
        | NodeValue::SoftBreak
        | NodeValue::LineBreak
        | NodeValue::FootnoteReference(_)
        | NodeValue::HtmlBlock(_)
        | NodeValue::HtmlInline(_)
        | NodeValue::TaskItem(_)
        | NodeValue::Document => {}
        _ => {}
    }
}

/// Remove trailing `\{...}` attribute block from rendered (escaped) heading content.
fn remove_trailing_attr_block(content: &str) -> String {
    // The escaped form looks like \{\#sec-intro\} or \{.unnumbered\}
    // Find the last occurrence of \{ and strip from there
    if let Some(pos) = content.rfind("\\{") {
        content[..pos].trim().to_string()
    } else if let Some(pos) = content.rfind('{') {
        content[..pos].trim().to_string()
    } else {
        content.trim().to_string()
    }
}

/// Escape special Typst characters in text.
fn escape_typst(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '#' => out.push_str("\\#"),
            '@' => out.push_str("\\@"),
            '<' => out.push_str("\\<"),
            '>' => out.push_str("\\>"),
            '\\' => out.push_str("\\\\"),
            '*' => out.push_str("\\*"),
            '_' => out.push_str("\\_"),
            '`' => out.push_str("\\`"),
            '$' => out.push_str("\\$"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_with_label() {
        let typst = markdown_to_typst_ast("# Introduction", &[]);
        assert!(typst.contains("= Introduction"), "typst: {}", typst);
        assert!(typst.contains("<introduction>"), "should have label: {}", typst);
    }

    #[test]
    fn test_heading_explicit_id() {
        let typst = markdown_to_typst_ast("# Introduction {#sec-intro}", &[]);
        assert!(typst.contains("<sec-intro>"), "should have explicit label: {}", typst);
        assert!(!typst.contains("{#sec-intro}"), "should strip attr: {}", typst);
    }

    #[test]
    fn test_emphasis() {
        let typst = markdown_to_typst_ast("*italic* and **bold**", &[]);
        assert!(typst.contains("_italic_"), "typst: {}", typst);
        assert!(typst.contains("*bold*"), "typst: {}", typst);
    }

    #[test]
    fn test_link() {
        let typst = markdown_to_typst_ast("[click](https://example.com)", &[]);
        assert!(typst.contains("#link(\"https://example.com\")[click]"), "typst: {}", typst);
    }

    #[test]
    fn test_code_block() {
        let typst = markdown_to_typst_ast("```python\nx = 1\n```", &[]);
        assert!(typst.contains("```python"), "typst: {}", typst);
        assert!(typst.contains("x = 1"), "typst: {}", typst);
    }

    #[test]
    fn test_table() {
        let md = "| A | B |\n|:--|--:|\n| 1 | 2 |";
        let typst = markdown_to_typst_ast(md, &[]);
        assert!(typst.contains("#table("), "typst: {}", typst);
        assert!(typst.contains("columns: 2"), "typst: {}", typst);
    }

    #[test]
    fn test_strikethrough() {
        let typst = markdown_to_typst_ast("~~deleted~~", &[]);
        assert!(typst.contains("#strike[deleted]"), "typst: {}", typst);
    }

    #[test]
    fn test_math_stripped_for_typst() {
        // LaTeX math ($...$) is stripped for Typst since Typst has its own math syntax.
        // The strip_math_for_typst filter removes the dollar-delimited expressions.
        let typst = markdown_to_typst_ast("The value $x^2$ is here.", &[]);
        assert!(!typst.contains("$x^2$"), "LaTeX math should be stripped: {}", typst);
    }

    #[test]
    fn test_image() {
        let typst = markdown_to_typst_ast("![alt](image.png)", &[]);
        assert!(typst.contains("image(\"image.png\")"), "typst: {}", typst);
    }
}

