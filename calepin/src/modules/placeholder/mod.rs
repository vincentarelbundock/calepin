//! Placeholder span module: `[]{.placeholder width=600 height=400}` -> placeholder image.

use std::collections::HashMap;

pub fn render(
    kv: &HashMap<String, String>,
    format: &str,
    defaults: &crate::config::Metadata,
) -> String {
    let pdefs = defaults.placeholder.as_ref();
    let default_pw = pdefs.and_then(|p| p.width.clone()).unwrap_or_else(|| "600".to_string());
    let default_ph = pdefs.and_then(|p| p.height.clone()).unwrap_or_else(|| "400".to_string());
    let default_color = pdefs.and_then(|p| p.color.clone()).unwrap_or_else(|| "#cccccc".to_string());

    let width = kv.get("width").map(|s| s.as_str()).unwrap_or(&default_pw);
    let height = kv.get("height").map(|s| s.as_str()).unwrap_or(&default_ph);
    let color = kv.get("color").map(|s| s.as_str()).unwrap_or(&default_color);
    let text = kv.get("text")
        .cloned()
        .unwrap_or_else(|| format!("{}\u{00d7}{}", width, height));

    let mut vars = HashMap::new();
    vars.insert("width".to_string(), width.to_string());
    vars.insert("height".to_string(), height.to_string());
    vars.insert("color".to_string(), crate::util::escape_html(color));
    vars.insert("text".to_string(), crate::util::escape_html(&text));

    let fallback = format!("[{} ({}x{})]", text, width, height);
    match crate::render::elements::resolve_builtin_partial("placeholder", format) {
        Some(tpl) => crate::render::template::apply_template(tpl, &vars),
        None => fallback,
    }
}
