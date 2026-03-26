//! Theorem module: handles theorem-type divs (.theorem, .lemma, .proof, etc.)
//!
//! Auto-numbers matching divs and renders them with the appropriate template
//! (theorem_italic, theorem_normal, or proof). Registered as a
//! TransformElementChildren module matching theorem classes.

use std::collections::HashMap;

use crate::types::Element;

/// Theorem class to template mapping.
const ITALIC_TYPES: &[&str] = &["theorem", "lemma", "corollary", "conjecture", "proposition"];
const NORMAL_TYPES: &[&str] = &["definition", "example", "exercise", "solution", "remark", "algorithm"];

/// Cross-reference prefix mapping (class -> short prefix).
const THEOREM_PREFIXES: &[(&str, &str)] = &[
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

    let mut vars = HashMap::new();
    vars.insert("base".to_string(), format.to_string());
    vars.insert("engine".to_string(), format.to_string());
    vars.insert("children".to_string(), children_rendered);
    vars.insert("classes".to_string(), classes.join(" "));
    vars.insert("type_class".to_string(), theorem_class.clone());

    if let Some(ref id_val) = id {
        vars.insert("id".to_string(), id_val.clone());
    } else {
        vars.insert("id".to_string(), String::new());
    }

    // Copy div attrs into vars
    for (k, val) in attrs {
        vars.insert(k.clone(), val.clone());
    }

    // Auto-numbering (proof is not numbered)
    if theorem_class != "proof" {
        static COUNTERS: std::sync::LazyLock<std::sync::Mutex<HashMap<String, usize>>> =
            std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));
        let mut counters = COUNTERS.lock().unwrap();
        let count = counters.entry(theorem_class.clone()).or_insert(0);
        *count += 1;
        let num = count.to_string();

        // Register for crossref resolution
        if let Some(ref id_val) = id {
            module_ids.borrow_mut().insert(id_val.clone(), num.clone());
        }

        vars.insert("number".to_string(), num);
    }

    // Labels for localisable strings
    let label_defs = defaults.labels.clone();
    vars.insert("label_proof".to_string(),
        label_defs.as_ref().and_then(|l| l.proof.clone()).unwrap_or_else(|| "Proof".to_string()));

    // Resolve template: proof -> "proof", italic types -> "theorem_italic", normal -> "theorem_normal"
    let template_name = if theorem_class == "proof" {
        "proof"
    } else if ITALIC_TYPES.contains(&theorem_class.as_str()) {
        "theorem_italic"
    } else {
        "theorem_normal"
    };

    let tpl = crate::render::elements::resolve_builtin_partial(template_name, format).unwrap_or("");
    crate::render::template::apply_template(tpl, &vars)
}

fn render_children(children: &[Element], render_element: &dyn Fn(&Element) -> String) -> String {
    children.iter()
        .map(render_element)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}
