use comrak::Options;
use regex::Regex;
use std::sync::LazyLock;

use crate::render::markers;

/// Common comrak options for all output formats.
pub fn build_comrak_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.header_ids = Some("".to_string());
    options.extension.superscript = true;
    options.extension.subscript = true;
    options.extension.underline = true;
    options.extension.highlight = true;
    options.extension.description_lists = true;
    options.render.r#unsafe = true;
    options.render.hardbreaks = false;
    options.parse.smart = true;
    options
}

// Re-export marker functions used by element_renderer and other modules
pub use markers::wrap_raw;

/// Render markdown to HTML via AST walk.
pub fn render_html(markdown: &str, raw_fragments: &[String], embed_resources: bool) -> String {
    crate::render::emit::html::markdown_to_html_ast(markdown, raw_fragments, false, false, embed_resources)
}

/// Render markdown to HTML and return collected metadata (headings, IDs).
pub fn render_html_with_metadata(
    markdown: &str,
    raw_fragments: &[String],
    options: &crate::render::emit::WalkOptions,
    embed_resources: bool,
) -> crate::render::emit::WalkResult {
    crate::render::emit::html::markdown_to_html_ast_with_metadata(markdown, raw_fragments, options, embed_resources)
}

/// Render markdown to Typst via AST walk.
pub fn render_typst(markdown: &str, raw_fragments: &[String]) -> String {
    crate::render::emit::typst::markdown_to_typst_ast(markdown, raw_fragments, false)
}

/// Render a short inline markdown string (e.g., title) to the target format.
/// Strips the wrapping <p> tags that comrak adds.
pub fn render_inline(text: &str, format: &str) -> String {
    let rendered = match format {
        "html" => render_html(text, &[], false),
        "latex" => crate::render::emit::latex::markdown_to_latex(text, &[], false),
        "typst" => render_typst(text, &[]),
        _ => crate::render::emit::markdown::markdown_to_markdown(text, &[]),
    };
    // Strip wrapping paragraph tags
    let trimmed = rendered.trim();
    let trimmed = trimmed.strip_prefix("<p>").unwrap_or(trimmed);
    let trimmed = trimmed.strip_suffix("</p>").unwrap_or(trimmed);
    trimmed.trim().to_string()
}

// ---------------------------------------------------------------------------
// Quarto-style image attributes: ![alt](path){width=30% height=200}
// ---------------------------------------------------------------------------

/// Parsed image attributes from `{key=value ...}` blocks.
pub struct ImageAttrs {
    pub width: Option<String>,
    pub height: Option<String>,
    pub fig_align: Option<String>,
    pub extra: Vec<(String, String)>,
}

/// Matches a single `key=value` or `key="value"` pair inside `{...}`.
static ATTR_PAIR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"([\w.-]+)\s*=\s*"?([^"\s}]+)"?"#).unwrap()
});

/// Ensure a CSS dimension value has a unit. Bare numbers get "px" appended.
fn css_dimension(val: &str) -> String {
    if val.parse::<f64>().is_ok() {
        format!("{}px", val)
    } else {
        val.to_string()
    }
}

impl ImageAttrs {
    /// Empty attributes (no width, height, etc.).
    pub fn empty() -> Self {
        Self { width: None, height: None, fig_align: None, extra: Vec::new() }
    }

    /// Parse a `key=value ...` attribute string (the content inside `{...}`).
    pub fn parse(attrs_str: &str) -> Self {
        let mut attrs = Self::empty();
        for cap in ATTR_PAIR_RE.captures_iter(attrs_str) {
            let key = &cap[1];
            let value = cap[2].to_string();
            match key {
                "width" => attrs.width = Some(value),
                "height" => attrs.height = Some(value),
                "fig-align" | "fig_align" => attrs.fig_align = Some(value),
                _ => attrs.extra.push((key.to_string(), value)),
            }
        }
        attrs
    }

