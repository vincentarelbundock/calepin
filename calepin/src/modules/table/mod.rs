//! Table module: handles `#tbl-` prefixed divs.
//!
//! Consolidates caption extraction, variable enrichment, and template
//! rendering for table divs. Registered as a TransformElementChildren
//! module matching `id_prefix = "tbl-"`.

use std::collections::HashMap;

use crate::types::Element;

/// Render a table div: extract caption from children, build template
/// vars, apply the `table_div` template.
pub fn render(
    id: &Option<String>,
    attrs: &HashMap<String, String>,
    children: &[Element],
    format: &str,
    render_element: &dyn Fn(&Element) -> String,
    module_ids: &std::cell::RefCell<HashMap<String, String>>,
) -> String {
    let id_val = match id.as_deref() {
        Some(id) => id,
        None => return render_children(children, render_element),
    };

    // Register ID for cross-referencing
    {
        let ids = module_ids.borrow();
        let count = ids.keys().filter(|k| k.starts_with("tbl-")).count();
        drop(ids);
        module_ids.borrow_mut().insert(id_val.to_string(), (count + 1).to_string());
    }

    // Extract caption from last non-table paragraph (unless tbl_cap is already set)
    let (content, caption_text) = if attrs.contains_key("tbl_cap") {
        (children.to_vec(), attrs.get("tbl_cap").cloned().unwrap_or_default())
    } else {
        separate_table_caption(children)
    };

    // Render children
    let children_rendered: String = content.iter()
        .map(render_element)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    // Build template vars
    let mut vars = HashMap::new();
    vars.insert("base".to_string(), format.to_string());
    vars.insert("engine".to_string(), format.to_string());
    vars.insert("children".to_string(), children_rendered);
    vars.insert("label".to_string(), id_val.to_string());
    vars.insert("id".to_string(), id_val.to_string());

    // Copy div attrs into vars
    for (k, val) in attrs {
        vars.insert(k.clone(), val.clone());
    }

    // Render caption markdown to target format
    if !caption_text.is_empty() {
        let rendered_caption = crate::render::convert::render_inline(&caption_text, format);
        vars.insert("caption".to_string(), rendered_caption);
    }

    // Caption location (default: top for tables)
    let cap_loc = attrs.get("tbl_cap_location")
        .cloned()
        .unwrap_or_else(|| "top".to_string());
    vars.insert("cap_location".to_string(), cap_loc);

    let tpl = crate::render::elements::resolve_builtin_partial("table_div", format).unwrap_or("");
    crate::render::template::apply_template(tpl, &vars)
}

fn render_children(children: &[Element], render_element: &dyn Fn(&Element) -> String) -> String {
    children.iter()
        .map(render_element)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

// ---------------------------------------------------------------------------
// Caption extraction (moved from render/table/)
// ---------------------------------------------------------------------------

/// Separate the caption from a table div's children.
/// The caption is the last paragraph that isn't part of a markdown table
/// (i.e., doesn't start with `|`).
pub fn separate_table_caption(children: &[Element]) -> (Vec<Element>, String) {
    let mut content = children.to_vec();
    let mut caption = String::new();

    if let Some(last_idx) = content.iter().rposition(|e| matches!(e, Element::Text { .. })) {
        if let Element::Text { content: ref text } = content[last_idx] {
            let trimmed = text.trim();
            if let Some(split_pos) = trimmed.rfind("\n\n") {
                let last_para = trimmed[split_pos..].trim();
                if !last_para.starts_with('|') && !last_para.is_empty() {
                    caption = last_para.to_string();
                    let remaining = trimmed[..split_pos].trim().to_string();
                    content[last_idx] = Element::Text { content: remaining };
                }
            } else if !trimmed.starts_with('|') && !trimmed.is_empty() {
                caption = trimmed.to_string();
                content.remove(last_idx);
            }
        }
    }

    (content, caption)
}
