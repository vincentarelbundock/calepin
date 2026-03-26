//! Pre-parse `{% include "file" %}` expansion.
//!
//! Runs before block parsing so that included content gets parsed as blocks
//! (code chunks, divs, etc.) rather than inline text.
//!
//! When an include path has no file extension, format-aware resolution is used
//! (target-specific -> engine-specific -> common), looking in `_calepin/partials/`.

use std::sync::LazyLock;
use regex::Regex;

use super::protection::{protect_code_blocks, restore_code_blocks};

/// Expand `{% include "file" %}` directives before block parsing.
/// Paths are resolved relative to `project_root`.
#[inline(never)]
pub fn expand_includes(text: &str, project_root: &std::path::Path, format: &str) -> String {
    // {% include "file" %} or {% include 'file' %}
    static INCLUDE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"\{%[-\s]\s*include\s+["'](.+?)["']\s*[-\s]?%\}"#).unwrap()
    });
    // {% raw %} ... {% endraw %} blocks (protect from include expansion)
    static RAW_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{%-?\s*raw\s*-?%\}[\s\S]*?\{%-?\s*endraw\s*-?%\}").unwrap()
    });

    // Protect fenced code blocks from include expansion
    let (text, code_blocks) = protect_code_blocks(text);

    // Protect {% raw %} blocks from include expansion
    let mut raw_blocks = Vec::new();
    let text = RAW_RE.replace_all(&text, |caps: &regex::Captures| {
        let idx = raw_blocks.len();
        raw_blocks.push(caps[0].to_string());
        format!("\u{FDD2}RAW{}\u{FDD3}", idx)
    }).to_string();

    // Expand includes (resolve relative to project root).
    // When the path has no extension, use format-aware resolution via
    // `resolve_include()` (target -> engine -> common under `_calepin/partials/`).
    let text = INCLUDE_RE.replace_all(&text, |caps: &regex::Captures| {
        let path = caps[1].trim();
        if std::path::Path::new(path).extension().is_some() {
            // Explicit extension: resolve relative to project root
            let resolved = project_root.join(path);
            include_file(&resolved.to_string_lossy())
        } else {
            // No extension: format-aware resolution
            match crate::paths::resolve_include(path, format) {
                Some(resolved) => match std::fs::read_to_string(&resolved) {
                    Ok(content) => content,
                    Err(e) => {
                        cwarn!("include '{}': {}", path, e);
                        String::new()
                    }
                },
                None => {
                    cwarn!("include '{}' not found", path);
                    String::new()
                }
            }
        }
    }).to_string();

    // Restore {% raw %} blocks
    let mut result = text;
    for (idx, block) in raw_blocks.iter().enumerate() {
        result = result.replace(&format!("\u{FDD2}RAW{}\u{FDD3}", idx), block);
    }

    // Restore fenced code blocks
    restore_code_blocks(&result, &code_blocks)
}

/// Read and include a file, stripping YAML front matter if present.
fn include_file(path: &str) -> String {
    if path.is_empty() {
        // Return error marker that will surface later
        return "\n\n**Error: `{% include %}` requires a file path**\n\n".to_string();
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
            // Surface as visible error in the rendered output
            format!("\n\n**Error: `{{%% include \"{}\" %}}`: {}**\n\n", path, e)
        }
    }
}
