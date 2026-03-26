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
    module_ids: &std::cell::RefCell<HashMap<String, String>>,
    template_env: &crate::render::template::TemplateEnv,
    defaults: &crate::config::Metadata,
) -> String {
    let matching = registry.matching_modules(classes, attrs, id.as_deref(), format, "div");

    // Phase 1: Element children transforms (structural rewriting)
    for (plugin, _filter_spec) in &matching {
        if let ModuleKind::ElementChildren(ref p) = plugin.kind {
            let mut ctx = ModuleContext::new(
                classes, id, attrs, children, format, defaults,
                render_element, raw_fragments, module_ids,
            );
            match p.apply(&mut ctx) {
                ModuleResult::Rendered(output) => return output,
                ModuleResult::Continue | ModuleResult::Pass => {}
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
    build_div_vars(&mut vars, classes, id, attrs, &children_rendered, format);

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

}

// ---------------------------------------------------------------------------
// Cross-reference ID validation
// ---------------------------------------------------------------------------

fn validate_div_id(id: &str, classes: &[String]) -> Option<String> {
    let prefix = match id.find('-') {
        Some(pos) => &id[..pos],
        None => return None,
    };

    let all_prefixes = crate::registry::all_crossref_prefixes();
    if !all_prefixes.iter().any(|(p, _)| *p == prefix) {
        return None;
    }

    // fig- and tbl- are always valid (matched by id_prefix in modules.toml)
    if prefix == "fig" || prefix == "tbl" {
        return None;
    }

    // Check if any class owns this prefix via module prefix lookup
    for cls in classes {
        if let Some(p) = crate::modules::prefix_for_class(cls) {
            if p == prefix { return None; }
        }
    }

    let prefix_list: Vec<&str> = all_prefixes.iter().map(|(p, _)| *p).collect();
    Some(format!(
        "Error: fenced div id '{}' uses reserved cross-reference prefix '{}'. \
         Reserved prefixes are: {}. \
         Use a matching class (e.g., ::: {{.theorem #thm-...}}) or choose a different id.",
        id, prefix, prefix_list.join(", "),
    ))
}
