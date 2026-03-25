//! Div filter: orchestrates the rendering pipeline for fenced divs.
//!
//! Dispatch order (unified via plugin registry):
//! 1. Iterate matching plugins in registry order (user first, then built-in)
//!    - Structural plugins: receive raw children + render closure
//!    - Filter/subprocess plugins: receive rendered children as string
//! 2. Template lookup (fallback)

use std::collections::HashMap;

use crate::render::transform_element::FilterResult;
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
    template_env: &crate::render::template::TemplateEnv,
    defaults: &crate::metadata::Metadata,
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
        v.insert("base".to_string(), format.to_string());
        v.insert("children".to_string(), children_rendered.to_string());
        v.insert("classes".to_string(), classes.join(" "));

        if let Some(ref id_val) = id {
            v.insert("id".to_string(), id_val.clone());
        } else {
            v.insert("id".to_string(), String::new());
        }

        // Labels for localisable strings in div templates (proof, etc.)
        let label_defs = defaults.labels.clone();
        v.insert("label_proof".to_string(), label_defs.as_ref().and_then(|l| l.proof.clone()).unwrap_or_else(|| "Proof".to_string()));

        // Render caption from raw markdown for fig-/tbl- divs
        if let Some(raw_cap) = v.get("fig_cap").cloned().or_else(|| v.get("tbl_cap").cloned()) {
            if !raw_cap.is_empty() {
                let rendered = crate::render::convert::render_inline(&raw_cap, format);
                v.insert("caption".to_string(), rendered);
            }
        }

        // Figure div enrichments
        if id.as_ref().map_or(false, |i| i.starts_with("fig-")) {
            let id_val = id.as_ref().unwrap();
            v.insert("label".to_string(), id_val.clone());
            v.entry("template".to_string()).or_insert_with(|| "figure_div".to_string());

            let default_align = defaults.figure.as_ref()
                .and_then(|f| f.alignment.as_deref()).unwrap_or("center");
            let align = v.get("fig_align").cloned().unwrap_or_else(|| default_align.to_string());
            let align_style = crate::render::transform_element::figure::format_align(&align, format);
            v.insert("align".to_string(), align);
            v.insert("align_style".to_string(), align_style);

            if let Some(pos) = v.get("fig_pos").cloned() {
                v.insert("fig_pos".to_string(), format!("[{}]", pos));
            }
            if !v.contains_key("fig_env") {
                v.insert("fig_env".to_string(), "figure".to_string());
            }
        }

        // Table div enrichments
        if id.as_ref().map_or(false, |i| i.starts_with("tbl-")) {
            let id_val = id.as_ref().unwrap();
            v.insert("label".to_string(), id_val.clone());
            v.entry("template".to_string()).or_insert_with(|| "table_div".to_string());

            let cap_loc = v.get("tbl_cap_location")
                .cloned()
                .unwrap_or_else(|| "top".to_string());
            v.insert("cap_location".to_string(), cap_loc);
        }

        *vars = Some(v);
    };

    for (plugin, _filter_spec) in &matching {
        match &plugin.kind {
            PluginKind::BuiltinStructural(handler) => {
                if let Some(output) = handler.render_div(
                    classes, id, attrs, children, format, render_element, resolve_template, raw_fragments, defaults,
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
                match filter.apply(&div_element, format, vars.as_mut().unwrap(), defaults) {
                    FilterResult::Rendered(output) => return output,
                    FilterResult::Continue | FilterResult::Pass => {}
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
    let (tpl_name, tpl_source) = vars.get("template")
        .and_then(|name| resolve_template(name).map(|t| (name.clone(), t)))
        .or_else(|| classes.iter().find_map(|cls| resolve_template(cls).map(|t| (cls.clone(), t))))
        .or_else(|| resolve_template("div").map(|t| ("div".to_string(), t)))
        .unzip();

    let (tpl_name, tpl_source) = match (tpl_name, tpl_source) {
        (Some(n), Some(s)) => (n, s),
        _ => {
            cwarn!("no template found for classes [{}]", classes.join(", "));
            return vars.remove("children").unwrap_or_default();
        }
    };

    template_env.render_dynamic(&tpl_name, &tpl_source, vars)
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

    // fig- and tbl- divs are identified by ID prefix, not classes
    if prefix == "fig" || prefix == "tbl" {
        return None;
    }

    for cls in classes {
        if let Some(p) = crate::render::transform_element::theorem::theorem_prefix(cls) {
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
