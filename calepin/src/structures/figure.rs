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

    let align = attrs.get("fig-align").map(|s| s.as_str()).unwrap_or("center");
    let align_style = format_align(align, format);

    let mut vars = HashMap::new();
    vars.insert("base".to_string(), format.to_string());
    vars.insert("children".to_string(), content_rendered);
    vars.insert("id".to_string(), id.to_string());
    vars.insert("align".to_string(), align.to_string());
    vars.insert("align_style".to_string(), align_style);

    // Pre-built format-specific strings (avoids triple-brace issues in Jinja)
    let fig_env = attrs.get("fig-env").map(|s| s.as_str()).unwrap_or("figure");
    let fig_pos = attrs.get("fig-pos").map(|s| format!("[{}]", s)).unwrap_or_default();
    vars.insert("fig_begin".to_string(), format!("\\begin{{{}}}{}", fig_env, fig_pos));
    vars.insert("fig_end".to_string(), format!("\\end{{{}}}", fig_env));
    vars.insert("caption_cmd".to_string(), if cap_rendered.is_empty() {
        String::new()
    } else {
        format!("\\caption{{{}}}", &cap_rendered)
    });
    vars.insert("caption".to_string(), cap_rendered);
    vars.insert("label".to_string(), match format {
        "latex" => format!("\\label{{{}}}", id),
        "typst" => format!("<{}>", id),
        _ => String::new(),
    });

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
