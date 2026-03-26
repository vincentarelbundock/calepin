//! Lorem ipsum span module: `[]{.lorem paragraphs=3}` -> placeholder text.

use std::collections::HashMap;

pub fn render(
    kv: &HashMap<String, String>,
    defaults: &crate::config::Metadata,
) -> String {
    let default_paragraphs = defaults.lipsum.as_ref()
        .and_then(|l| l.paragraphs)
        .unwrap_or(1) as usize;

    if let Some(n) = kv.get("words").and_then(|s| s.parse::<usize>().ok()) {
        return crate::jinja::lipsum_words(n);
    }
    if let Some(n) = kv.get("sentences").and_then(|s| s.parse::<usize>().ok()) {
        return crate::jinja::lipsum::lipsum_sentences(n);
    }
    let n = kv.get("paragraphs")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(default_paragraphs);
    crate::jinja::lipsum_paragraphs(n)
}
