use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::typst::crossref::has_crossref_prefix;
use crate::typst::io::write_if_changed;
use crate::typst::model::LayoutPaths;
const RUNTIME_IMPORT: &str = "/.calepin/calepin.typ";
const PREVIEW_IMPORT_PREFIX: &str = "@preview/calepin:";

pub fn write_staged_source(layout: &LayoutPaths) -> Result<PathBuf> {
    let staged_relative = layout.artifact_relative_path("source.typ");

    let source = std::fs::read_to_string(&layout.input)
        .with_context(|| format!("failed to read {}", layout.input.display()))?;
    reject_preview_calepin_imports(&source)?;
    let staged = rewrite_calepin_imports(&source);
    let staged_path = layout.root.join(&staged_relative);

    write_if_changed(&staged_path, staged)?;
    Ok(staged_relative)
}

fn reject_preview_calepin_imports(source: &str) -> Result<()> {
    if let Some(suggestion) = preview_calepin_import_suggestion(source) {
        return Err(anyhow::anyhow!(
            "unsupported Calepin Typst package import. Calepin documents must import the binary-written local runtime instead:\n{suggestion}\nRun `calepin compile` or `calepin watch` so Calepin writes .calepin/calepin.typ before Typst renders the document."
        ));
    }
    Ok(())
}

fn preview_calepin_import_suggestion(source: &str) -> Option<String> {
    let mut raw_block: Option<usize> = None;

    for segment in source.split_inclusive('\n') {
        let (line, _) = segment
            .strip_suffix('\n')
            .map(|line| (line, "\n"))
            .unwrap_or((segment, ""));
        let trimmed = line.trim_start();

        if let Some(fence_len) = raw_block {
            if is_closing_fence(trimmed, fence_len) {
                raw_block = None;
            }
            continue;
        }

        if let Some((fence_len, _)) = opening_fence(trimmed) {
            raw_block = Some(fence_len);
            continue;
        }

        if let Some(suggestion) = preview_calepin_import_suggestion_in_line(line) {
            return Some(suggestion);
        }
    }

    None
}

fn preview_calepin_import_suggestion_in_line(line: &str) -> Option<String> {
    let mut rest = line;

    while let Some(index) = rest.find("#import") {
        let (before, candidate) = rest.split_at(index);
        if before.contains("//") {
            return None;
        }

        if !import_keyword_boundary(candidate) {
            rest = &candidate["#import".len()..];
            continue;
        }

        let Some((suggestion, _tail)) = preview_import_candidate_suggestion(candidate) else {
            rest = &candidate["#import".len()..];
            continue;
        };
        return Some(suggestion);
    }

    None
}

fn preview_import_candidate_suggestion(candidate: &str) -> Option<(String, &str)> {
    let after_keyword = &candidate["#import".len()..];
    let whitespace_len = after_keyword
        .char_indices()
        .find_map(|(idx, ch)| if ch.is_whitespace() { None } else { Some(idx) })
        .unwrap_or(after_keyword.len());
    let whitespace = &after_keyword[..whitespace_len];
    let after_whitespace = &after_keyword[whitespace_len..];
    let literal = parse_string_literal(after_whitespace)?;
    if !literal.value.starts_with(PREVIEW_IMPORT_PREFIX) {
        return None;
    }

    let tail = &after_whitespace[literal.source_len..];
    Some((
        format!("#import{}\"{}\"{}", whitespace, RUNTIME_IMPORT, tail),
        tail,
    ))
}

fn rewrite_calepin_imports(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut raw_block: Option<RawBlock> = None;

    for segment in source.split_inclusive('\n') {
        let (line, newline) = segment
            .strip_suffix('\n')
            .map(|line| (line, "\n"))
            .unwrap_or((segment, ""));
        let trimmed = line.trim_start();

        if let Some(block) = raw_block.as_mut() {
            block.segments.push(segment.to_string());
            if is_closing_fence(trimmed, block.fence_len) {
                let block = raw_block.take().expect("raw block exists");
                out.push_str(&rewrite_raw_block(block));
            }
            continue;
        }

        if let Some((fence_len, lang)) = opening_fence(trimmed) {
            raw_block = Some(RawBlock {
                fence_len,
                lang: lang.map(str::to_string),
                segments: vec![segment.to_string()],
            });
        } else {
            out.push_str(&rewrite_calepin_imports_in_line(line));
            out.push_str(newline);
        }
    }

    if let Some(block) = raw_block {
        out.push_str(&block.segments.concat());
    }
    out
}

struct RawBlock {
    fence_len: usize,
    lang: Option<String>,
    segments: Vec<String>,
}

