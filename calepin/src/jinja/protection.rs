//! Code block protection for Jinja evaluation.
//!
//! Extracts fenced code blocks and inline code spans, replacing them with
//! placeholders that Jinja won't try to evaluate. After Jinja rendering,
//! the placeholders are restored.

use std::sync::LazyLock;
use regex::Regex;

/// Placeholder prefix for protected code blocks (uses Unicode noncharacters).
pub(crate) const CODE_PLACEHOLDER_PREFIX: &str = "\u{FDD0}CODE";
pub(crate) const CODE_PLACEHOLDER_SUFFIX: &str = "\u{FDD1}";

/// Extract fenced code blocks and inline code spans, replacing them with
/// placeholders that Jinja won't try to evaluate.
pub(crate) fn protect_code_blocks(text: &str) -> (String, Vec<String>) {
    let mut blocks: Vec<String> = Vec::new();
    let mut result = String::new();

    // First pass: protect fenced code blocks
    let mut in_fence = false;
    let mut fence_marker = String::new();
    let mut fence_content = String::new();

    for line in text.split('\n') {
        let trimmed = line.trim_start();
        if !in_fence {
            // Check for opening fence (3+ backticks or tildes)
            if let Some(marker) = detect_fence_open(trimmed) {
                in_fence = true;
                fence_marker = marker;
                fence_content = line.to_string();
                fence_content.push('\n');
                continue;
            }
            result.push_str(line);
            result.push('\n');
        } else {
            fence_content.push_str(line);
            fence_content.push('\n');
            // Check for closing fence (same marker)
            if trimmed.starts_with(&fence_marker) && trimmed.trim_end().len() <= fence_marker.len() + 1 {
                // Fence closed -- store and emit placeholder
                let idx = blocks.len();
                // Remove trailing newline from content
                if fence_content.ends_with('\n') {
                    fence_content.pop();
                }
                blocks.push(fence_content.clone());
                result.push_str(&format!("{}{}{}", CODE_PLACEHOLDER_PREFIX, idx, CODE_PLACEHOLDER_SUFFIX));
                result.push('\n');
                fence_content.clear();
                in_fence = false;
            }
        }
    }
    // Handle unclosed fence (shouldn't happen in valid qmd)
    if in_fence {
        result.push_str(&fence_content);
    }

    // Remove trailing newline added by split/join
    if result.ends_with('\n') && !text.ends_with('\n') {
        result.pop();
    }

    (result, blocks)
}

/// Replace inline code spans (`` `...` ``) with placeholders.
/// Only protects spans that contain Jinja-like syntax.
pub(crate) fn protect_inline_code(text: &str, blocks: &mut Vec<String>) -> String {
    let mut result = String::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '`' {
            let start = i;
            let mut tick_count = 0;
            while i < len && chars[i] == '`' {
                tick_count += 1;
                i += 1;
            }
            let mut found_end = false;
            while i <= len - tick_count {
                if chars[i] == '`' {
                    let mut closing = 0;
                    while i < len && chars[i] == '`' {
                        closing += 1;
                        i += 1;
                    }
                    if closing == tick_count {
                        let full: String = chars[start..i].iter().collect();
                        if full.contains("{{") || full.contains("{%") || full.contains("{#") {
                            let idx = blocks.len();
                            blocks.push(full);
                            result.push_str(&format!("{}{}{}", CODE_PLACEHOLDER_PREFIX, idx, CODE_PLACEHOLDER_SUFFIX));
                        } else {
                            result.push_str(&full);
                        }
                        found_end = true;
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            if !found_end {
                let unmatched: String = chars[start..i].iter().collect();
                result.push_str(&unmatched);
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

/// Detect a fenced code block opening (3+ backticks or tildes).
pub(crate) fn detect_fence_open(trimmed: &str) -> Option<String> {
    let ch = trimmed.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let count = trimmed.chars().take_while(|&c| c == ch).count();
    if count >= 3 {
        Some(std::iter::repeat(ch).take(count).collect())
    } else {
        None
    }
}

/// Restore protected code blocks from placeholders.
pub(crate) fn restore_code_blocks(text: &str, blocks: &[String]) -> String {
    if blocks.is_empty() || !text.contains(CODE_PLACEHOLDER_PREFIX) {
        return text.to_string();
    }
    static RE_PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(&format!(
            "{}(\\d+){}",
            regex::escape(CODE_PLACEHOLDER_PREFIX),
            regex::escape(CODE_PLACEHOLDER_SUFFIX)
        )).unwrap()
    });
    RE_PLACEHOLDER.replace_all(text, |caps: &regex::Captures| {
        let idx: usize = caps[1].parse().unwrap_or(usize::MAX);
        blocks.get(idx).cloned().unwrap_or_default()
    }).to_string()
}
