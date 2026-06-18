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
    let staged = if staged.contains("#calepin_runtime.chunk_from_raw_plain(") {
        format!("#import \"{RUNTIME_IMPORT}\" as calepin_runtime\n{staged}")
    } else {
        staged
    };
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
    let mut parse_state = TypstParseState::default();

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
                in_calepin_chunk: parse_state.in_calepin_chunk(),
            });
        } else {
            let rewritten = rewrite_calepin_imports_in_line(line);
            parse_state.scan_line(&rewritten);
            out.push_str(&rewritten);
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
    in_calepin_chunk: bool,
}

#[derive(Default)]
struct TypstParseState {
    brackets: Vec<BracketContext>,
    paren_depth: usize,
    pending_chunk_call: Option<PendingChunkCall>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BracketContext {
    Plain,
    CalepinChunk,
}

#[derive(Clone, Copy)]
struct PendingChunkCall {
    target_paren_depth: usize,
    state: PendingChunkCallState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingChunkCallState {
    AwaitingArgsOrBody,
    InArgs,
    ReadyForBody,
}

impl TypstParseState {
    fn in_calepin_chunk(&self) -> bool {
        self.brackets
            .iter()
            .any(|context| *context == BracketContext::CalepinChunk)
    }

    fn scan_line(&mut self, line: &str) {
        let mut chars = line.char_indices().peekable();
        while let Some((idx, ch)) = chars.next() {
            if ch == '/' && chars.peek().is_some_and(|(_, next)| *next == '/') {
                break;
            }

            if ch == '"' {
                let mut escaped = false;
                for (_, inner) in chars.by_ref() {
                    if escaped {
                        escaped = false;
                    } else if inner == '\\' {
                        escaped = true;
                    } else if inner == '"' {
                        break;
                    }
                }
                continue;
            }

            if chunk_call_match_len(&line[idx..]).is_some() {
                self.pending_chunk_call = Some(PendingChunkCall {
                    target_paren_depth: self.paren_depth,
                    state: PendingChunkCallState::AwaitingArgsOrBody,
                });
            }

            match ch {
                '(' => {
                    if let Some(pending) = self.pending_chunk_call.as_mut() {
                        if pending.state == PendingChunkCallState::AwaitingArgsOrBody
                            && self.paren_depth == pending.target_paren_depth
                        {
                            pending.state = PendingChunkCallState::InArgs;
                        }
                    }
                    self.paren_depth += 1;
                }
                ')' => {
                    self.paren_depth = self.paren_depth.saturating_sub(1);
                    if let Some(pending) = self.pending_chunk_call.as_mut() {
                        if pending.state == PendingChunkCallState::InArgs
                            && self.paren_depth == pending.target_paren_depth
                        {
                            pending.state = PendingChunkCallState::ReadyForBody;
                        }
                    }
                }
                '[' => {
                    let is_chunk_body = self.pending_chunk_call.is_some_and(|pending| {
                        pending.state == PendingChunkCallState::AwaitingArgsOrBody
                            || pending.state == PendingChunkCallState::ReadyForBody
                    });
                    self.brackets.push(if is_chunk_body {
                        self.pending_chunk_call = None;
                        BracketContext::CalepinChunk
                    } else {
                        BracketContext::Plain
                    });
                }
                ']' => {
                    self.brackets.pop();
                }
                _ if !ch.is_whitespace()
                    && self.pending_chunk_call.is_some_and(|pending| {
                        pending.state == PendingChunkCallState::ReadyForBody
                    }) =>
                {
                    self.pending_chunk_call = None;
                }
                _ => {}
            }
        }
    }
}

fn chunk_call_match_len(input: &str) -> Option<usize> {
    let rest = input.strip_prefix('#')?;
    let first_len = identifier_len(rest);
    if first_len == 0 {
        return None;
    }
    let mut consumed = 1 + first_len;
    let mut tail = &rest[first_len..];
    let mut last = &rest[..first_len];
    while let Some(after_dot) = tail.strip_prefix('.') {
        let part_len = identifier_len(after_dot);
        if part_len == 0 {
            return None;
        }
        consumed += 1 + part_len;
        last = &after_dot[..part_len];
        tail = &after_dot[part_len..];
    }
    if last != "chunk" || tail.starts_with('.') || tail.chars().next().is_some_and(is_ident_char) {
        return None;
    }
    Some(consumed)
}

fn identifier_len(input: &str) -> usize {
    input
        .char_indices()
        .take_while(|(_, ch)| is_ident_char(*ch))
        .last()
        .map(|(idx, ch)| idx + ch.len_utf8())
        .unwrap_or(0)
}

fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
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
    let should_rewrite_as_chunk =
        !block.in_calepin_chunk && is_source_rewritten_chunk_lang(block.lang.as_deref());
    if !should_rewrite_as_chunk && !is_executable_label_candidate_lang(block.lang.as_deref()) {
        return block.segments.concat();
    }
    let had_trailing_label = rewrite_trailing_fence_label(&mut block, should_rewrite_as_chunk);
    if should_rewrite_as_chunk {
        return rewrite_raw_block_as_chunk_from_raw_plain(&block, had_trailing_label);
    }
    block.segments.concat()
}

fn rewrite_trailing_fence_label(block: &mut RawBlock, rewrite_plain_labels: bool) -> bool {
    let Some(last) = block.segments.last() else {
        return false;
    };
    let (line, newline) = split_segment(last);
    let Some((prefix, label)) = trailing_fence_label(line) else {
        return false;
    };
    if !rewrite_plain_labels && !is_routed_crossref_label(label) {
        return false;
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

    block
        .segments
        .insert(1, format!("#| label: {}\n", qmd_string_literal(&label)));
    true
}

fn rewrite_raw_block_as_chunk_from_raw_plain(block: &RawBlock, had_trailing_label: bool) -> String {
    let Some(lang) = block.lang.as_deref() else {
        return block.segments.concat();
    };
    let code = if block.segments.len() > 2 {
        block.segments[1..block.segments.len() - 1].concat()
    } else {
        String::new()
    };
    let label_metadata = if had_trailing_label {
        String::new()
    } else {
        trailing_label_metadata(&block.segments)
            .map(|label| {
                format!(
                    " #metadata((label: {})) <calepin-fence-label>",
                    qmd_string_literal(label)
                )
            })
            .unwrap_or_default()
    };
    format!(
        "#calepin_runtime.chunk_from_raw_plain({}, raw({}, block: true, lang: {})){}\n",
        qmd_string_literal(lang),
        qmd_string_literal(&code),
        qmd_string_literal(lang),
        label_metadata
    )
}

fn trailing_label_metadata(segments: &[String]) -> Option<&str> {
    let last = segments.last()?;
    let (line, _) = split_segment(last);
    let (_, label) = trailing_fence_label(line)?;
    Some(label)
}

fn is_source_rewritten_chunk_lang(raw_lang: Option<&str>) -> bool {
    let Some(lang) = raw_lang else {
        return false;
    };
    if matches!(lang, "typ" | "typst") {
        return false;
    }
    matches!(lang, "python" | "r") || crate::engines::diagram::is_known_diagram_engine_name(lang)
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
        assert_eq!(
            rewritten,
            "#calepin_runtime.chunk_from_raw_plain(\"r\", raw(\"#| label: \\\"fig-plot\\\"\\nplot(1)\\n\", block: true, lang: \"r\"))\n"
        );
    }

    #[test]
    fn leaves_unrouted_and_typst_fence_labels_for_strict_query_validation() {
        let source = "```r\nplot(1)\n```<plot>\n```typ\n#strong[x]\n```<fig-typ>\n";
        let rewritten = rewrite_calepin_imports(source);
        assert_eq!(
            rewritten,
            "#calepin_runtime.chunk_from_raw_plain(\"r\", raw(\"#| label: \\\"plot\\\"\\nplot(1)\\n\", block: true, lang: \"r\"))\n```typ\n#strong[x]\n```<fig-typ>\n"
        );
    }

    #[test]
    fn rewrites_bare_executable_fences_to_chunk_calls() {
        let source = "Before\n```python\nprint(\"x\")\n```\nAfter\n";
        let rewritten = rewrite_calepin_imports(source);
        assert_eq!(
            rewritten,
            "Before\n#calepin_runtime.chunk_from_raw_plain(\"python\", raw(\"print(\\\"x\\\")\\n\", block: true, lang: \"python\"))\nAfter\n"
        );
    }

    #[test]
    fn does_not_wrap_raw_blocks_inside_explicit_chunks() {
        let source = "#calepin.chunk(\"python\")[\n```python\nprint(\"x\")\n```\n]\n";
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
