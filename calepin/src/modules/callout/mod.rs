//! Callout module: handles `.callout-*` divs (note, tip, warning, caution, important).
//!
//! Renders children and applies the `callout` template. Registers IDs
//! in `module_ids` for cross-referencing.

use std::collections::HashMap;

use crate::types::Element;
use crate::render::template::TemplateVars;

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
    let children_rendered = super::render_children(children, render_element);

    let mut vars = TemplateVars::with_writer(format);
    vars.calepin.insert("children".to_string(), children_rendered);
    vars.config.insert("classes".to_string(), classes.join(" "));

    if let Some(ref id_val) = id {
        vars.config.insert("id".to_string(), id_val.clone());

        // Register ID for cross-referencing (uses synthetic counter key)
        for cls in classes {
            if let Some(prefix) = callout_prefix(cls) {
                let counter_key = format!("__callout_count:{}", prefix);
                let mut ids = module_ids.borrow_mut();
                let prev = ids.get(&counter_key).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
                let num = prev + 1;
                ids.insert(counter_key, num.to_string());
                ids.insert(id_val.clone(), num.to_string());
                break;
            }
        }
    } else {
        vars.config.insert("id".to_string(), String::new());
    }

    // Copy div attrs into vars (title, icon, collapse, appearance)
    for (k, val) in attrs {
        vars.config.insert(k.clone(), val.clone());
    }

    let tpl = crate::render::elements::resolve_builtin_template("callout", format).unwrap_or("");
    crate::render::template::apply_template(tpl, &vars)
}
