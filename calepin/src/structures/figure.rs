//! Figure div structure: renders `#fig-` prefixed divs as figure environments.

use std::collections::HashMap;

use crate::types::Element;
use crate::filters::figure::format_align;

/// Render a `#fig-` prefixed div as a figure environment.
pub fn render_div(
    id: &str,
    attrs: &HashMap<String, String>,
    children: &[Element],
    format: &str,
    render_element: &dyn Fn(&Element) -> String,
    resolve_template: &dyn Fn(&str) -> Option<String>,
) -> String {
    // Caption: fig-cap attribute takes priority, then last text element in div
    let (content_children, caption) = if let Some(cap) = attrs.get("fig-cap") {
        (children.to_vec(), cap.clone())
    } else {
        separate_figure_caption(children)
    };

    let content_rendered: String = content_children.iter()
        .map(render_element)
        .collect::<Vec<_>>()
        .join("\n\n");

    let cap_rendered = if !caption.is_empty() {
        crate::render::markdown::render_inline(&caption, format)
    } else {
        String::new()
    };

    let defs = crate::project::get_defaults();
    let default_align = defs.figure.as_ref().and_then(|f| f.alignment.as_deref()).unwrap_or("center");
    let align = attrs.get("fig-align").map(|s| s.as_str()).unwrap_or(default_align);
    let align_style = format_align(align, format);

    let mut vars = HashMap::new();
    vars.insert("base".to_string(), format.to_string());
    vars.insert("engine".to_string(), format.to_string());
    vars.insert("children".to_string(), content_rendered);
    vars.insert("id".to_string(), id.to_string());
    vars.insert("align".to_string(), align.to_string());
    vars.insert("align_style".to_string(), align_style);

    if let Some(env) = attrs.get("fig-env") {
        vars.insert("fig_env".to_string(), env.clone());
    }
    if let Some(pos) = attrs.get("fig-pos") {
        vars.insert("fig_pos".to_string(), format!("[{}]", pos));
    }
    vars.insert("caption".to_string(), cap_rendered);
    // label is the raw id -- template constructs format-specific syntax
    vars.insert("label".to_string(), id.to_string());

    let tpl = resolve_template("figure_div")
        .unwrap_or_else(|| include_str!("../project/templates/common/figure_div.jinja").to_string());
    crate::render::template::apply_template(&tpl, &vars)
}

/// Separate the caption from children in a figure div.
pub fn separate_figure_caption(children: &[Element]) -> (Vec<Element>, String) {
    let mut content = children.to_vec();
    let mut caption = String::new();
    if let Some(last_idx) = content.iter().rposition(|e| matches!(e, Element::Text { .. })) {
        if let Element::Text { content: ref text } = content[last_idx] {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                caption = trimmed.to_string();
                content.remove(last_idx);
            }
        }
    }
    (content, caption)
}