    /// Emit HTML style and extra attributes for an `<img>` tag.
    pub fn to_html(&self) -> (String, Vec<String>) {
        let mut style_parts: Vec<String> = Vec::new();
        let mut html_attrs: Vec<String> = Vec::new();

        if let Some(ref w) = self.width {
            style_parts.push(format!("width:{}", css_dimension(w)));
        }
        if let Some(ref h) = self.height {
            style_parts.push(format!("height:{}", css_dimension(h)));
        }
        if let Some(ref align) = self.fig_align {
            match align.as_str() {
                "left" => style_parts.push("display:block;margin-right:auto".to_string()),
                "right" => style_parts.push("display:block;margin-left:auto".to_string()),
                _ => style_parts.push("display:block;margin:auto".to_string()),
            }
        }
        for (k, v) in &self.extra {
            html_attrs.push(format!("{}=\"{}\"", k, v));
        }

        let style = if style_parts.is_empty() {
            String::new()
        } else {
            format!(" style=\"{}\"", style_parts.join(";"))
        };
        (style, html_attrs)
    }

    /// Emit LaTeX `[options]` string for `\includegraphics`.
    pub fn to_latex_options(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(ref w) = self.width {
            parts.push(format!("width={}", w));
        }
        if let Some(ref h) = self.height {
            parts.push(format!("height={}", h));
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("[{}]", parts.join(","))
        }
    }

