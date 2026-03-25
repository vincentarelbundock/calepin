use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

use crate::render::elements::ElementRenderer;
use crate::render::template;
use crate::formats::OutputRenderer;
use crate::metadata::Metadata;

pub struct RevealJsRenderer;

impl OutputRenderer for RevealJsRenderer {
    fn format(&self) -> &str { "revealjs" }
    fn engine(&self) -> &str { "html" }
    fn extension(&self) -> &str { "html" }

    fn transform_body(&self, body: &str, renderer: &ElementRenderer) -> String {
        let footnotes = renderer.render_footnote_section();
        let full_body = if footnotes.is_empty() {
            body.to_string()
        } else {
            format!("{}{}", body, footnotes)
        };
        split_into_slides(&full_body)
    }

    fn assemble_page(
        &self,
        body: &str,
        meta: &Metadata,
        renderer: &ElementRenderer,
    ) -> Option<String> {
        let walk_meta = renderer.walk_metadata();
        let html = template::assemble_page(
            body, meta, "revealjs", &walk_meta.headings, renderer.preamble(),
            |vars| {
                // Inject syntax highlighting CSS
                let syntax_css = renderer.syntax_css_with_scope(
                    crate::render::highlighting::ColorScope::Both,
                );
                if !syntax_css.is_empty() {
                    vars.insert("syntax_css".to_string(), syntax_css);
                }
                // Render math include using html engine (since ext="revealjs"
                // would skip the html-only branch in build_template_vars)
                let defs = &renderer.metadata;
                let mut math_vars = HashMap::new();
                math_vars.insert("html_math_method".to_string(),
                    meta.html_math_method.as_deref()
                        .unwrap_or_else(|| defs.math.as_deref().unwrap_or("katex")).to_string());
                vars.insert("math".to_string(),
                    template::render_element("math", "html", &math_vars));
                // Set base to html so {% include "math.jinja" %} resolves
                vars.insert("base".to_string(), "html".to_string());
            },
        );
        Some(html)
    }
}

// ---------------------------------------------------------------------------
// Slide splitting
// ---------------------------------------------------------------------------

static HEADING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^<(h[1-6])\b").unwrap());

/// Split rendered HTML body into `<section>` slides.
///
/// Detects the first heading level in the body and splits on every
/// occurrence of that level. Each chunk is wrapped in `<section>`.
fn split_into_slides(body: &str) -> String {
    // Find the heading tag used for slide boundaries
    let split_tag = match HEADING_RE.captures(body) {
        Some(caps) => caps[1].to_string(),
        None => return format!("<section>\n{}\n</section>", body),
    };

    let boundary = Regex::new(&format!(r"(?m)^<{}\b", split_tag)).unwrap();

    // Collect split points
    let starts: Vec<usize> = boundary.find_iter(body).map(|m| m.start()).collect();
    if starts.is_empty() {
        return format!("<section>\n{}\n</section>", body);
    }

    let mut sections = Vec::new();

    // Content before first heading (if any)
    let before = body[..starts[0]].trim();
    if !before.is_empty() {
        sections.push(format!("<section>\n{}\n</section>", before));
    }

    // Each heading starts a new slide, ending at the next heading
    for (i, &start) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(body.len());
        let chunk = body[start..end].trim();
        if !chunk.is_empty() {
            sections.push(format!("<section>\n{}\n</section>", chunk));
        }
    }

    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_by_h2() {
        let body = "<h2>Slide 1</h2>\n<p>Content 1</p>\n<h2>Slide 2</h2>\n<p>Content 2</p>";
        let result = split_into_slides(body);
        assert!(result.contains("<section>\n<h2>Slide 1</h2>"));
        assert!(result.contains("<section>\n<h2>Slide 2</h2>"));
        assert_eq!(result.matches("<section>").count(), 2);
    }

    #[test]
    fn test_split_by_h3() {
        let body = "<h3>A</h3>\n<p>one</p>\n<h3>B</h3>\n<p>two</p>";
        let result = split_into_slides(body);
        assert_eq!(result.matches("<section>").count(), 2);
    }

    #[test]
    fn test_no_headings() {
        let body = "<p>Just a paragraph</p>";
        let result = split_into_slides(body);
        assert_eq!(result.matches("<section>").count(), 1);
    }
}
