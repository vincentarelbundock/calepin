//! Div rendering: orchestrates the plugin pipeline for fenced divs.
//!
//! Dispatch order:
//! 1. Iterate matching plugins in registry order (user first, then built-in)
//!    - Raw plugins: receive raw children, render them directly
//!    - Rendered plugins: receive pre-rendered children + template vars
//! 2. Template lookup (fallback)

use std::collections::HashMap;

use crate::registry::{ModuleKind, ModuleContext, ModuleResult, ModuleRegistry};
use crate::types::Element;

/// Render a fenced div through the unified plugin pipeline.
pub fn render(
    classes: &[String],
    id: &Option<String>,
    attrs: &HashMap<String, String>,
    children: &[Element],
    format: &str,
    registry: &ModuleRegistry,
    render_element: &dyn Fn(&Element) -> String,
    resolve_partial: &dyn Fn(&str) -> Option<String>,
    raw_fragments: &std::cell::RefCell<Vec<String>>,
    theorem_numbers: &std::cell::RefCell<HashMap<String, String>>,
    template_env: &crate::render::template::TemplateEnv,
    defaults: &crate::config::Metadata,
) -> String {
    let matching = registry.matching_modules(classes, attrs, id.as_deref(), format, "div");

    // Phase 1: Element children transforms (structural rewriting)
    for (plugin, _filter_spec) in &matching {
        if let ModuleKind::ElementChildren(ref p) = plugin.kind {
            let mut ctx = ModuleContext::new(
                classes, id, attrs, children, format, defaults,
                render_element, resolve_partial, raw_fragments,
            );
            match p.apply(&mut ctx) {
                ModuleResult::Rendered(output) => return output,
                ModuleResult::Continue | ModuleResult::Pass => {}
            }
        }
    }

    // Phase 2: Auto-numbering for modules with number=true
    let mut extra_vars: Option<HashMap<String, String>> = None;
    for (_plugin, filter_spec) in &matching {
        if filter_spec.match_rule.number {
            // Find the matching class for numbering
            for cls in classes {
                if filter_spec.match_rule.classes.iter().any(|c| c == cls) {
                    static COUNTERS: std::sync::LazyLock<std::sync::Mutex<HashMap<String, usize>>> =
                        std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));
                    let mut counters = COUNTERS.lock().unwrap();
                    let count = counters.entry(cls.clone()).or_insert(0);
                    *count += 1;
                    let mut vars = HashMap::new();
                    vars.insert("number".to_string(), count.to_string());
                    vars.insert("type_class".to_string(), cls.clone());
                    extra_vars = Some(vars);
                    break;
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

    // Ensure children are rendered and vars are built
    let children_rendered: String = children.iter()
        .map(render_element)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    let mut vars = HashMap::new();
    build_div_vars(&mut vars, classes, id, attrs, &children_rendered, format, defaults);

    // Merge auto-numbering vars
    if let Some(ev) = extra_vars {
        for (k, v) in ev {
            vars.insert(k, v);
        }
    }

    // Register theorem numbers for crossref resolution
    if let (Some(id_val), Some(num)) = (id.as_deref(), vars.get("number")) {
        if !num.is_empty() {
            theorem_numbers.borrow_mut().insert(id_val.to_string(), num.clone());
        }
    }

    // Template lookup: explicit override -> class-based -> fallback
    let (tpl_name, tpl_source) = vars.get("template")
        .and_then(|name| resolve_partial(name).map(|t| (name.clone(), t)))
        .or_else(|| classes.iter().find_map(|cls| resolve_partial(cls).map(|t| (cls.clone(), t))))
        .or_else(|| resolve_partial("div").map(|t| ("div".to_string(), t)))
        .unzip();

    let (tpl_name, tpl_source) = match (tpl_name, tpl_source) {
        (Some(n), Some(s)) => (n, s),
        _ => {
            cwarn!("no partial found for classes [{}]", classes.join(", "));
            return vars.remove("children").unwrap_or_default();
        }
    };

    template_env.render_dynamic(&tpl_name, &tpl_source, &vars)
}

/// Build the default template variables for a div.
fn build_div_vars(
    vars: &mut HashMap<String, String>,
    classes: &[String],
    id: &Option<String>,
    attrs: &HashMap<String, String>,
    children_rendered: &str,
    format: &str,
    defaults: &crate::config::Metadata,
) {
    for (k, val) in attrs {
        vars.insert(k.clone(), val.clone());
    }
    vars.insert("base".to_string(), format.to_string());
    vars.insert("children".to_string(), children_rendered.to_string());
    vars.insert("classes".to_string(), classes.join(" "));

    if let Some(ref id_val) = id {
        vars.insert("id".to_string(), id_val.clone());
    } else {
        vars.insert("id".to_string(), String::new());
    }

    // Labels for localisable strings
    let label_defs = defaults.labels.clone();
    vars.insert("label_proof".to_string(),
        label_defs.as_ref().and_then(|l| l.proof.clone()).unwrap_or_else(|| "Proof".to_string()));

    // Render caption from raw markdown
    if let Some(raw_cap) = vars.get("fig_cap").cloned().or_else(|| vars.get("tbl_cap").cloned()) {
        if !raw_cap.is_empty() {
            let rendered = crate::render::convert::render_inline(&raw_cap, format);
            vars.insert("caption".to_string(), rendered);
        }
    }

    // Figure div enrichments
    if id.as_ref().map_or(false, |i| i.starts_with("fig-")) {
        let id_val = id.as_ref().unwrap();
        vars.insert("label".to_string(), id_val.clone());
        vars.entry("template".to_string()).or_insert_with(|| "figure_div".to_string());

        let fig_attrs = crate::render::filter::figure::figure_attrs_from_div(attrs);
        crate::render::filter::figure::build_figure_wrapper_vars(
            vars, &fig_attrs, format, None, defaults,
        );
    }

    // Table div enrichments
    if id.as_ref().map_or(false, |i| i.starts_with("tbl-")) {
        let id_val = id.as_ref().unwrap();
        vars.insert("label".to_string(), id_val.clone());
        vars.entry("template".to_string()).or_insert_with(|| "table_div".to_string());

        let cap_loc = vars.get("tbl_cap_location")
            .cloned()
            .unwrap_or_else(|| "top".to_string());
        vars.insert("cap_location".to_string(), cap_loc);
    }
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

    if prefix == "fig" || prefix == "tbl" {
        return None;
    }

    for cls in classes {
        if let Some(p) = crate::render::filter::theorem::theorem_prefix(cls) {
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
