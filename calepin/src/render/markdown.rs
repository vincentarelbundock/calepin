use comrak::{markdown_to_html, markdown_to_typst, Options};
use regex::Regex;
use std::sync::LazyLock;

use crate::render::markers;

/// Shift all markdown heading levels down by one (`#` → `##`, `##` → `###`, etc.).
/// Only modifies ATX headings at the start of a line. Headings at level 6 stay at level 6.
pub fn shift_headings(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len() + 64);
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            // Count leading #s (on the trimmed line)
            let level = trimmed.bytes().take_while(|&b| b == b'#').count();
            // Must be followed by a space or end of line to be a heading
            let after = trimmed.as_bytes().get(level);
            if level >= 1 && level <= 6 && (after == Some(&b' ') || after.is_none()) {
                let indent = &line[..line.len() - trimmed.len()];
                let new_level = (level + 1).min(6);
                let hashes = "#".repeat(new_level);
                out.push_str(indent);
                out.push_str(&hashes);
                out.push_str(&trimmed[level..]);
                out.push('\n');
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    // Remove trailing newline if original didn't have one
    if !markdown.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    out
}


/// Common comrak options for all output formats.
pub fn comrak_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.header_ids = Some("".to_string());
    options.extension.superscript = true;
    options.extension.description_lists = true;
    options.render.r#unsafe = true;
    options.render.hardbreaks = false;
    options.parse.smart = true;
    options
}

// Re-export marker functions used by element_renderer and other modules
pub use markers::{protect_math, restore_math, wrap_raw, resolve_raw, preprocess};

/// Render markdown to HTML using comrak (CommonMark + GFM extensions).
pub fn render_html(markdown: &str, raw_fragments: &[String]) -> String {
    let preprocessed = preprocess(markdown);
    let (protected, math) = protect_math(&preprocessed);
    let html = markdown_to_html(&protected, &comrak_options());
    let restored = restore_math(&html, &math);
    let restored = markers::resolve_equation_labels(&restored, "html");
    let restored = markers::resolve_escaped_dollars(&restored, "html");
    let restored = apply_image_attrs_html(&restored);
    resolve_raw(&restored, raw_fragments)
}

/// Render markdown to Typst using comrak.
pub fn render_typst(markdown: &str, raw_fragments: &[String]) -> String {
    let preprocessed = preprocess(markdown);
    let (protected, math) = protect_math(&preprocessed);
    let typst = markdown_to_typst(&protected, &comrak_options());
    let typst = apply_image_attrs_typst(&typst);
    let restored = restore_math(&typst, &math);
    let restored = markers::resolve_equation_labels(&restored, "typst");
    let restored = markers::resolve_escaped_dollars(&restored, "typst");
    resolve_raw(&restored, raw_fragments)
}

/// Render a short inline markdown string (e.g., title) to the target format.
/// Strips the wrapping <p> tags that comrak adds.
pub fn render_inline(text: &str, format: &str) -> String {
    let rendered = match format {
        "html" => {
            let html = markdown_to_html(text, &comrak_options());
            apply_image_attrs_html(&html)
        }
        "latex" => {
            crate::render::latex::markdown_to_latex(text, &[], false)
        }
        "typst" => {
            let typst = markdown_to_typst(text, &comrak_options());
            apply_image_attrs_typst(&typst)
        }
        _ => text.to_string(),
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

impl ImageAttrs {
    /// Parse a `key=value ...` attribute string (the content inside `{...}`).
    pub fn parse(attrs_str: &str) -> Self {
        let mut attrs = ImageAttrs {
            width: None,
            height: None,
            fig_align: None,
            extra: Vec::new(),
        };
        for cap in ATTR_PAIR_RE.captures_iter(attrs_str) {
            let key = &cap[1];
            let value = cap[2].to_string();
            match key {
                "width" => attrs.width = Some(value),
                "height" => attrs.height = Some(value),
                "fig-align" => attrs.fig_align = Some(value),
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
            style_parts.push(format!("width:{}", w));
        }
        if let Some(ref h) = self.height {
            style_parts.push(format!("height:{}", h));
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

// ---------------------------------------------------------------------------
// Format-specific post-processing
// ---------------------------------------------------------------------------

/// Matches `<img .../>` or `<img ...>` followed by `{key=value ...}`.
static HTML_IMG_ATTR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(<img\s[^>]*?)/?\s*>\s*\{([^}]+)\}"#).unwrap()
});

/// Post-process HTML to absorb `{key=value}` attribute blocks into preceding `<img>` tags.
pub fn apply_image_attrs_html(html: &str) -> String {
    HTML_IMG_ATTR_RE.replace_all(html, |caps: &regex::Captures| {
        let img_open = &caps[1];
        let attrs = ImageAttrs::parse(&caps[2]);
        let (style, extra) = attrs.to_html();

        let mut result = img_open.to_string();
        result.push_str(&style);
        for a in &extra {
            result.push_str(&format!(" {}", a));
        }
        result.push_str(" />");
        result
    }).to_string()
}

/// Matches Typst `#box(image("url")){key=value ...}`.
static TYPST_IMG_ATTR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"#box\(image\("([^"]+)"\)\)\{([^}]+)\}"#).unwrap()
});

