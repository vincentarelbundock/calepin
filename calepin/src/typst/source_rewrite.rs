use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::typst::model::LayoutPaths;
const RUNTIME_IMPORT: &str = "/.calepin/calepin.typ";

pub fn write_staged_source(layout: &LayoutPaths) -> Result<PathBuf> {
    let mut staged_relative = PathBuf::from(".calepin");
    let mut stem = layout.input_rel.clone();
    stem.set_extension("");
    staged_relative.push(stem);
    staged_relative.push("source.typ");

    let source = std::fs::read_to_string(&layout.input)
        .with_context(|| format!("failed to read {}", layout.input.display()))?;
    let staged = rewrite_calepin_imports(&source);
    let staged_path = layout.root.join(&staged_relative);

    if let Some(parent) = staged_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    if std::fs::read_to_string(&staged_path).is_ok_and(|existing| existing == staged) {
        return Ok(staged_relative);
    }
    std::fs::write(&staged_path, staged)
        .with_context(|| format!("failed to write {}", staged_path.display()))?;
    Ok(staged_relative)
}

fn rewrite_calepin_imports(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut in_raw_block = false;

    for segment in source.split_inclusive('\n') {
        let (line, newline) = segment
            .strip_suffix('\n')
            .map(|line| (line, "\n"))
            .unwrap_or((segment, ""));
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_raw_block = !in_raw_block;
            out.push_str(line);
            out.push_str(newline);
            continue;
        }
        if in_raw_block {
            out.push_str(line);
        } else {
            out.push_str(&rewrite_calepin_imports_in_line(line));
        }
        out.push_str(newline);
    }

    out
}

fn rewrite_calepin_imports_in_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;

    while let Some(index) = rest.find("#import") {
        let (before, candidate) = rest.split_at(index);
        if before.contains("//") {
            out.push_str(rest);
            return out;
        }
        out.push_str(before);

        if !import_keyword_boundary(candidate) {
            out.push_str("#import");
            rest = &candidate["#import".len()..];
            continue;
        }

        let Some((rewritten, tail)) = rewrite_import_candidate(candidate) else {
            out.push_str("#import");
            rest = &candidate["#import".len()..];
            continue;
        };
        out.push_str(&rewritten);
        rest = tail;
    }

    out.push_str(rest);
    out
}

fn import_keyword_boundary(candidate: &str) -> bool {
    candidate["#import".len()..]
        .chars()
        .next()
        .is_none_or(|ch| ch.is_whitespace() || ch == '"')
}

fn rewrite_import_candidate(candidate: &str) -> Option<(String, &str)> {
    let after_keyword = &candidate["#import".len()..];
    let whitespace_len = after_keyword
        .char_indices()
        .find_map(|(idx, ch)| if ch.is_whitespace() { None } else { Some(idx) })
        .unwrap_or(after_keyword.len());
    let whitespace = &after_keyword[..whitespace_len];
    let after_whitespace = &after_keyword[whitespace_len..];
    let literal = parse_string_literal(after_whitespace)?;
    if !is_calepin_runtime_import(&literal.value) {
        return None;
    }

    let tail = &after_whitespace[literal.source_len..];
    Some((format!("#import{}\"{}\"", whitespace, RUNTIME_IMPORT), tail))
}

struct StringLiteral {
    value: String,
    source_len: usize,
}

fn parse_string_literal(input: &str) -> Option<StringLiteral> {
    if !input.starts_with('"') {
        return None;
    }
    let mut escaped = false;
    let mut value = String::new();
    for (idx, ch) in input[1..].char_indices() {
        if escaped {
            value.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            return Some(StringLiteral {
                value,
                source_len: idx + 2,
            });
        }
        value.push(ch);
    }
    None
}

fn is_calepin_runtime_import(value: &str) -> bool {
    value == ".calepin/calepin.typ"
        || value == RUNTIME_IMPORT
        || value.starts_with("@preview/calepin:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_preview_import_path_and_preserves_style() {
        let source = r#"#import "@preview/calepin:0.0.1" as cp
#import "@preview/calepin:9.8.7": chunk, inline
#import "@preview/other:1.0.0" as other
"#;
        let rewritten = rewrite_calepin_imports(source);
        assert_eq!(
            rewritten,
            r#"#import "/.calepin/calepin.typ" as cp
#import "/.calepin/calepin.typ": chunk, inline
#import "@preview/other:1.0.0" as other
"#
        );
    }

    #[test]
    fn rewrites_legacy_relative_import() {
        assert_eq!(
            rewrite_calepin_imports(r#"#import ".calepin/calepin.typ""#),
            r#"#import "/.calepin/calepin.typ""#
        );
    }

    #[test]
    fn does_not_rewrite_comments_or_raw_blocks() {
        let source = r#"// #import "@preview/calepin:0.0.1"
```typ
#import "@preview/calepin:0.0.1"
```
#import "@preview/calepin:0.0.1" as calepin
"#;
        let rewritten = rewrite_calepin_imports(source);
        assert_eq!(
            rewritten,
            r#"// #import "@preview/calepin:0.0.1"
```typ
#import "@preview/calepin:0.0.1"
```
#import "/.calepin/calepin.typ" as calepin
"#
        );
    }
}