    /// Emit Typst named parameters for `image()`.
    pub fn to_typst_params(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(ref w) = self.width {
            parts.push(format!("width: {}", w));
        }
        if let Some(ref h) = self.height {
            parts.push(format!("height: {}", h));
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!(", {}", parts.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_math_preserved_in_html() {
        let input = "The formula $a^2 + b^2 = c^2$ is important.";
        let result = render_html(input, &[], false);
        assert!(result.contains("$a^2 + b^2 = c^2$"), "result: {}", result);
        assert!(!result.contains("<sup>"), "result: {}", result);
    }

    #[test]
    fn test_display_math_preserved() {
        let input = "Below:\n\n$$x = \\frac{-b}{2a}$$\n\nAbove.";
        let result = render_html(input, &[], false);
        assert!(result.contains("$$x = \\frac{-b}{2a}$$"), "result: {}", result);
    }

    #[test]
    fn test_escaped_dollar_is_literal() {
        let input = r"Costs \$5 and \$10 per unit.";
        let result = render_html(input, &[], false);
        assert!(result.contains("nodollar\">$</span>5"), "should contain literal $5: {}", result);
        assert!(result.contains("nodollar\">$</span>10"), "should contain literal $10: {}", result);
    }

    #[test]
    fn test_unescaped_dollar_is_math() {
        let input = "The value $x$ is important.";
        let result = render_html(input, &[], false);
        assert!(result.contains("$x$"), "result: {}", result);
    }

    #[test]
    fn test_multiline_display_math() {
        let input = "Before:\n\n$$\n\\begin{aligned}\na &= b + c \\\\\nd &= e + f\n\\end{aligned}\n$$\n\nAfter.";
        let result = render_html(input, &[], false);
        assert!(result.contains("\\begin{aligned}"), "result: {}", result);
        assert!(result.contains("$$"), "result: {}", result);
    }

    #[test]
    fn test_img_attrs_latex() {
        let attrs = ImageAttrs::parse("width=7em");
        assert_eq!(attrs.to_latex_options(), "[width=7em]");
    }

    #[test]
    fn test_img_attrs_empty() {
        let attrs = ImageAttrs::parse("");
        assert_eq!(attrs.to_latex_options(), "");
    }

    #[test]
    fn test_typst_heading_ids_roundtrip() {
        let md = "# Introduction {#sec-intro}\n\nSome text.\n";
        let result = render_typst(md, &[]);
        assert!(result.contains("<sec-intro>"), "should have explicit label: {}", result);
        assert!(!result.contains("{#sec-intro}"), "should strip attr syntax: {}", result);
    }

    // -----------------------------------------------------------------------
    // Comprehensive markdown syntax tests across all formats
    // -----------------------------------------------------------------------

    fn latex(md: &str) -> String {
        crate::render::emit::latex::markdown_to_latex(md, &[], false)
    }

    // -- Inline formatting --

    #[test] fn emphasis_html()  { assert!(render_html("*italic*", &[], false).contains("<em>italic</em>")); }
    #[test] fn emphasis_latex() { assert!(latex("*italic*").contains("\\emph{italic}")); }
    #[test] fn emphasis_typst() { assert!(render_typst("*italic*", &[]).contains("_italic_")); }

    #[test] fn strong_html()  { assert!(render_html("**bold**", &[], false).contains("<strong>bold</strong>")); }
    #[test] fn strong_latex() { assert!(latex("**bold**").contains("\\textbf{bold}")); }
    #[test] fn strong_typst() { assert!(render_typst("**bold**", &[]).contains("*bold*")); }

    #[test] fn strikethrough_html()  { assert!(render_html("~~del~~", &[], false).contains("<del>del</del>")); }
    #[test] fn strikethrough_latex() { assert!(latex("~~del~~").contains("\\sout{del}")); }
    #[test] fn strikethrough_typst() { assert!(render_typst("~~del~~", &[]).contains("#strike[del]")); }

    #[test] fn superscript_html()  { assert!(render_html("x^2^", &[], false).contains("<sup>2</sup>")); }
    #[test] fn superscript_latex() { assert!(latex("x^2^").contains("\\textsuperscript{2}")); }
    #[test] fn superscript_typst() { assert!(render_typst("x^2^", &[]).contains("#super[2]")); }

    #[test] fn subscript_html()  { assert!(render_html("H~2~O", &[], false).contains("<sub>2</sub>")); }
    #[test] fn subscript_latex() { assert!(latex("H~2~O").contains("\\textsubscript{2}")); }
    #[test] fn subscript_typst() { assert!(render_typst("H~2~O", &[]).contains("#sub[2]")); }

    #[test] fn highlight_html()  { assert!(render_html("==marked==", &[], false).contains("<mark>marked</mark>")); }
    #[test] fn highlight_latex() { assert!(latex("==marked==").contains("\\hl{marked}")); }
    #[test] fn highlight_typst() { assert!(render_typst("==marked==", &[]).contains("#highlight[marked]")); }

    #[test] fn inline_code_html()  { assert!(render_html("`code`", &[], false).contains("<code>code</code>")); }
    #[test] fn inline_code_latex() { assert!(latex("`code`").contains("\\texttt{code}")); }
    #[test] fn inline_code_typst() { assert!(render_typst("`code`", &[]).contains("`code`")); }

    // -- Links --

    #[test] fn link_html()  { assert!(render_html("[t](https://x.com)", &[], false).contains("<a href=\"https://x.com\">t</a>")); }
    #[test] fn link_latex() { assert!(latex("[t](https://x.com)").contains("\\href{https://x.com}")); }
    #[test] fn link_typst() { assert!(render_typst("[t](https://x.com)", &[]).contains("#link(\"https://x.com\")[t]")); }

    // -- Images --

    #[test] fn image_html()  { let r = render_html("![a](i.png)", &[], false); assert!(r.contains("<img") && r.contains("i.png")); }
    #[test] fn image_latex() { let r = latex("![a](i.png)"); assert!(r.contains("\\includegraphics") && r.contains("i.png")); }
    #[test] fn image_typst() { assert!(render_typst("![a](i.png)", &[]).contains("image(\"i.png\")")); }

    #[test] fn image_attrs_html()  { assert!(render_html("![a](i.png){width=50%}", &[], false).contains("width:50%")); }
    #[test] fn image_attrs_latex() { assert!(latex("![a](i.png){width=50%}").contains("[width=50%]")); }
    #[test] fn image_attrs_typst() { assert!(render_typst("![a](i.png){width=50%}", &[]).contains("width: 50%")); }

    // -- Headings --

    #[test] fn heading_html()  { let r = render_html("# Title", &[], false); assert!(r.contains("<h1") && r.contains("id=\"title\"")); }
    #[test] fn heading_latex() { assert!(latex("# Title").contains("\\section*")); }
    #[test] fn heading_typst() { let r = render_typst("# Title", &[]); assert!(r.contains("= Title") && r.contains("<title>")); }

    #[test] fn heading_id_html()  { let r = render_html("## M {#sec-m}", &[], false); assert!(r.contains("id=\"sec-m\"") && !r.contains("{#sec-m}")); }
    #[test] fn heading_id_latex() { assert!(latex("## M {#sec-m}").contains("\\label{sec-m}")); }
    #[test] fn heading_id_typst() { let r = render_typst("## M {#sec-m}", &[]); assert!(r.contains("<sec-m>") && !r.contains("{#sec-m}")); }

    // -- Code blocks --

    #[test] fn code_block_html()  { let r = render_html("```py\nx=1\n```", &[], false); assert!(r.contains("language-py") && r.contains("x=1")); }
    #[test] fn code_block_latex() { let r = latex("```\nx=1\n```"); assert!(r.contains("\\begin{verbatim}") && r.contains("x=1")); }
    #[test] fn code_block_typst() { let r = render_typst("```py\nx=1\n```", &[]); assert!(r.contains("```py") && r.contains("x=1")); }

    // -- Lists --

    #[test] fn ul_html()  { let r = render_html("- a\n- b", &[], false); assert!(r.contains("<ul>") && r.contains("<li>")); }
    #[test] fn ol_html()  { assert!(render_html("1. a\n2. b", &[], false).contains("<ol>")); }
    #[test] fn ul_latex() { assert!(latex("- a\n- b").contains("\\begin{itemize}")); }
    #[test] fn ol_latex() { assert!(latex("1. a\n2. b").contains("\\begin{enumerate}")); }
    #[test] fn task_html() { let r = render_html("- [ ] t\n- [x] d", &[], false); assert!(r.contains("checkbox") && r.contains("checked")); }

    // -- Tables --

    #[test] fn table_html()  { let r = render_html("| A |\n|:--|\n| 1 |", &[], false); assert!(r.contains("<table>") && r.contains("<th")); }
    #[test] fn table_latex() { let r = latex("| A |\n|:--|\n| 1 |"); assert!(r.contains("\\begin{tabular}")); }
    #[test] fn table_typst() { assert!(render_typst("| A |\n|:--|\n| 1 |", &[]).contains("#table(")); }

    // -- Blockquotes --

    #[test] fn bq_html()  { assert!(render_html("> q", &[], false).contains("<blockquote>")); }
    #[test] fn bq_latex() { assert!(latex("> q").contains("\\begin{quote}")); }
    #[test] fn bq_typst() { assert!(render_typst("> q", &[]).contains("#quote(block: true)")); }

    // -- Horizontal rules --

    #[test] fn hr_html()  { assert!(render_html("---", &[], false).contains("<hr")); }
    #[test] fn hr_latex() { assert!(latex("---").contains("\\rule")); }
    #[test] fn hr_typst() { assert!(render_typst("---", &[]).contains("#line(length: 100%)")); }

    // -- Footnotes --

    #[test] fn fn_html()  { let r = render_html("T[^1].\n\n[^1]: N.", &[], false); assert!(r.contains("footnote-ref") && r.contains("N.")); }
    #[test] fn fn_latex() { let r = latex("T[^1].\n\n[^1]: N."); assert!(r.contains("\\footnotemark[1]") && r.contains("\\footnotetext[1]")); }
    #[test] fn fn_typst() { assert!(render_typst("T[^1].\n\n[^1]: N.", &[]).contains("#footnote[N.]")); }

    // -- Special character escaping --

    #[test] fn html_escapes() { let r = render_html("a < b & c", &[], false); assert!(r.contains("&lt;") && r.contains("&amp;")); }
    #[test] fn latex_escapes() { let r = latex("$10 & 20%"); assert!(r.contains("\\$") && r.contains("\\&") && r.contains("\\%")); }

    // -- Cross-block footnotes (tested via ElementRenderer) --

    #[test]
    fn cross_block_footnotes() {
        // Simulate two separate Text elements: one with a reference, one with the definition.
        // The ElementRenderer should collect defs and inject them so comrak resolves both.
        use crate::render::elements::ElementRenderer;
        use crate::types::Element;
        use crate::render::highlighting::HighlightConfig;

        let elements = vec![
            Element::Text { content: "See note[^abc].".to_string() },
            Element::Text { content: "[^abc]: The definition.".to_string() },
        ];

        for fmt in ["html", "latex", "typst"] {
            let renderer = ElementRenderer::new(fmt, HighlightConfig::None);
            renderer.collect_footnote_defs(&elements);
            let parts: Vec<String> = elements.iter()
                .map(|el| renderer.render(el))
                .filter(|s| !s.is_empty())
                .collect();
            let body = parts.join("\n\n");

            // The reference should have been resolved (not left as literal [^abc])
            assert!(
                !body.contains("[^abc]") || body.contains("footnote"),
                "footnote should resolve in {}: {}",
                fmt, body,
            );
        }
    }
}
