//! Table module: handles `#tbl-` prefixed divs.
//!
//! Consolidates caption extraction, variable enrichment, and template
//! rendering for table divs. Registered as a TransformElementChildren
//! module matching `id_prefix = "tbl-"`.

use std::collections::HashMap;

use crate::types::Element;
use crate::render::template::TemplateVars;

/// Render a table div: extract caption from children, build template
/// vars, apply the `table` template.
pub fn render(
    id: &Option<String>,
    attrs: &HashMap<String, String>,
    children: &[Element],
    writer: &str,
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

    // If the rendered children already contain a table float environment
    // (e.g., from tinytable or gt), skip the calepin table wrapper to
    // avoid nested \begin{table} errors.
    if (writer == "latex" && children_rendered.contains("\\begin{table}"))
        || (writer == "typst" && children_rendered.contains("#figure("))
    {
        return children_rendered;
    }

    // Build template vars
    let mut vars = TemplateVars::with_writer(writer);
    vars.clp.insert("content".to_string(), minijinja::Value::from(children_rendered));
    vars.cfg.insert("label".to_string(), minijinja::Value::from(id_val.to_string()));
    vars.cfg.insert("id".to_string(), minijinja::Value::from(id_val.to_string()));

    // Copy div attrs into vars
    for (k, val) in attrs {
        vars.cfg.insert(k.clone(), minijinja::Value::from(val.clone()));
    }

    // Render caption markdown to target format
    if !caption_text.is_empty() {
        let rendered_caption = crate::render::convert::render_inline(&caption_text, writer);
        vars.cfg.insert("caption".to_string(), minijinja::Value::from(rendered_caption));
    }

    // Caption location (default: top for tables)
    let cap_loc = attrs.get("tbl_cap_location")
        .cloned()
        .unwrap_or_else(|| "top".to_string());
    vars.cfg.insert("cap_location".to_string(), minijinja::Value::from(cap_loc));

    let tpl = crate::render::elements::resolve_element_template("table", writer).unwrap_or_default();
    crate::render::template::apply_template(&tpl, &vars)
}

fn render_children(children: &[Element], render_element: &dyn Fn(&Element) -> String) -> String {
    super::render_children(children, render_element)
}

// ---------------------------------------------------------------------------
// Caption extraction
// ---------------------------------------------------------------------------

/// Separate the caption from a table div's children.
/// The caption is the last paragraph that isn't part of a markdown table
/// (i.e., doesn't start with `|`).
pub fn separate_table_caption(children: &[Element]) -> (Vec<Element>, String) {
    super::figure::separate_caption(children, |text| {
        if let Some(split_pos) = text.rfind("\n\n") {
            let last_para = text[split_pos..].trim();
            if !last_para.starts_with('|') && !last_para.is_empty() {
                return (last_para.to_string(), Some(text[..split_pos].trim().to_string()));
            }
        } else if !text.starts_with('|') {
            return (text.to_string(), None);
        }
        (String::new(), None)
    })
}
