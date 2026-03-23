//! Figure div structure: renders `#fig-` prefixed divs as figure environments.

use std::collections::HashMap;

use crate::types::Element;
use crate::filters::figure::format_align;

/// Render a `#fig-` prefixed div as a figure environment.
pub fn render_div(
    id: &str,
    attrs: &HashMap<String, String>,
    children: &[Element],
    format: &str,
    render_element: &dyn Fn(&Element) -> String,
    _raw_fragments: &std::cell::RefCell<Vec<String>>,
) -> String {
    // Caption: fig-cap attribute takes priority, then last text element in div
    let (content_children, caption) = if let Some(cap) = attrs.get("fig-cap") {
        (children.to_vec(), cap.clone())
    } else {
        separate_figure_caption(children)
    };

    let content_rendered: String = content_children.iter()
        .map(render_element)
        .collect::<Vec<_>>()
        .join("\n\n");

    let cap_rendered = if !caption.is_empty() {
        crate::render::markdown::render_inline(&caption, format)
    } else {
        String::new()
    };

    let align = attrs.get("fig-align").map(|s| s.as_str()).unwrap_or("center");

    match format {
        "html" => {
            let align_style = format_align(align, format);
            format!(
                "<div class=\"figure\" id=\"{}\" style=\"{}\">\n{}\n<p class=\"caption\">{}</p>\n</div>",
                id, align_style, content_rendered, cap_rendered
            )
        }
        "latex" => {
            let env = attrs.get("fig-env").map(|s| s.as_str()).unwrap_or("figure");
            let pos = attrs.get("fig-pos").map(|s| format!("[{}]", s)).unwrap_or_default();
            let align_cmd = format_align(align, format);
            format!(
                "\\begin{{{}}}{}\n{}\n{}\n\\caption{{{}}}\n\\label{{{}}}\n\\end{{{}}}",
                env, pos, align_cmd, content_rendered, cap_rendered, id, env
            )
        }
        "typst" => {
            format!("#figure([\n{}\n], caption: [{}]) <{}>", content_rendered, cap_rendered, id)
        }
        _ => {
            if cap_rendered.is_empty() { content_rendered }
            else { format!("{}\n\n*{}*", content_rendered, cap_rendered) }
        }
    }
}

/// Separate the caption from children in a figure div.
pub fn separate_figure_caption(children: &[Element]) -> (Vec<Element>, String) {
    let mut content = children.to_vec();
    let mut caption = String::new();
    if let Some(last_idx) = content.iter().rposition(|e| matches!(e, Element::Text { .. })) {
        if let Element::Text { content: ref text } = content[last_idx] {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                caption = trimmed.to_string();
                content.remove(last_idx);
            }
        }
    }
    (content, caption)
}
