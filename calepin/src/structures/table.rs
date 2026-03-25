//! Table div utilities: caption extraction for `#tbl-` prefixed divs.

use crate::types::Element;

/// Separate the caption from a table div's children.
/// The caption is the last paragraph that isn't part of a markdown table
/// (i.e., doesn't start with `|`).
pub fn separate_table_caption(children: &[Element]) -> (Vec<Element>, String) {
    let mut content = children.to_vec();
    let mut caption = String::new();

    // Find the last Text element
    if let Some(last_idx) = content.iter().rposition(|e| matches!(e, Element::Text { .. })) {
        if let Element::Text { content: ref text } = content[last_idx] {
            let trimmed = text.trim();
            // Split on double newline to find the last paragraph
            if let Some(split_pos) = trimmed.rfind("\n\n") {
                let last_para = trimmed[split_pos..].trim();
                // If the last paragraph doesn't look like a table, it's the caption
                if !last_para.starts_with('|') && !last_para.is_empty() {
                    caption = last_para.to_string();
                    let remaining = trimmed[..split_pos].trim().to_string();
                    content[last_idx] = Element::Text { content: remaining };
                }
            } else if !trimmed.starts_with('|') && !trimmed.is_empty() {
                // Single paragraph with no table -- it's just a caption
                caption = trimmed.to_string();
                content.remove(last_idx);
            }
        }
    }

    (content, caption)
}
