//! Div filter: orchestrates the rendering pipeline for fenced divs.
//!
//! Dispatch order (unified via plugin registry):
//! 1. Iterate matching plugins in registry order (user first, then built-in)
//!    - Structural plugins: receive raw children + render closure
//!    - Filter/subprocess plugins: receive rendered children as string
//! 2. Template lookup (fallback)

use std::collections::HashMap;

use crate::filters::FilterResult;
use crate::registry::{PluginKind, PluginRegistry};
use crate::types::Element;

/// Render a fenced div through the unified plugin pipeline.
pub fn render(
    classes: &[String],
    id: &Option<String>,
    attrs: &HashMap<String, String>,
    children: &[Element],
    format: &str,
    registry: &PluginRegistry,
    render_element: &dyn Fn(&Element) -> String,
    resolve_template: &dyn Fn(&str) -> Option<String>,
    raw_fragments: &std::cell::RefCell<Vec<String>>,
    theorem_numbers: &std::cell::RefCell<HashMap<String, String>>,
) -> String {
    let matching = registry.matching_filters(classes, attrs, id.as_deref(), format, "div");

    // Lazy child rendering: structural plugins get raw children,
    // filter/subprocess plugins get rendered children.
    let mut children_rendered: Option<String> = None;
    let mut vars: Option<HashMap<String, String>> = None;

    let ensure_rendered = |children_rendered: &mut Option<String>| {
        if children_rendered.is_none() {
            *children_rendered = Some(
                children
                    .iter()
                    .map(render_element)
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            );
        }
    };

    let ensure_vars = |vars: &mut Option<HashMap<String, String>>,
                       children_rendered: &str| {
        if vars.is_some() {
            return;
        }
        let mut v = HashMap::new();
        for (k, val) in attrs {
            v.insert(k.clone(), val.clone());
        }
        v.insert("children".to_string(), children_rendered.to_string());
        v.insert("classes".to_string(), classes.join(" "));

        if let Some(ref id_val) = id {
            v.insert("id_attr".to_string(), format!(" id=\"{}\"", id_val));
            v.insert("id".to_string(), id_val.clone());
        } else {
            v.insert("id_attr".to_string(), String::new());
            v.insert("id".to_string(), String::new());
        }

        let label_str = match id {
            Some(ref id_val) if !id_val.is_empty() => match format {
                "latex" => format!(" \\label{{{}}}", id_val),
                "typst" => format!(" <{}>", id_val),
                _ => String::new(),
            },
            _ => String::new(),
        };
        v.insert("label".to_string(), label_str);
        *vars = Some(v);
    };

    for (plugin, filter_spec) in &matching {
        match &plugin.kind {
            PluginKind::BuiltinStructural(handler) => {
                if let Some(output) = handler.render_div(
                    classes, id, attrs, children, format, render_element, raw_fragments,
                ) {
                    return output;
                }
            }
            PluginKind::BuiltinFilter(filter) => {
                ensure_rendered(&mut children_rendered);
                ensure_vars(&mut vars, children_rendered.as_ref().unwrap());
                let div_element = Element::Div {
                    classes: classes.to_vec(),
                    id: id.clone(),
                    attrs: attrs.clone(),
                    children: vec![],
                };
                match filter.apply(&div_element, format, vars.as_mut().unwrap()) {
                    FilterResult::Rendered(output) => return output,
                    FilterResult::Continue | FilterResult::Pass => {}
                }
            }
            PluginKind::Subprocess { .. } | PluginKind::PersistentSubprocess { .. } => {
                ensure_rendered(&mut children_rendered);
                ensure_vars(&mut vars, children_rendered.as_ref().unwrap());
                if let Some(output) = registry.call_subprocess_filter(
                    plugin,
                    filter_spec,
                    "div",
                    children_rendered.as_ref().unwrap(),
                    classes,
                    id.as_deref().unwrap_or(""),
                    format,
                    vars.as_ref().unwrap(),
                ) {
                    return output;
                }
            }
        }
    }

    // Validate div id (after plugin dispatch)
    if let Some(ref id_val) = id {
        if let Some(err) = validate_div_id(id_val, classes) {
            cwarn!("{}", err);
        }
    }

    // Ensure children are rendered and vars are built for template lookup
    ensure_rendered(&mut children_rendered);
    ensure_vars(&mut vars, children_rendered.as_ref().unwrap());
    let vars = vars.as_mut().unwrap();

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

    crate::render::template::apply_template(&tpl, vars)
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
