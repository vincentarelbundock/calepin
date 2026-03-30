//! Theorem module: handles theorem-type divs (.theorem, .lemma, .proof, etc.)
//!
//! Auto-numbers matching divs and renders them with the appropriate template
//! (theorem_italic, theorem_normal, or proof). Registered as a
//! TransformElementChildren module matching theorem classes.

use std::collections::HashMap;

use crate::types::Element;
use crate::render::template::TemplateVars;

/// Theorem class to template mapping.
const ITALIC_TYPES: &[&str] = &["theorem", "lemma", "corollary", "conjecture", "proposition"];
const NORMAL_TYPES: &[&str] = &["definition", "example", "exercise", "solution", "remark", "algorithm"];

/// Cross-reference prefix mapping (class -> short prefix).
pub const THEOREM_PREFIXES: &[(&str, &str)] = &[
    ("theorem", "thm"), ("lemma", "lem"), ("corollary", "cor"),
    ("proposition", "prp"), ("conjecture", "cnj"), ("definition", "def"),
    ("example", "exm"), ("exercise", "exr"), ("solution", "sol"),
    ("remark", "rem"), ("algorithm", "alg"),
];

/// Return the cross-reference prefix for a theorem class, if any.
pub fn theorem_prefix(class: &str) -> Option<&'static str> {
    THEOREM_PREFIXES.iter().find(|(c, _)| *c == class).map(|(_, p)| *p)
}

/// Render a theorem div: auto-number, build vars, apply template.
pub fn render(
    classes: &[String],
    id: &Option<String>,
    attrs: &HashMap<String, String>,
    children: &[Element],
    format: &str,
    render_element: &dyn Fn(&Element) -> String,
    defaults: &crate::config::Metadata,
    module_ids: &std::cell::RefCell<HashMap<String, String>>,
) -> String {
    // Find the matching theorem class
    let theorem_class = classes.iter().find(|c| {
        ITALIC_TYPES.contains(&c.as_str())
            || NORMAL_TYPES.contains(&c.as_str())
            || c.as_str() == "proof"
    });

    let theorem_class = match theorem_class {
        Some(c) => c.clone(),
        None => return render_children(children, render_element),
    };

    // Render children
    let children_rendered: String = children.iter()
        .map(render_element)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    let mut vars = TemplateVars::with_writer(format);
    vars.calepin.insert("children".to_string(), children_rendered);
    vars.config.insert("classes".to_string(), classes.join(" "));
    vars.calepin.insert("type_class".to_string(), theorem_class.clone());

    if let Some(ref id_val) = id {
        vars.config.insert("id".to_string(), id_val.clone());
    } else {
        vars.config.insert("id".to_string(), String::new());
    }

    // Copy div attrs into vars
    for (k, val) in attrs {
        vars.config.insert(k.clone(), val.clone());
    }

    // Auto-numbering (proof is not numbered).
    // Uses module_ids with a synthetic key `__thm_count:{class}` to track
    // per-class counters within a single render (resets per document).
    if theorem_class != "proof" {
        let counter_key = format!("__thm_count:{}", theorem_class);
        let mut ids = module_ids.borrow_mut();
        let prev = ids.get(&counter_key).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
        let num = prev + 1;
        ids.insert(counter_key, num.to_string());

        // Register for crossref resolution
        if let Some(ref id_val) = id {
            ids.insert(id_val.clone(), num.to_string());
        }
        drop(ids);

        vars.calepin.insert("number".to_string(), num.to_string());
    }

    // Labels for localisable strings
    let label_defs = defaults.labels.clone();
    vars.calepin.insert("label_proof".to_string(),
        label_defs.as_ref().and_then(|l| l.proof.clone()).unwrap_or_else(|| "Proof".to_string()));

    // Resolve template: proof -> "proof", italic types -> "theorem_italic", normal -> "theorem_normal"
    let template_name = if theorem_class == "proof" {
        "proof"
    } else if ITALIC_TYPES.contains(&theorem_class.as_str()) {
        "theorem_italic"
    } else {
        "theorem_normal"
    };

    let tpl = crate::render::elements::resolve_builtin_template(template_name, format).unwrap_or("");
    crate::render::template::apply_template(tpl, &vars)
}

fn render_children(children: &[Element], render_element: &dyn Fn(&Element) -> String) -> String {
    super::render_children(children, render_element)
}