fn opening_fence(trimmed_line: &str) -> Option<(usize, Option<&str>)> {
    let fence_len = leading_backtick_count(trimmed_line);
    if fence_len < 3 {
        return None;
    }
    let rest = trimmed_line[fence_len..].trim_start();
    let lang = if rest.is_empty() {
        None
    } else {
        rest.split_whitespace().next()
    };
    Some((fence_len, lang))
}

fn is_closing_fence(trimmed_line: &str, fence_len: usize) -> bool {
    let closing_len = leading_backtick_count(trimmed_line);
    if closing_len < fence_len {
        return false;
    }
    let rest = trimmed_line[closing_len..].trim_start();
    rest.is_empty() || rest.starts_with('<')
}

fn leading_backtick_count(value: &str) -> usize {
    value.chars().take_while(|ch| *ch == '`').count()
}

fn rewrite_raw_block(mut block: RawBlock) -> String {
    if !is_executable_label_candidate_lang(block.lang.as_deref()) {
        return block.segments.concat();
    }
    let Some(last) = block.segments.last() else {
        return String::new();
    };
    let (line, newline) = split_segment(last);
    let Some((prefix, label)) = trailing_fence_label(line) else {
        return block.segments.concat();
    };
    if !is_routed_crossref_label(label) {
        return block.segments.concat();
    }
    let label = label.to_string();

    let closing = format!(
        "{}{}{}",
        prefix,
        line_suffix_after_trimmed_end(line),
        newline
    );
    let last_index = block.segments.len() - 1;
    block.segments[last_index] = closing;

    let mut out = String::with_capacity(block.segments.concat().len() + label.len() + 16);
    out.push_str(&block.segments[0]);
    out.push_str("#| label: ");
    out.push_str(&qmd_string_literal(&label));
    out.push('\n');
    for segment in block.segments.iter().skip(1) {
        out.push_str(segment);
    }
    out
}

fn split_segment(segment: &str) -> (&str, &str) {
    segment
        .strip_suffix('\n')
        .map(|line| (line, "\n"))
        .unwrap_or((segment, ""))
}

fn qmd_string_literal(value: &str) -> String {
    format!("\"{}\"", typst_string_escape(value))
}

fn trailing_fence_label(line: &str) -> Option<(&str, &str)> {
    let trimmed_end = line.trim_end();
    if !trimmed_end.ends_with('>') {
        return None;
    }
    let label_start = trimmed_end.rfind('<')?;
    let label = &trimmed_end[label_start + 1..trimmed_end.len() - 1];
    let before_label = &trimmed_end[..label_start];
    let fence = before_label.trim();
    if fence.len() < 3 || !fence.chars().all(|ch| ch == '`') {
        return None;
    }
    if label.is_empty() {
        return None;
    }
    let prefix = &line[..label_start];
    Some((prefix, label))
}

fn line_suffix_after_trimmed_end(line: &str) -> &str {
    let trimmed_len = line.trim_end().len();
    &line[trimmed_len..]
}

fn is_executable_label_candidate_lang(raw_lang: Option<&str>) -> bool {
    !matches!(raw_lang, None | Some("typ" | "typst"))
}

fn is_routed_crossref_label(label: &str) -> bool {
    has_crossref_prefix(label)
}

fn typst_string_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
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
    value == ".calepin/calepin.typ" || value == RUNTIME_IMPORT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_preview_import_path_for_migration_diagnostic() {
        let source = r#"#import "@preview/calepin:0.0.1" as cp
#import "@preview/calepin:9.8.7": chunk, inline
#import "@preview/other:1.0.0" as other
"#;
        let rewritten = rewrite_calepin_imports(source);
        assert_eq!(
            rewritten,
            r#"#import "@preview/calepin:0.0.1" as cp
#import "@preview/calepin:9.8.7": chunk, inline
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
    fn does_not_rewrite_comments_raw_blocks_or_preview_imports() {
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
#import "@preview/calepin:0.0.1" as calepin
"#
        );
    }

    #[test]
    fn rewrites_routed_executable_fence_label_to_qmd_header() {
        let source = "```r\nplot(1)\n```<fig-plot>\n";
        let rewritten = rewrite_calepin_imports(source);
        assert_eq!(rewritten, "```r\n#| label: \"fig-plot\"\nplot(1)\n```\n");
    }

    #[test]
    fn leaves_unrouted_and_typst_fence_labels_for_strict_query_validation() {
        let source = "```r\nplot(1)\n```<plot>\n```typ\n#strong[x]\n```<fig-typ>\n";
        let rewritten = rewrite_calepin_imports(source);
        assert_eq!(rewritten, source);
    }

    #[test]
    fn does_not_rewrite_nested_fences_inside_typst_examples() {
        let source = "````typ\n```r\nplot(1)\n```<fig-example>\n````\n";
        let rewritten = rewrite_calepin_imports(source);
        assert_eq!(rewritten, source);
    }
}