/// Post-process Typst to absorb `{key=value}` attribute blocks into preceding `#box(image(...))`.
pub fn apply_image_attrs_typst(typst: &str) -> String {
    TYPST_IMG_ATTR_RE.replace_all(typst, |caps: &regex::Captures| {
        let url = &caps[1];
        let attrs = ImageAttrs::parse(&caps[2]);
        let params = attrs.to_typst_params();
        format!("#box(image(\"{}\"{}))", url, params)
    }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_math_preserved_in_html() {
        let input = "The formula $a^2 + b^2 = c^2$ is important.";
        let result = render_html(input, &[]);
        assert!(result.contains("$a^2 + b^2 = c^2$"), "result: {}", result);
        assert!(!result.contains("<sup>"), "result: {}", result);
    }

    #[test]
    fn test_display_math_preserved() {
        let input = "Below:\n\n$$x = \\frac{-b}{2a}$$\n\nAbove.";
        let result = render_html(input, &[]);
        assert!(result.contains("$$x = \\frac{-b}{2a}$$"), "result: {}", result);
    }

    #[test]
    fn test_escaped_dollar_is_literal() {
        let input = r"Costs \$5 and \$10 per unit.";
        let result = render_html(input, &[]);
        assert!(result.contains("nodollar\">$</span>5"), "should contain literal $5: {}", result);
        assert!(result.contains("nodollar\">$</span>10"), "should contain literal $10: {}", result);
        assert!(!result.contains('\u{FFFF}'), "should not have placeholders: {}", result);
    }

    #[test]
    fn test_unescaped_dollar_is_math() {
        let input = "The value $x$ is important.";
        let result = render_html(input, &[]);
        assert!(result.contains("$x$"), "result: {}", result);
    }

    #[test]
    fn test_unmatched_dollar_passthrough() {
        let input = "This has a lone $ sign.";
        let result = render_html(input, &[]);
        assert!(result.contains("$"), "result: {}", result);
    }

    #[test]
    fn test_multiline_display_math() {
        let input = "Before:\n\n$$\n\\begin{aligned}\na &= b + c \\\\\nd &= e + f\n\\end{aligned}\n$$\n\nAfter.";
        let result = render_html(input, &[]);
        assert!(result.contains("\\begin{aligned}"), "result: {}", result);
        assert!(result.contains("\\end{aligned}"), "result: {}", result);
        assert!(result.contains("$$"), "result: {}", result);
    }

    #[test]
    fn test_escaped_dollar_inside_math() {
        let input = r"The price is $\$5$ in math mode.";
        let result = render_html(input, &[]);
        assert!(result.contains(r"$\$5$"), "result: {}", result);
    }

    #[test]
    fn test_escaped_dollar_after_number() {
        let input = r"Costs 24\$ or enclose math: $a^2 + b^2 = c^2$.";
        let result = render_html(input, &[]);
        assert!(result.contains("24<span class=\"nodollar\">$</span>"), "24$ should be wrapped: {}", result);
        assert!(result.contains("$a^2 + b^2 = c^2$"), "math should be preserved: {}", result);
    }

    #[test]
    fn test_img_attrs_html() {
        let attrs = ImageAttrs::parse("width=7em height=3em");
        let (style, extra) = attrs.to_html();
        assert!(style.contains("width:7em"), "style: {}", style);
        assert!(style.contains("height:3em"), "style: {}", style);
        assert!(extra.is_empty());
    }

    #[test]
    fn test_img_attrs_latex() {
        let attrs = ImageAttrs::parse("width=7em");
        assert_eq!(attrs.to_latex_options(), "[width=7em]");
    }

    #[test]
    fn test_img_attrs_typst() {
        let attrs = ImageAttrs::parse("width=7em height=3em");
        assert_eq!(attrs.to_typst_params(), ", width: 7em, height: 3em");
    }

    #[test]
    fn test_img_attrs_empty() {
        let attrs = ImageAttrs::parse("");
        assert_eq!(attrs.to_latex_options(), "");
        assert_eq!(attrs.to_typst_params(), "");
    }

    #[test]
    fn test_shift_headings() {
        let input = "# Title\n\nSome text\n\n## Sub\n\n### SubSub";
        let result = shift_headings(input);
        assert!(result.starts_with("## Title"), "result: {}", result);
        assert!(result.contains("### Sub\n"), "result: {}", result);
        assert!(result.contains("#### SubSub"), "result: {}", result);
    }

    #[test]
    fn test_shift_headings_no_false_positive() {
        // Lines starting with # but not headings (no space after)
        let input = "#hashtag\n\n# Real heading";
        let result = shift_headings(input);
        assert!(result.contains("#hashtag"), "should not shift non-headings: {}", result);
        assert!(result.contains("## Real heading"), "result: {}", result);
    }

    #[test]
    fn test_shift_headings_h6_stays() {
        let input = "###### Deep";
        let result = shift_headings(input);
        assert_eq!(result, "###### Deep", "h6 should stay at h6");
    }
}
