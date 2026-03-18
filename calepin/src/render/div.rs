//! Div filter: orchestrates the rendering pipeline for fenced divs.

use std::collections::HashMap;

use crate::types::Element;
use crate::filters::{self, Filter, FilterResult};
use crate::plugins::PluginHandle;

/// Render a fenced div through the full filter pipeline.
pub fn render(
    classes: &[String],
    id: &Option<String>,
    attrs: &HashMap<String, String>,
    children: &[Element],
    format: &str,
    plugins: &[PluginHandle],
    builtin_filters: &[Box<dyn Filter>],
    render_element: &dyn Fn(&Element) -> String,
    resolve_template: &dyn Fn(&str) -> Option<String>,
    raw_fragments: &std::cell::RefCell<Vec<String>>,
    theorem_numbers: &std::cell::RefCell<HashMap<String, String>>,
) -> String {
    // Tabset
    if classes.iter().any(|c| c == "panel-tabset") {
        return crate::structures::tabset::render(format, attrs, children, render_element);
    }

    // Layout
    if attrs.contains_key("layout-ncol") || attrs.contains_key("layout-nrow") || attrs.contains_key("layout") {
        return crate::structures::layout::render(id, attrs, children, format, render_element, raw_fragments);
    }

    // Figure div
    if let Some(ref id_val) = id {
        if id_val.starts_with("fig-") && !classes.iter().any(|c| c.starts_with("callout-")) {
            return crate::structures::figure::render_div(id_val, attrs, children, format, render_element, raw_fragments);
        }
    }

    // Validate div id
    if let Some(ref id_val) = id {
        if let Some(err) = validate_div_id(id_val, classes) {
            cwarn!("{}", err);
        }
    }

    // Render children
    let children_rendered: String = children
        .iter()
        .map(render_element)
        .collect::<Vec<_>>()
        .join("\n\n");

    // Build vars
    let mut vars = HashMap::new();
    for (k, v) in attrs {
        vars.insert(k.clone(), v.clone());
    }
    vars.insert("children".to_string(), children_rendered.clone());
    vars.insert("classes".to_string(), classes.join(" "));

    if let Some(ref id_val) = id {
        vars.insert("id-attr".to_string(), format!(" id=\"{}\"", id_val));
        vars.insert("id".to_string(), id_val.clone());
    } else {
        vars.insert("id-attr".to_string(), String::new());
        vars.insert("id".to_string(), String::new());
    }

    let label_str = match id {
        Some(ref id_val) if !id_val.is_empty() => match format {
            "latex" => format!(" \\label{{{}}}", id_val),
            "typst" => format!(" <{}>", id_val),
            _ => String::new(),
        },
        _ => String::new(),
    };
    vars.insert("label".to_string(), label_str);

    // Reconstruct element for uniform filter interface
    let div_element = Element::Div {
        classes: classes.to_vec(),
        id: id.clone(),
        attrs: attrs.clone(),
        children: vec![], // children already rendered into vars["children"]
    };

    // Filter pipeline: WASM plugins → external → built-in
    for plugin in plugins {
        let ctx = crate::plugins::FilterContext {
            context: "div".to_string(),
            content: children_rendered.clone(),
            classes: classes.to_vec(),
            id: id.as_deref().unwrap_or("").to_string(),
            format: format.to_string(),
            attrs: vars.clone(),
        };
        match plugin.call_filter(&ctx) {
            crate::plugins::FilterResult::Rendered(output) => return output,
            crate::plugins::FilterResult::Pass => {}
        }
    }

    for cls in classes {
        if let Some(filter) = filters::resolve_external_filter(cls, format) {
            match filter.apply(&div_element, format, &mut vars) {
                FilterResult::Rendered(output) => return output,
                FilterResult::Continue | FilterResult::Pass => {}
            }
        }
    }

    for filter in builtin_filters {
        match filter.apply(&div_element, format, &mut vars) {
            FilterResult::Rendered(output) => return output,
            FilterResult::Continue | FilterResult::Pass => {}
        }
    }

    // Register theorem numbers for crossref resolution
    if let (Some(id_val), Some(num)) = (id.as_deref(), vars.get("number")) {
        if !num.is_empty() {
            theorem_numbers.borrow_mut().insert(id_val.to_string(), num.clone());
        }
    }

    // Template lookup: explicit override → class-based → fallback
    let tpl = vars.get("template")
        .and_then(|name| resolve_template(name))
        .or_else(|| classes.iter().find_map(|cls| resolve_template(cls)))
        .or_else(|| resolve_template("div"));

    let tpl = match tpl {
        Some(t) => t,
        None => {
            cwarn!("no template found for classes [{}]", classes.join(", "));
            return vars.remove("children").unwrap_or_default();
        }
    };

    crate::render::template::apply_template(&tpl, &vars)
}

// ---------------------------------------------------------------------------
// Cross-reference ID validation
// ---------------------------------------------------------------------------

const RESERVED_PREFIXES: &[&str] = &[
    "fig", "tbl", "lst", "tip", "nte", "wrn", "imp", "cau",
    "thm", "lem", "cor", "prp", "cnj", "def", "exm", "exr",
    "sol", "rem", "alg", "eq", "sec",
];

fn validate_div_id(id: &str, classes: &[String]) -> Option<String> {
    let prefix = match id.find('-') {
        Some(pos) => &id[..pos],
        None => return None,
    };

    if !RESERVED_PREFIXES.contains(&prefix) {
        return None;
    }

    for cls in classes {
        if let Some(p) = crate::filters::theorem::theorem_prefix(cls) {
            if p == prefix {
                return None;
            }
        }
    }

    let callout_map: &[(&str, &str)] = &[
        ("callout-tip", "tip"), ("callout-note", "nte"),
        ("callout-warning", "wrn"), ("callout-important", "imp"),
        ("callout-caution", "cau"),
    ];
    for cls in classes {
        for (callout_cls, callout_pfx) in callout_map {
            if cls.as_str() == *callout_cls && prefix == *callout_pfx {
                return None;
            }
        }
    }

    Some(format!(
        "Error: fenced div id '{}' uses reserved cross-reference prefix '{}'. \
         Reserved prefixes are: {}. \
         Use a matching class (e.g., ::: {{.theorem #thm-...}}) or choose a different id.",
        id, prefix, RESERVED_PREFIXES.join(", "),
    ))
}
