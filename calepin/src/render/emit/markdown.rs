//! Markdown format emitter for the unified AST walker.
//!
//! Round-trips markdown through the AST walker so that markdown output
//! goes through the same pipeline as HTML/LaTeX/Typst, eliminating
//! format-specific conditionals in the rest of the codebase.

use comrak::nodes::TableAlignment;

use crate::render::emit::{FormatEmitter, FootnoteStrategy, HeadingAttrs, WalkOptions, walk_and_render_with_metadata};
use crate::render::convert::ImageAttrs;

pub struct MarkdownEmitter;

/// Convert markdown to markdown via the shared AST walker.
pub fn markdown_to_markdown(markdown: &str, raw_fragments: &[String]) -> String {
    markdown_to_markdown_with_counter(markdown, raw_fragments, 0).0
}

/// Convert markdown to markdown, returning (output, final_footnote_counter).
pub fn markdown_to_markdown_with_counter(
    markdown: &str,
    raw_fragments: &[String],
    footnote_counter_start: usize,
) -> (String, usize) {
    let emitter = MarkdownEmitter;
    let options = WalkOptions { footnote_counter_start, ..WalkOptions::default() };
    let result = walk_and_render_with_metadata(&emitter, markdown, raw_fragments, &options);
    (result.output, result.metadata.footnote_counter_end)
}

impl FormatEmitter for MarkdownEmitter {
    fn format_name(&self) -> &str { "markdown" }

    fn escape_text(&self, text: &str) -> String {
        text.to_string()
    }

    fn blockquote_open(&self) -> &str { "> " }
    fn blockquote_close(&self) -> &str { "\n" }

    fn list_open(&self, _ordered: bool, _start: usize, _tight: bool) -> String {
        String::new()
    }
    fn list_close(&self, _ordered: bool) -> String { "\n".to_string() }

    fn item_open(&self, _tight: bool) -> String { "- ".to_string() }
    fn item_close(&self) -> &str { "\n" }

    fn paragraph_open(&self, _in_tight_list_item: bool) -> &str { "" }
    fn paragraph_close(&self, _in_tight_list_item: bool) -> &str { "\n\n" }

    fn heading_prefix(&self, level: u8) -> String {
        format!("{} ", "#".repeat(level as usize))
    }

    fn heading(
        &self,
        _level: u8,
        attrs: &HeadingAttrs,
        rendered_content: &str,
        _section_number: Option<&str>,
    ) -> String {
        let mut out = rendered_content.to_string();
        // Emit explicit id/classes if present
        let has_explicit_id = !attrs.id.is_empty();
        let has_classes = !attrs.classes.is_empty();
        if has_explicit_id || has_classes {
            out.push_str(" {");
            if has_explicit_id {
                out.push_str(&format!("#{}", attrs.id));
            }
            for cls in &attrs.classes {
                out.push_str(&format!(" .{}", cls));
            }
            out.push('}');
        }
        out.push('\n');
        out
    }

    fn code_inline(&self, literal: &str) -> String {
        let backticks = if literal.contains('`') { "``" } else { "`" };
        if literal.contains('`') {
            format!("{} {} {}", backticks, literal, backticks)
        } else {
            format!("{}{}{}", backticks, literal, backticks)
        }
    }

    fn code_block(&self, info: &str, literal: &str) -> String {
        let mut out = format!("```{}\n", info);
        out.push_str(literal);
        if !literal.ends_with('\n') { out.push('\n'); }
        out.push_str("```\n\n");
        out
    }

    fn strong_open(&self) -> &str { "**" }
    fn strong_close(&self) -> &str { "**" }
    fn emph_open(&self) -> &str { "*" }
    fn emph_close(&self) -> &str { "*" }
    fn strikethrough_open(&self) -> &str { "~~" }
    fn strikethrough_close(&self) -> &str { "~~" }
    fn superscript_open(&self) -> &str { "^" }
    fn superscript_close(&self) -> &str { "^" }
    fn subscript_open(&self) -> &str { "~" }
    fn subscript_close(&self) -> &str { "~" }
    fn underline_open(&self) -> &str { "[" }
    fn underline_close(&self) -> &str { "]{.underline}" }
    fn highlight_open(&self) -> &str { "==" }
    fn highlight_close(&self) -> &str { "==" }

    fn link_open(&self, _url: &str) -> String {
        "[".to_string()
    }
    fn link_close(&self, url: &str) -> String { format!("]({})", url) }

