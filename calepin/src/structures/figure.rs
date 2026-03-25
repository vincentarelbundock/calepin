//! Figure div utilities: caption extraction for `#fig-` prefixed divs.

use crate::types::Element;

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
