//! Typst format emitter for the unified AST walker.

use comrak::nodes::TableAlignment;

use crate::render::ast::{FormatEmitter, FootnoteStrategy, HeadingAttrs, WalkOptions, walk_and_render_with_metadata};
use crate::render::markdown::ImageAttrs;

pub struct TypstEmitter;

/// Convert markdown to Typst via the shared AST walker.
pub fn markdown_to_typst_ast(markdown: &str, raw_fragments: &[String]) -> String {
    markdown_to_typst_with_counter(markdown, raw_fragments, 0).0
}

/// Convert markdown to Typst, returning (output, final_footnote_counter).
pub fn markdown_to_typst_with_counter(
    markdown: &str,
    raw_fragments: &[String],
    footnote_counter_start: usize,
) -> (String, usize) {
    let emitter = TypstEmitter;
    let options = WalkOptions { footnote_counter_start, ..WalkOptions::default() };
    let result = walk_and_render_with_metadata(&emitter, markdown, raw_fragments, &options);
    let output = crate::filters::math::strip_math_for_typst(&result.output);
    (output, result.metadata.footnote_counter_end)
}

impl FormatEmitter for TypstEmitter {
    fn format_name(&self) -> &str { "typst" }

    fn escape_text(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for c in text.chars() {
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

    fn blockquote_open(&self) -> &str { "#quote(block: true)[\n" }
    fn blockquote_close(&self) -> &str { "]\n\n" }

    fn list_open(&self, _ordered: bool, _start: usize, _tight: bool) -> String {
        "\n".to_string()
    }
    fn list_close(&self, _ordered: bool) -> String { "\n".to_string() }

    fn item_open(&self, _tight: bool) -> String { "- ".to_string() }
    fn item_close(&self) -> &str { "\n" }

    fn paragraph_open(&self, _in_tight_list_item: bool) -> &str { "" }
    fn paragraph_close(&self, _in_tight_list_item: bool) -> &str { "\n\n" }

    fn heading_prefix(&self, level: u8) -> String {
        format!("{} ", "=".repeat(level as usize))
    }

    fn heading(
        &self,
        _level: u8,
        attrs: &HeadingAttrs,
        rendered_content: &str,
        _section_number: Option<&str>,
    ) -> String {
        format!("{} <{}>\n", rendered_content, attrs.id)
    }

    fn code_inline(&self, literal: &str) -> String {
        let backticks = if literal.contains('`') { "``" } else { "`" };
        format!("{}{}{}", backticks, literal, backticks)
    }

    fn code_block(&self, info: &str, literal: &str) -> String {
        let lang = if info.is_empty() { "" } else { info.split_whitespace().next().unwrap_or("") };
        let mut out = format!("```{}\n", lang);
        out.push_str(literal);
        if !literal.ends_with('\n') { out.push('\n'); }
        out.push_str("```\n\n");
        out
    }

    fn strong_open(&self) -> &str { "*" }
    fn strong_close(&self) -> &str { "*" }
    fn emph_open(&self) -> &str { "_" }
    fn emph_close(&self) -> &str { "_" }
    fn strikethrough_open(&self) -> &str { "#strike[" }
    fn strikethrough_close(&self) -> &str { "]" }
    fn superscript_open(&self) -> &str { "#super[" }
    fn superscript_close(&self) -> &str { "]" }
    fn subscript_open(&self) -> &str { "#sub[" }
    fn subscript_close(&self) -> &str { "]" }
    fn underline_open(&self) -> &str { "#underline[" }
    fn underline_close(&self) -> &str { "]" }
    fn highlight_open(&self) -> &str { "#highlight[" }
    fn highlight_close(&self) -> &str { "]" }

    fn link_open(&self, url: &str) -> String {
        format!("#link(\"{}\")[", url)
    }
    fn link_close(&self) -> &str { "]" }

    fn image(&self, url: &str, _alt: &str, attrs: &ImageAttrs) -> String {
        let resolved = crate::filters::figure::select_image_variant(
            std::path::Path::new(url), "typst",
        );
        let params = attrs.to_typst_params();
        format!("#box(image(\"{}\"{}))", resolved.display(), params)
    }

    fn table_open(&self, alignments: &[TableAlignment]) -> String {
        let align_strs: Vec<&str> = alignments.iter().map(|a| match a {
            TableAlignment::Left => "left",
            TableAlignment::Center => "center",
            TableAlignment::Right => "right",
            _ => "auto",
        }).collect();
        format!(
            "#table(\n  columns: {},\n  align: ({}),\n",
            alignments.len(),
            align_strs.join(", ")
        )
    }
    fn table_close(&self) -> &str { ")\n\n" }

    fn table_row_open(&self, _is_header: bool) -> String { String::new() }
    fn table_row_close(&self, _is_header: bool) -> String { String::new() }

    fn table_cell_open(&self, _is_header: bool, _align: TableAlignment, _index: usize) -> String {
        "  [".to_string()
    }
    fn table_cell_close(&self, _is_header: bool) -> String { "],\n".to_string() }

    fn thematic_break(&self) -> &str { "#line(length: 100%)\n\n" }
    fn soft_break(&self) -> &str { "\n" }
    fn line_break(&self) -> &str { "\\\n" }

    fn footnote_strategy(&self) -> FootnoteStrategy { FootnoteStrategy::InlineAtRef }

    fn footnote_ref(&self, _id: usize) -> String { String::new() }

    fn footnote_ref_with_content(&self, _id: usize, content: &str) -> String {
        format!("#footnote[{}]", content.trim())
    }

    fn html_block(&self, literal: &str) -> String { literal.to_string() }
    fn html_inline(&self, literal: &str) -> String { literal.to_string() }

    fn task_item(&self, checked: bool) -> String {
        if checked { "- [x] ".to_string() } else { "- [ ] ".to_string() }
    }

    fn description_list_open(&self) -> &str { "" }
    fn description_list_close(&self) -> &str { "" }
    fn description_term_open(&self) -> &str { "/ " }
    fn description_term_close(&self) -> &str { "" }
    fn description_details_open(&self) -> &str { ": " }
    fn description_details_close(&self) -> &str { "\n" }
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
        let typst = markdown_to_typst_ast("The value $x^2$ is here.", &[]);
        assert!(!typst.contains("$x^2$"), "LaTeX math should be stripped: {}", typst);
    }

    #[test]
    fn test_image() {
        let typst = markdown_to_typst_ast("![alt](image.png)", &[]);
        assert!(typst.contains("image(\"image.png\")"), "typst: {}", typst);
    }
}
