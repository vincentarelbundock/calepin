//! Placeholder span module: `[]{.placeholder width=600 height=400}` -> placeholder image.

use std::collections::HashMap;

use crate::render::template::TemplateVars;

pub fn render(
    kv: &HashMap<String, String>,
    writer: &str,
    defaults: &crate::config::Metadata,
) -> String {
    let pdefs = defaults.placeholder.as_ref();
    let default_pw = pdefs.and_then(|p| p.width.clone()).unwrap_or_else(|| "600".to_string());
    let default_ph = pdefs.and_then(|p| p.height.clone()).unwrap_or_else(|| "400".to_string());
    let width = kv.get("width").map(|s| s.as_str()).unwrap_or(&default_pw);
    let height = kv.get("height").map(|s| s.as_str()).unwrap_or(&default_ph);
    let text = kv.get("text")
        .cloned()
        .unwrap_or_else(|| format!("{}\u{00d7}{}", width, height));

    let mut vars = TemplateVars::new();
    vars.cfg.insert("width".to_string(), minijinja::Value::from(width.to_string()));
    vars.cfg.insert("height".to_string(), minijinja::Value::from(height.to_string()));
    vars.cfg.insert("text".to_string(), minijinja::Value::from(crate::util::escape_html(&text)));

    let fallback = format!("[{} ({}x{})]", text, width, height);
    match crate::render::elements::resolve_element_template("placeholder", writer) {
        Some(tpl) => crate::render::template::apply_template(&tpl, &vars),
        None => fallback,
    }
}
