//! Callout module: handles `.callout-*` divs (note, tip, warning, caution, important).
//!
//! Renders children and applies the `callout` template. Registers IDs
//! in `module_ids` for cross-referencing.

use std::collections::HashMap;

use crate::types::Element;

/// Cross-reference prefix mapping (class -> short prefix).
pub const CALLOUT_PREFIXES: &[(&str, &str)] = &[
    ("callout-tip", "tip"),
    ("callout-note", "nte"),
    ("callout-warning", "wrn"),
    ("callout-important", "imp"),
    ("callout-caution", "cau"),
];

/// Return the cross-reference prefix for a callout class, if any.
pub fn callout_prefix(class: &str) -> Option<&'static str> {
    CALLOUT_PREFIXES.iter().find(|(c, _)| *c == class).map(|(_, p)| *p)
}

/// Render a callout div.
pub fn render(
    classes: &[String],
    id: &Option<String>,
    attrs: &HashMap<String, String>,
    children: &[Element],
    format: &str,
    render_element: &dyn Fn(&Element) -> String,
    module_ids: &std::cell::RefCell<HashMap<String, String>>,
) -> String {
    let children_rendered: String = children.iter()
        .map(render_element)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    let mut vars = HashMap::new();
    vars.insert("base".to_string(), format.to_string());
    vars.insert("writer".to_string(), format.to_string());
    vars.insert("children".to_string(), children_rendered);
    vars.insert("classes".to_string(), classes.join(" "));

    if let Some(ref id_val) = id {
        vars.insert("id".to_string(), id_val.clone());

        // Register ID for cross-referencing
        for cls in classes {
            if let Some(prefix) = callout_prefix(cls) {
                let ids = module_ids.borrow();
                let count = ids.keys().filter(|k| k.starts_with(prefix)).count();
                drop(ids);
                module_ids.borrow_mut().insert(id_val.clone(), (count + 1).to_string());
                break;
            }
        }
    } else {
        vars.insert("id".to_string(), String::new());
    }

    // Copy div attrs into vars (title, icon, collapse, appearance)
    for (k, val) in attrs {
        vars.insert(k.clone(), val.clone());
    }

    let tpl = crate::render::elements::resolve_builtin_partial("callout", format).unwrap_or("");
    crate::render::template::apply_template(tpl, &vars)
}