    fn image(&self, url: &str, alt: &str, attrs: &ImageAttrs) -> String {
        let mut out = format!("![{}]({})", alt, url);
        // Append {width= height=} if present
        let mut attr_parts = Vec::new();
        if let Some(ref w) = attrs.width {
            attr_parts.push(format!("width={}", w));
        }
        if let Some(ref h) = attrs.height {
            attr_parts.push(format!("height={}", h));
        }
        if let Some(ref a) = attrs.fig_align {
            attr_parts.push(format!("fig-align={}", a));
        }
        for (k, v) in &attrs.extra {
            attr_parts.push(format!("{}={}", k, v));
        }
        if !attr_parts.is_empty() {
            out.push_str(&format!("{{{}}}", attr_parts.join(" ")));
        }
        out
    }

    fn table_open(&self, _alignments: &[TableAlignment]) -> String {
        String::new()
    }
    fn table_close(&self) -> &str { "\n" }

    fn table_row_open(&self, _is_header: bool) -> String { "|".to_string() }
    fn table_row_close(&self, is_header: bool) -> String {
        let mut out = "\n".to_string();
        if is_header {
            // We don't have alignments here, but the walker already called table_open with them.
            // For simplicity, emit a generic separator. The real alignment info would need
            // to be threaded through state if perfect round-tripping is needed.
            out.push_str("|---|\n");
        }
        out
    }

    fn table_cell_open(&self, _is_header: bool, _align: TableAlignment, _index: usize) -> String {
        " ".to_string()
    }
    fn table_cell_close(&self, _is_header: bool) -> String { " |".to_string() }

    fn thematic_break(&self) -> &str { "---\n\n" }
    fn soft_break(&self) -> &str { "\n" }
    fn line_break(&self) -> &str { "  \n" }

    fn footnote_strategy(&self) -> FootnoteStrategy { FootnoteStrategy::CollectToSection }

    fn footnote_ref(&self, id: usize) -> String {
        format!("[^{}]", id)
    }

    fn footnote_section(&self, defs: &[(usize, String)]) -> String {
        let mut out = String::new();
        for (id, content) in defs {
            out.push_str(&format!("[^{}]: {}\n", id, content.trim()));
        }
        out
    }

    fn html_block(&self, literal: &str) -> String { literal.to_string() }
    fn html_inline(&self, literal: &str) -> String { literal.to_string() }

    fn task_item(&self, checked: bool) -> String {
        if checked { "- [x] ".to_string() } else { "- [ ] ".to_string() }
    }

    fn description_list_open(&self) -> &str { "" }
    fn description_list_close(&self) -> &str { "\n" }
    fn description_term_open(&self) -> &str { "" }
    fn description_term_close(&self) -> &str { "\n" }
    fn description_details_open(&self) -> &str { ":   " }
    fn description_details_close(&self) -> &str { "\n" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading() {
        let md = markdown_to_markdown("# Title", &[]);
        assert!(md.contains("# Title"), "md: {}", md);
    }

    #[test]
    fn test_heading_with_id() {
        let md = markdown_to_markdown("## Methods {#sec-methods}", &[]);
        assert!(md.contains("## Methods"), "md: {}", md);
        assert!(md.contains("#sec-methods"), "should preserve id: {}", md);
    }

    #[test]
    fn test_emphasis() {
        let md = markdown_to_markdown("*italic* and **bold**", &[]);
        assert!(md.contains("*italic*"), "md: {}", md);
        assert!(md.contains("**bold**"), "md: {}", md);
    }

    #[test]
    fn test_link() {
        let md = markdown_to_markdown("[click](https://example.com)", &[]);
        assert!(md.contains("[click]"), "md: {}", md);
    }

    #[test]
    fn test_code_block() {
        let md = markdown_to_markdown("```python\nx = 1\n```", &[]);
        assert!(md.contains("```python"), "md: {}", md);
        assert!(md.contains("x = 1"), "md: {}", md);
    }

    #[test]
    fn test_strikethrough() {
        let md = markdown_to_markdown("~~deleted~~", &[]);
        assert!(md.contains("~~deleted~~"), "md: {}", md);
    }

    #[test]
    fn test_image() {
        let md = markdown_to_markdown("![alt](image.png)", &[]);
        assert!(md.contains("![alt](image.png)"), "md: {}", md);
    }
}
