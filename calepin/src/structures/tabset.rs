//! Tabset filter: converts `.panel-tabset` divs into tabbed navigation.

use std::collections::HashMap;

use regex::Regex;

use crate::types::Element;

/// Render a `.panel-tabset` div as tabs (HTML) or plain sections (other formats).
pub fn render(
    format: &str,
    attrs: &HashMap<String, String>,
    children: &[Element],
    render_element: &dyn Fn(&Element) -> String,
) -> String {
    let rendered_parts: Vec<String> = children
        .iter()
        .map(render_element)
        .filter(|s| !s.is_empty())
        .collect();
    let rendered = rendered_parts.join("\n\n");
    if format != "html" {
        return rendered;
    }

    let heading_re = Regex::new(r#"<h([2-6])[^>]*>(.*?)</h[2-6]>"#).unwrap();
    let mut tabs: Vec<(String, String)> = Vec::new();
    let mut positions: Vec<(usize, usize, String)> = Vec::new();

    for caps in heading_re.captures_iter(&rendered) {
        let full = caps.get(0).unwrap();
        let tag_re = Regex::new(r"<[^>]+>").unwrap();
        let title = tag_re.replace_all(&caps[2], "").trim().to_string();
        positions.push((full.start(), full.end(), title));
    }

    if positions.is_empty() {
        return rendered;
    }

    for i in 0..positions.len() {
        let content_start = positions[i].1;
        let content_end = if i + 1 < positions.len() {
            positions[i + 1].0
        } else {
            rendered.len()
        };
        let content = rendered[content_start..content_end].trim().to_string();
        tabs.push((positions[i].2.clone(), content));
    }

    let group = attrs.get("group").map(|s| s.as_str()).unwrap_or("");
    let group_attr = if group.is_empty() {
        String::new()
    } else {
        format!(" data-group=\"{}\"", group)
    };

    let mut html = format!(
        "<div class=\"panel-tabset\"{}>\n<ul class=\"nav nav-tabs\" role=\"tablist\">\n",
        group_attr
    );

    for (i, (title, _)) in tabs.iter().enumerate() {
        let active = if i == 0 { " active" } else { "" };
        let selected = if i == 0 { "true" } else { "false" };
        let id = crate::util::slugify(title);
        html.push_str(&format!(
            "  <li class=\"nav-item\" role=\"presentation\"><button class=\"nav-link{}\" data-tab=\"{}\" role=\"tab\" aria-selected=\"{}\" aria-controls=\"tabpanel-{}\">{}</button></li>\n",
            active, id, selected, id, title
        ));
    }
    html.push_str("</ul>\n<div class=\"tab-content\">\n");

    for (i, (title, content)) in tabs.iter().enumerate() {
        let active = if i == 0 { " active" } else { "" };
        let hidden = if i == 0 { "" } else { " aria-hidden=\"true\"" };
        let id = crate::util::slugify(title);
        html.push_str(&format!(
            "<div class=\"tab-pane{}\" id=\"tabpanel-{}\" data-tab=\"{}\" role=\"tabpanel\"{}>\n{}\n</div>\n",
            active, id, id, hidden, content
        ));
    }
    html.push_str("</div>\n</div>");

    html
}
