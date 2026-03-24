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
                // Single paragraph with no table -- it's just a caption
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
    resolve_template: &dyn Fn(&str) -> Option<String>,
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

    let mut vars = HashMap::new();
    vars.insert("base".to_string(), format.to_string());
    vars.insert("engine".to_string(), format.to_string());
    vars.insert("children".to_string(), content_rendered);
    vars.insert("id".to_string(), id.to_string());
    vars.insert("cap_location".to_string(), cap_location.to_string());

    // Pre-built format-specific strings (avoids triple-brace issues in Jinja)
    let tbl_pos = attrs.get("tbl-pos").map(|s| format!("[{}]", s)).unwrap_or_default();
    vars.insert("tbl_begin".to_string(), format!("\\begin{{table}}{}", tbl_pos));
    vars.insert("caption_cmd".to_string(), if cap_rendered.is_empty() {
        String::new()
    } else {
        format!("\\caption{{{}}}", &cap_rendered)
    });
    vars.insert("caption".to_string(), cap_rendered);
    vars.insert("label".to_string(), match format {
        "latex" => format!("\\label{{{}}}", id),
        "typst" => format!("<{}>", id),
        _ => String::new(),
    });

    let tpl = resolve_template("table_div")
        .unwrap_or_else(|| include_str!("../project/templates/common/table_div.jinja").to_string());
    crate::render::template::apply_template(&tpl, &vars)
}
