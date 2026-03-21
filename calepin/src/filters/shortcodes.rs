// Legacy shortcode support: pre-parse include expansion and marker resolution.
//
// Shortcodes have been replaced by Tera body processing (see tera_engine.rs).
// This module retains:
// - expand_includes()       — Pre-parse `{{< include file >}}` expansion
// - resolve_shortcode_raw() — Resolve shortcode markers in rendered output
// - VARIABLES               — Cached _variables.yml (used by tera_engine.rs)

use std::sync::LazyLock;

use regex::Regex;

use crate::render::markers;

/// Expand `{% include "file" %}` directives before block parsing.
/// This must run on the raw body text so that included content gets
/// parsed as blocks (code chunks, divs, etc.) rather than inline text.
pub fn expand_includes(text: &str) -> String {
    // {% include "file" %} or {% include 'file' %}
    static INCLUDE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"\{%[-\s]\s*include\s+["'](.+?)["']\s*[-\s]?%\}"#).unwrap()
    });
    // {% raw %} ... {% endraw %} blocks (protect from include expansion)
    static RAW_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{%-?\s*raw\s*-?%\}[\s\S]*?\{%-?\s*endraw\s*-?%\}").unwrap()
    });

    // Protect {% raw %} blocks from include expansion
    let mut raw_blocks = Vec::new();
    let text = RAW_RE.replace_all(text, |caps: &regex::Captures| {
        let idx = raw_blocks.len();
        raw_blocks.push(caps[0].to_string());
        format!("\u{FDD2}RAW{}\u{FDD3}", idx)
    }).to_string();

    // Expand includes
    let text = INCLUDE_RE.replace_all(&text, |caps: &regex::Captures| {
        let path = caps[1].trim();
        include_file(path)
    }).to_string();

    // Restore {% raw %} blocks
    let mut result = text;
    for (idx, block) in raw_blocks.iter().enumerate() {
        result = result.replace(&format!("\u{FDD2}RAW{}\u{FDD3}", idx), block);
    }
    result
}

/// Read and include a file, stripping YAML front matter if present.
fn include_file(path: &str) -> String {
    if path.is_empty() {
        cwarn!("{{{{< include >}}}} requires a file path");
        return String::new();
    }
    match std::fs::read_to_string(path) {
        Ok(content) => {
            // Strip YAML front matter if present
            if content.starts_with("---") {
                if let Some(end) = content[3..].find("\n---") {
                    let after = end + 3 + 4; // skip past closing ---
                    return content[after..].to_string();
                }
            }
            content
        }
        Err(e) => {
            cwarn!("{{{{< include {} >}}}}: {}", path, e);
            String::new()
        }
    }
}

/// Resolve shortcode raw markers in rendered text, restoring the protected content.
pub fn resolve_shortcode_raw(text: &str, fragments: &[String]) -> String {
    markers::resolve_shortcode_raw(text, fragments)
}

/// Cached _variables.yml content, loaded once per process.
/// Used by tera_engine.rs for the `var` context variable.
pub static VARIABLES: LazyLock<Option<saphyr::YamlOwned>> = LazyLock::new(|| {
    use saphyr::LoadableYamlNode;
    let content = std::fs::read_to_string("_variables.yml")
        .or_else(|_| std::fs::read_to_string("_variables.yaml"))
        .unwrap_or_default();
    if content.is_empty() {
        return None;
    }
    match saphyr::YamlOwned::load_from_str(&content) {
        Ok(docs) => docs.into_iter().next(),
        Err(e) => {
            cwarn!("failed to parse _variables.yml: {}", e);
            None
        }
    }
});
