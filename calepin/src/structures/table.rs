//! Table div structure: renders `#tbl-` prefixed divs as captioned table environments.

use std::collections::HashMap;

use crate::types::Element;

/// Separate the caption from a table div's children.
/// The caption is the last paragraph that isn't part of a markdown table
/// (i.e., doesn't start with `|`).
fn separate_table_caption(children: &[Element]) -> (Vec<Element>, String) {
    let mut content = children.to_vec();
    let mut caption = String::new();

    // Find the last Text element
    if let Some(last_idx) = content.iter().rposition(|e| matches!(e, Element::Text { .. })) {
        if let Element::Text { content: ref text } = content[last_idx] {
            let trimmed = text.trim();
            // Split on double newline to find the last paragraph
            if let Some(split_pos) = trimmed.rfind("\n\n") {
                let last_para = trimmed[split_pos..].trim();
                // If the last paragraph doesn't look like a table, it's the caption
                if !last_para.starts_with('|') && !last_para.is_empty() {
                    caption = last_para.to_string();
                    let remaining = trimmed[..split_pos].trim().to_string();
                    content[last_idx] = Element::Text { content: remaining };
                }
            } else if !trimmed.starts_with('|') && !trimmed.is_empty() {
                // Single paragraph with no table — it's just a caption
                caption = trimmed.to_string();
                content.remove(last_idx);
            }
        }
    }

    (content, caption)
}

/// Render a `#tbl-` prefixed div as a captioned table environment.
pub fn render_div(
    id: &str,
    attrs: &HashMap<String, String>,
    children: &[Element],
    format: &str,
    render_element: &dyn Fn(&Element) -> String,
    _raw_fragments: &std::cell::RefCell<Vec<String>>,
) -> String {
    let (content_children, caption) = separate_table_caption(children);

    let content_rendered: String = content_children
        .iter()
        .map(render_element)
        .collect::<Vec<_>>()
        .join("\n\n");

    let cap_rendered = if !caption.is_empty() {
        crate::render::markdown::render_inline(&caption, format)
    } else {
        String::new()
    };

    let cap_location = attrs
        .get("tbl-cap-location")
        .map(|s| s.as_str())
        .unwrap_or("top");

    match format {
        "html" => {
            let cap_html = if cap_rendered.is_empty() {
                String::new()
            } else {
                format!("<p class=\"table-caption\">{}</p>\n", cap_rendered)
            };
            if cap_location == "bottom" {
                format!(
                    "<div class=\"table-div\" id=\"{}\">\n{}\n{}</div>",
                    id, content_rendered, cap_html
                )
            } else {
                format!(
                    "<div class=\"table-div\" id=\"{}\">\n{}{}\n</div>",
                    id, cap_html, content_rendered
                )
            }
        }
        "latex" => {
            let pos = attrs
                .get("tbl-pos")
                .map(|s| format!("[{}]", s))
                .unwrap_or_default();
            let cap = if cap_rendered.is_empty() {
                String::new()
            } else {
                format!("\\caption{{{}}}\n", cap_rendered)
            };
            let label = format!("\\label{{{}}}\n", id);
            if cap_location == "bottom" {
                format!(
                    "\\begin{{table}}{}\n\\centering\n{}\n{}{}\n\\end{{table}}",
                    pos, content_rendered, cap, label
                )
            } else {
                format!(
                    "\\begin{{table}}{}\n\\centering\n{}{}{}\n\\end{{table}}",
                    pos, cap, label, content_rendered
                )
            }
        }
        "typst" => {
            if cap_rendered.is_empty() {
                format!("{} <{}>", content_rendered, id)
            } else {
                format!(
                    "#figure(kind: table, [\n{}\n], caption: [{}]) <{}>",
                    content_rendered, cap_rendered, id
                )
            }
        }
        _ => {
            if cap_rendered.is_empty() {
                content_rendered
            } else if cap_location == "bottom" {
                format!("{}\n\n: {}", content_rendered, cap_rendered)
            } else {
                format!(": {}\n\n{}", cap_rendered, content_rendered)
            }
        }
    }
}
