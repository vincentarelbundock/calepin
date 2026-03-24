//! HTML format emitter for the unified AST walker.

use comrak::nodes::TableAlignment;

use crate::render::ast::{FormatEmitter, FootnoteStrategy, HeadingAttrs, WalkOptions, WalkResult, walk_and_render_with_metadata};
use crate::render::markdown::ImageAttrs;

pub struct HtmlEmitter;

/// Convert markdown to HTML via the shared AST walker.
pub fn markdown_to_html_ast(
    markdown: &str,
    raw_fragments: &[String],
    number_sections: bool,
    shift_headings: bool,
) -> String {
    let options = WalkOptions { number_sections, shift_headings, ..WalkOptions::default() };
    markdown_to_html_ast_with_metadata(markdown, raw_fragments, &options).output
}

/// Convert markdown to HTML and return collected metadata (headings, IDs).
pub fn markdown_to_html_ast_with_metadata(
    markdown: &str,
    raw_fragments: &[String],
    options: &WalkOptions,
) -> WalkResult {
    let emitter = HtmlEmitter;
    walk_and_render_with_metadata(&emitter, markdown, raw_fragments, options)
}

/// Render a combined footnote section from accumulated defs.
pub fn render_footnote_section(defs: &[(usize, String)]) -> String {
    HtmlEmitter.footnote_section(defs)
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
    fn subscript_open(&self) -> &str { "<sub>" }
    fn subscript_close(&self) -> &str { "</sub>" }
    fn underline_open(&self) -> &str { "<u>" }
    fn underline_close(&self) -> &str { "</u>" }
    fn highlight_open(&self) -> &str { "<mark>" }
    fn highlight_close(&self) -> &str { "</mark>" }

    fn link_open(&self, url: &str) -> String {
        format!("<a href=\"{}\">", self.escape_text(url))
    }
    fn link_close(&self) -> &str { "</a>" }

    fn image(&self, url: &str, alt: &str, attrs: &ImageAttrs) -> String {
        let resolved = crate::filters::figure::select_image_variant(
            std::path::Path::new(url), "html",
        );
        let embed = crate::project::get_defaults().embed_resources.unwrap_or(true);
        let src = if embed && !url.starts_with("http://") && !url.starts_with("https://") {
            crate::util::base64_encode_image(&resolved)
                .map(|(mime, data)| format!("data:{};base64,{}", mime, data))
                .unwrap_or_else(|_| self.escape_text(&resolved.display().to_string()))
        } else {
            self.escape_text(&resolved.display().to_string())
        };
        let (style, extra) = attrs.to_html();
        let mut out = format!("<img src=\"{}\" alt=\"{}\"", src, self.escape_text(alt));
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
        // Build individual footnote items in Rust (backref injection is complex)
        let mut footnote_items = String::new();
        for (id, content) in defs {
            let backref = format!(
                " <a href=\"#fnref-{}\" class=\"footnote-backref\" data-footnote-backref data-footnote-backref-idx=\"{}\" aria-label=\"Back to reference {}\">↩</a>",
                id, id, id
            );
            // Insert backref before the last </p> so it appears inline
            let body = if let Some(pos) = content.rfind("</p>") {
                format!("{}{}{}", &content[..pos], backref, &content[pos..])
            } else {
                format!("{}{}", content, backref)
            };
            footnote_items.push_str(&format!("<li id=\"fn-{}\">\n{}\n</li>\n", id, body));
        }

        // Use the footnotes template for the wrapper
        let mut vars = std::collections::HashMap::new();
        vars.insert("base".to_string(), "html".to_string());
        vars.insert("engine".to_string(), "html".to_string());
        vars.insert("footnotes".to_string(), "true".to_string());
        vars.insert("footnote_items".to_string(), footnote_items);
        let tpl = include_str!("../project/templates/common/footnotes.jinja");
        crate::render::template::apply_template(tpl, &vars)
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

    #[test]
    fn test_toc_strips_heading_attrs() {
        let md = "# Introduction {#sec-intro}\n\n## Methods {#sec-methods .custom}";
        let options = WalkOptions { number_sections: false, ..WalkOptions::default() };
        let result = markdown_to_html_ast_with_metadata(md, &[], &options);
        let headings = &result.metadata.headings;
        assert_eq!(headings.len(), 2);
        assert_eq!(headings[0].text, "Introduction");
        assert_eq!(headings[1].text, "Methods");
        assert_eq!(headings[0].id, "sec-intro");
        assert_eq!(headings[1].id, "sec-methods");
    }

    #[test]
    fn test_section_counters_chain_across_walks() {
        // First walk: two headings -> counters 1, 2
        let opts1 = WalkOptions { number_sections: true, ..WalkOptions::default() };
        let r1 = markdown_to_html_ast_with_metadata("# First\n\n# Second", &[], &opts1);
        assert!(r1.output.contains("section-number\">1</span> First"), "r1: {}", r1.output);
        assert!(r1.output.contains("section-number\">2</span> Second"), "r1: {}", r1.output);

        // Second walk: chain counters -> should continue as 3
        let opts2 = WalkOptions {
            number_sections: true,
            section_counters_start: Some(r1.metadata.section_counters_end),
            min_heading_level: Some(r1.metadata.min_heading_level),
            ..WalkOptions::default()
        };
        let r2 = markdown_to_html_ast_with_metadata("# Third", &[], &opts2);
        assert!(r2.output.contains("section-number\">3</span> Third"), "r2: {}", r2.output);
    }

    #[test]
    fn test_footnotes_suppressed_and_collected() {
        let md = "Text[^1].\n\n[^1]: Footnote content.";
        let opts = WalkOptions { suppress_footnote_section: true, ..WalkOptions::default() };
        let result = markdown_to_html_ast_with_metadata(md, &[], &opts);
        // Footnote section should NOT be in the output
        assert!(!result.output.contains("class=\"footnotes\""), "should suppress: {}", result.output);
        // But defs should be returned in metadata
        assert_eq!(result.metadata.footnote_defs.len(), 1);
        assert_eq!(result.metadata.footnote_defs[0].0, 1); // ID = 1
        assert!(result.metadata.footnote_defs[0].1.contains("Footnote content"));
    }

    #[test]
    fn test_footnote_counter_chains_without_gaps() {
        // First walk: footnotes 1, 2
        let md1 = "A[^a] B[^b].\n\n[^a]: Note A.\n[^b]: Note B.";
        let opts1 = WalkOptions { suppress_footnote_section: true, ..WalkOptions::default() };
        let r1 = markdown_to_html_ast_with_metadata(md1, &[], &opts1);
        assert_eq!(r1.metadata.footnote_counter_end, 2);
        assert!(r1.output.contains("fnref-1"), "r1: {}", r1.output);
        assert!(r1.output.contains("fnref-2"), "r1: {}", r1.output);

        // Second walk: footnote 3 (continues from 2), with global defs appended
        let md2 = "C[^c].\n\n[^c]: Note C.\n\n[^a]: Note A.\n[^b]: Note B.";
        let opts2 = WalkOptions {
            footnote_counter_start: r1.metadata.footnote_counter_end,
            suppress_footnote_section: true,
            ..WalkOptions::default()
        };
        let r2 = markdown_to_html_ast_with_metadata(md2, &[], &opts2);
        // Ref should be fnref-3, not fnref-1
        assert!(r2.output.contains("fnref-3"), "r2: {}", r2.output);
        assert!(!r2.output.contains("fnref-1"), "should not have fnref-1: {}", r2.output);
        // Counter should advance by 1 (only the ref, not the global defs)
        assert_eq!(r2.metadata.footnote_counter_end, 3);
        // Only 1 def collected (for [^c], not [^a] or [^b])
        assert_eq!(r2.metadata.footnote_defs.len(), 1);
    }

    #[test]
    fn test_footnote_backref_inline() {
        let md = "Text[^1].\n\n[^1]: Footnote content here.";
        let opts = WalkOptions::default();
        let result = markdown_to_html_ast_with_metadata(md, &[], &opts);
        // Backref should be inside the <p>, before </p>
        assert!(
            result.output.contains("Footnote content here. <a href=\"#fnref-1\""),
            "backref should be inline: {}", result.output
        );
    }

    #[test]
    fn test_render_footnote_section_helper() {
        let defs = vec![
            (1, "<p>First note.</p>".to_string()),
            (2, "<p>Second note.</p>".to_string()),
        ];
        let html = render_footnote_section(&defs);
        assert!(html.contains("id=\"fn-1\""), "html: {}", html);
        assert!(html.contains("id=\"fn-2\""), "html: {}", html);
        assert!(html.contains("fnref-1"), "html: {}", html);
        assert!(html.contains("fnref-2"), "html: {}", html);
        // Backref should be before </p>, not on a separate line
        assert!(html.contains("First note. <a href=\"#fnref-1\""), "backref inline: {}", html);
    }
}
