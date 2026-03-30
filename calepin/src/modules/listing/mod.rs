//! Listing module: wraps `lst-` labeled code blocks in a listing div with
//! auto-numbering and optional caption.

use std::collections::HashMap;

use crate::render::template::TemplateVars;

/// Wrap a rendered code block in a listing div if it has a `lst-` label.
/// Returns `None` if the element is not a listing, `Some(output)` if wrapped.
pub fn wrap_listing(
    label: &str,
    lst_cap: Option<&str>,
    rendered_code: &str,
    format: &str,
    module_ids: &std::cell::RefCell<HashMap<String, String>>,
    metadata: &crate::config::Metadata,
    template_env: &crate::render::template::TemplateEnv,
    resolve_template: &dyn Fn(&str) -> Option<String>,
) -> String {
    // Track listing ID for cross-references
    let ids = module_ids.borrow();
    let count = ids.keys().filter(|k| k.starts_with("lst-")).count();
    drop(ids);
    let num = count + 1;
    module_ids.borrow_mut().insert(label.to_string(), num.to_string());

    let label_defs = metadata.labels.clone();
    let mut vars = TemplateVars::with_writer(format);
    vars.clp.insert("label".to_string(), label.to_string());
    vars.clp.insert("number".to_string(), num.to_string());
    vars.clp.insert("content".to_string(), rendered_code.to_string());
    vars.clp.insert("label_listing".to_string(),
        label_defs.as_ref().and_then(|l| l.listing.clone())
            .unwrap_or_else(|| "Listing".to_string()));
    if let Some(cap) = lst_cap {
        vars.cfg.insert("lst_cap".to_string(), cap.to_string());
    }

    let tpl = resolve_template("code_listing")
        .unwrap_or_else(|| crate::render::elements::resolve_builtin_template("code_listing", format)
            .unwrap_or("").to_string());
    template_env.render_dynamic("code_listing", &tpl, &vars)
}
