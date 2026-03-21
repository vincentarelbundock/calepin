//! HTML format emitter for the unified AST walker.

use comrak::nodes::TableAlignment;

use crate::render::ast::{FormatEmitter, FootnoteStrategy, HeadingAttrs, WalkOptions, walk_and_render};
use crate::render::markdown::ImageAttrs;

pub struct HtmlEmitter;

/// Convert markdown to HTML via the shared AST walker.
pub fn markdown_to_html_ast(
    markdown: &str,
    raw_fragments: &[String],
    number_sections: bool,
    shift_headings: bool,
) -> String {
    let emitter = HtmlEmitter;
    let options = WalkOptions { number_sections, shift_headings };
    walk_and_render(&emitter, markdown, raw_fragments, &options)
}

impl FormatEmitter for HtmlEmitter {
    fn format_name(&self) -> &str { "html" }

    fn escape_text(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for c in text.chars() {
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

    fn blockquote_open(&self) -> &str { "<blockquote>\n" }
    fn blockquote_close(&self) -> &str { "</blockquote>\n" }

    fn list_open(&self, ordered: bool, start: usize, _tight: bool) -> String {
        if ordered {
            if start == 1 { "<ol>\n".to_string() }
            else { format!("<ol start=\"{}\">\n", start) }
        } else {
            "<ul>\n".to_string()
        }
    }

    fn list_close(&self, ordered: bool) -> String {
        if ordered { "</ol>\n".to_string() } else { "</ul>\n".to_string() }
    }

    fn item_open(&self, tight: bool) -> String {
        if tight { "<li>".to_string() } else { "<li>\n".to_string() }
    }
    fn item_close(&self) -> &str { "</li>\n" }

    fn paragraph_open(&self, in_tight_list_item: bool) -> &str {
        if in_tight_list_item { "" } else { "<p>" }
    }
    fn paragraph_close(&self, in_tight_list_item: bool) -> &str {
        if in_tight_list_item { "\n" } else { "</p>\n" }
    }

    fn heading_prefix(&self, _level: u8) -> String { String::new() }

    fn heading(
        &self,
        level: u8,
        attrs: &HeadingAttrs,
        rendered_content: &str,
        section_number: Option<&str>,
    ) -> String {
        let class_attr = if attrs.classes.is_empty() {
            String::new()
        } else {
            format!(" class=\"{}\"", attrs.classes.join(" "))
        };
        let mut out = format!("<h{}{} id=\"{}\">", level, class_attr, attrs.id);
        if let Some(num) = section_number {
            out.push_str(&format!("<span class=\"section-number\">{}</span> ", num));
        }
        out.push_str(rendered_content);
        out.push_str(&format!("</h{}>\n", level));
        out
    }

    fn code_inline(&self, literal: &str) -> String {
        format!("<code>{}</code>", self.escape_text(literal))
    }

    fn code_block(&self, info: &str, literal: &str) -> String {
        if info.is_empty() {
            format!("<pre><code>{}</code></pre>\n", self.escape_text(literal))
        } else {
            let lang = info.split_whitespace().next().unwrap_or("");
            format!(
                "<pre><code class=\"language-{}\">{}</code></pre>\n",
                self.escape_text(lang),
                self.escape_text(literal)
            )
        }
    }

    fn strong_open(&self) -> &str { "<strong>" }
    fn strong_close(&self) -> &str { "</strong>" }
    fn emph_open(&self) -> &str { "<em>" }
    fn emph_close(&self) -> &str { "</em>" }
    fn strikethrough_open(&self) -> &str { "<del>" }
    fn strikethrough_close(&self) -> &str { "</del>" }
    fn superscript_open(&self) -> &str { "<sup>" }
    fn superscript_close(&self) -> &str { "</sup>" }

    fn link_open(&self, url: &str) -> String {
        format!("<a href=\"{}\">", self.escape_text(url))
    }
    fn link_close(&self) -> &str { "</a>" }

    fn image(&self, url: &str, alt: &str, attrs: &ImageAttrs) -> String {
        let resolved = crate::filters::figure::resolve_path(
            std::path::Path::new(url), "html",
        );
        let (style, extra) = attrs.to_html();
        let mut out = format!("<img src=\"{}\" alt=\"{}\"", self.escape_text(&resolved.display().to_string()), self.escape_text(alt));
        out.push_str(&style);
        for a in &extra {
            out.push_str(&format!(" {}", a));
        }
        out.push_str(" />");
        out
    }

    fn table_open(&self, _alignments: &[TableAlignment]) -> String {
        "<table>\n".to_string()
    }
    fn table_close(&self) -> &str { "</table>\n" }

    fn table_row_open(&self, is_header: bool) -> String {
        if is_header { "<thead>\n<tr>\n".to_string() } else { "<tr>\n".to_string() }
    }
    fn table_row_close(&self, is_header: bool) -> String {
        if is_header { "</tr>\n</thead>\n<tbody>\n".to_string() } else { "</tr>\n".to_string() }
    }

    fn table_cell_open(&self, is_header: bool, align: TableAlignment, _index: usize) -> String {
        let tag = if is_header { "th" } else { "td" };
        let align_attr = match align {
            TableAlignment::Left => " style=\"text-align: left\"",
            TableAlignment::Center => " style=\"text-align: center\"",
            TableAlignment::Right => " style=\"text-align: right\"",
            _ => "",
        };
        format!("<{}{}>", tag, align_attr)
    }
    fn table_cell_close(&self, is_header: bool) -> String {
        let tag = if is_header { "th" } else { "td" };
        format!("</{}>\n", tag)
    }

    fn thematic_break(&self) -> &str { "<hr />\n" }
    fn soft_break(&self) -> &str { "\n" }
    fn line_break(&self) -> &str { "<br />\n" }

    fn footnote_strategy(&self) -> FootnoteStrategy { FootnoteStrategy::CollectToSection }

    fn footnote_ref(&self, id: usize) -> String {
        format!(
            "<sup class=\"footnote-ref\" id=\"fnref-{}\"><a href=\"#fn-{}\" data-footnote-ref>{}</a></sup>",
            id, id, id
        )
    }

    fn footnote_section(&self, defs: &[(usize, String)]) -> String {
        let mut out = String::from("\n<section class=\"footnotes\" data-footnotes>\n<ol>\n");
        for (id, content) in defs {
            out.push_str(&format!(
                "<li id=\"fn-{}\">\n{}<a href=\"#fnref-{}\" class=\"footnote-backref\" data-footnote-backref data-footnote-backref-idx=\"{}\" aria-label=\"Back to reference {}\">↩</a>\n</li>\n",
                id, content, id, id, id
            ));
        }
        out.push_str("</ol>\n</section>\n");
        out
    }

    fn html_block(&self, literal: &str) -> String { literal.to_string() }
    fn html_inline(&self, literal: &str) -> String { literal.to_string() }

    fn task_item(&self, checked: bool) -> String {
        if checked {
            "<input type=\"checkbox\" checked=\"\" disabled=\"\" /> ".to_string()
        } else {
            "<input type=\"checkbox\" disabled=\"\" /> ".to_string()
        }
    }

    fn description_list_open(&self) -> &str { "<dl>\n" }
    fn description_list_close(&self) -> &str { "</dl>\n" }
    fn description_term_open(&self) -> &str { "<dt>" }
    fn description_term_close(&self) -> &str { "</dt>\n" }
    fn description_details_open(&self) -> &str { "<dd>" }
    fn description_details_close(&self) -> &str { "</dd>\n" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_with_id() {
        let html = markdown_to_html_ast("# Hello World", &[], false, false);
        assert!(html.contains("id=\"hello-world\""), "html: {}", html);
        assert!(html.contains("<h1"), "html: {}", html);
    }

    #[test]
    fn test_heading_explicit_id() {
        let html = markdown_to_html_ast("## Methods {#sec-methods}", &[], false, false);
        assert!(html.contains("id=\"sec-methods\""), "html: {}", html);
        assert!(!html.contains("{#sec-methods}"), "should strip attr: {}", html);
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
    }

    #[test]
    fn test_strikethrough() {
        let html = markdown_to_html_ast("~~deleted~~", &[], false, false);
        assert!(html.contains("<del>deleted</del>"), "html: {}", html);
    }

    #[test]
    fn test_image() {
        let html = markdown_to_html_ast("![alt text](image.png)", &[], false, false);
        assert!(html.contains("<img"), "html: {}", html);
        assert!(html.contains("image.png"), "html: {}", html);
    }

    #[test]
    fn test_hr() {
        let html = markdown_to_html_ast("---", &[], false, false);
        assert!(html.contains("<hr"), "html: {}", html);
    }
}
