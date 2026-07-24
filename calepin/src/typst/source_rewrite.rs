use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::PathBuf;

use crate::typst::crossref::has_crossref_prefix;
use crate::typst::fence_label::{
    trailing_fence_label, trailing_fence_label_candidate, FENCE_LABEL_METADATA_LABEL,
};
use crate::typst::io::write_if_changed;
use crate::typst::markdown_fence::{is_closing_fence, leading_backtick_count};
use crate::typst::model::LayoutPaths;
const DEFAULT_RUNTIME_IMPORT: &str = "/.calepin/calepin.typ";
const RUNTIME_IMPORT_LEGACY: &str = "_calepin/calepin.typ";
const RUNTIME_ALIAS: &str = "calepin_runtime";
const RUNTIME_DEFAULT_ALIAS: &str = "calepin";
const PREVIEW_IMPORT_PREFIX: &str = "@preview/calepin:";
const SOURCE_REWRITTEN_CHUNK_LANGS: &[&str] = &["python", "r", "julia", "sh", "bash"];

pub fn write_staged_source(layout: &LayoutPaths, runtime_import: &str) -> Result<PathBuf> {
    let staged_relative = layout.artifact_relative_path("source.typ");

    let source = std::fs::read_to_string(&layout.input)
        .with_context(|| format!("failed to read {}", layout.input.display()))?;
    reject_preview_calepin_imports(&source)?;
    let staged = stage_user_source(&source, runtime_import);
    let staged_path = layout.root.join(&staged_relative);

    write_if_changed(&staged_path, staged)?;
    Ok(staged_relative)
}

fn reject_preview_calepin_imports(source: &str) -> Result<()> {
    if let Some(suggestion) = preview_calepin_import_suggestion(source) {
        return Err(anyhow::anyhow!(
            "unsupported Calepin Typst package import. Calepin documents must import the binary-written local runtime instead:\n{suggestion}\nRun `calepin compile` or `calepin watch` so Calepin writes its local runtime before Typst renders the document."
        ));
    }
    Ok(())
}

fn preview_calepin_import_suggestion(source: &str) -> Option<String> {
    let mut raw_block: Option<usize> = None;
    let mut lex = LexState::default();

    for segment in source.split_inclusive('\n') {
        let (line, _) = split_segment(segment);
        let trimmed = line.trim_start();

        if let Some(fence_len) = raw_block {
            if is_closing_fence(trimmed, fence_len) {
                raw_block = None;
            }
            continue;
        }

        if !lex.in_block_comment() {
            if let Some((fence_len, _)) = opening_fence(trimmed) {
                raw_block = Some(fence_len);
                continue;
            }
        }

        if let Some(suggestion) = preview_calepin_import_suggestion_in_line(line, &mut lex) {
            return Some(suggestion);
        }
    }

    None
}

fn preview_calepin_import_suggestion_in_line(line: &str, lex: &mut LexState) -> Option<String> {
    scan_line_for_imports(line, lex, |candidate| {
        preview_import_candidate_suggestion(candidate).map(|(suggestion, _)| suggestion)
    })
}

fn preview_import_candidate_suggestion(candidate: &str) -> Option<(String, &str)> {
    let import = parse_import_candidate(candidate)?;
    if !import.literal.value.starts_with(PREVIEW_IMPORT_PREFIX) {
        return None;
    }

    Some((
        format!(
            "#import{}\"{}\"{}",
            import.whitespace, DEFAULT_RUNTIME_IMPORT, import.tail
        ),
        import.tail,
    ))
}

pub(crate) fn rewrite_runtime_imports(source: &str, runtime_import: &str) -> String {
    let rewritten = rewrite_source(source, runtime_import);
    if rewritten.needs_runtime_alias {
        format!(
            "{}{}",
            runtime_alias_import(runtime_import),
            rewritten.source
        )
    } else {
        rewritten.source
    }
}

/// Rewrites a user document for staging. When the document never imports the
/// Calepin runtime itself, a default `as calepin` import is prepended so authors
/// can call `calepin.setup`/`calepin.chunk` without writing the import line. Any
/// user-written runtime import (any path spelling or alias) suppresses the
/// injection, so an author keeping their own `#import ... as calepin` is left
/// untouched rather than duplicated.
fn stage_user_source(source: &str, runtime_import: &str) -> String {
    let rewritten = rewrite_source(source, runtime_import);
    let mut prefix = String::new();
    if !rewritten.saw_runtime_import {
        prefix.push_str(&runtime_default_import(runtime_import));
    }
    if rewritten.needs_runtime_alias {
        prefix.push_str(&runtime_alias_import(runtime_import));
    }
    if prefix.is_empty() {
        rewritten.source
    } else {
        format!("{prefix}{}", rewritten.source)
    }
}

fn runtime_alias_import(runtime_import: &str) -> String {
    format!("#import \"{runtime_import}\" as {RUNTIME_ALIAS}\n")
}

fn runtime_default_import(runtime_import: &str) -> String {
    format!("#import \"{runtime_import}\" as {RUNTIME_DEFAULT_ALIAS}\n")
}

struct RewriteResult {
    source: String,
    needs_runtime_alias: bool,
    saw_runtime_import: bool,
}

#[cfg(test)]
fn rewrite_calepin_imports(source: &str) -> String {
    rewrite_source(source, DEFAULT_RUNTIME_IMPORT).source
}

fn rewrite_source(source: &str, runtime_import: &str) -> RewriteResult {
    let mut out = String::with_capacity(source.len());
    let mut raw_block: Option<RawBlock> = None;
    let mut parse_state = TypstParseState::default();
    let mut source_lex = LexState::default();
    let mut store_set_lex = StoreSetLexState::default();
    let store_bases = store_set_bases(source, runtime_import);
    let mut needs_runtime_alias = false;
    let mut saw_runtime_import = false;

    for segment in source.split_inclusive('\n') {
        let (line, newline) = split_segment(segment);
        let trimmed = line.trim_start();

        if let Some(block) = raw_block.as_mut() {
            block.segments.push(segment.to_string());
            let closes_block = is_closing_fence(trimmed, block.fence_len)
                || (!block.opening_prefix.is_empty()
                    && leading_backtick_count(trimmed) >= block.fence_len
                    && closing_fence_tail(line, block.fence_len)
                        .trim_start()
                        .starts_with(']'));
            if closes_block {
                let parser_tail = closing_fence_tail(line, block.fence_len).to_string();
                let block = raw_block.take().expect("raw block exists");
                let rewritten = rewrite_raw_block(block);
                needs_runtime_alias |= rewritten.needs_runtime_alias;
                out.push_str(&rewritten.source);
                if !parser_tail.trim().is_empty() {
                    let _ = parse_state.scan_line(parser_tail.trim_start());
                }
            }
            continue;
        }

        if parse_state.raw_fence_len.is_some() {
            let _ = parse_state.scan_line(line);
            out.push_str(segment);
            continue;
        }

        if !source_lex.in_block_comment() {
            if let Some((fence_len, lang)) = opening_fence(trimmed) {
                raw_block = Some(RawBlock {
                    fence_len,
                    lang: lang.map(str::to_string),
                    segments: vec![segment.to_string()],
                    in_calepin_chunk: parse_state.in_calepin_chunk(),
                    opening_prefix: String::new(),
                });
                continue;
            }
        }

        let rewritten = rewrite_calepin_imports_in_line(
            line,
            &mut source_lex,
            runtime_import,
            &mut saw_runtime_import,
        );
        let rewritten =
            rewrite_store_set_calls_in_line(&rewritten, &mut store_set_lex, &store_bases);
        let inline_fence = parse_state.scan_line(&rewritten);
        if let Some(offset) = inline_fence {
            let opening = &rewritten[offset..];
            if let Some((fence_len, Some(lang))) = opening_fence(opening) {
                if parse_state.in_calepin_chunk() && lang.contains('.') {
                    parse_state.raw_fence_len = None;
                    raw_block = Some(RawBlock {
                        fence_len,
                        lang: Some(lang.to_string()),
                        segments: vec![format!("{opening}{newline}")],
                        in_calepin_chunk: true,
                        opening_prefix: rewritten[..offset].to_string(),
                    });
                    continue;
                }
            }
        }
        out.push_str(&rewritten);
        out.push_str(newline);
    }

    if let Some(block) = raw_block {
        out.push_str(&block.original_source());
    }
    RewriteResult {
        source: out,
        needs_runtime_alias,
        saw_runtime_import,
    }
}

fn store_set_bases(source: &str, runtime_import: &str) -> HashSet<String> {
    let mut bases = HashSet::new();
    let mut saw_runtime_import = false;
    let mut raw_block = None;
    let mut lex = LexState::default();

    for segment in source.split_inclusive('\n') {
        let (line, _) = split_segment(segment);
        let trimmed = line.trim_start();
        if let Some(fence_len) = raw_block {
            if is_closing_fence(trimmed, fence_len) {
                raw_block = None;
            }
            continue;
        }
        if !lex.in_block_comment() {
            if let Some((fence_len, _)) = opening_fence(trimmed) {
                raw_block = Some(fence_len);
                continue;
            }
        }
        let _ = scan_line_for_imports(line, &mut lex, |candidate| {
            let import = parse_import_candidate(candidate)?;
            if !is_calepin_runtime_import(&import.literal.value, runtime_import) {
                return None;
            }
            saw_runtime_import = true;
            record_store_import_bases(import.tail, &mut bases);
            Some(())
        });
    }

    if !saw_runtime_import {
        bases.insert(format!("{RUNTIME_DEFAULT_ALIAS}.store"));
    }
    bases
}

fn record_store_import_bases(tail: &str, bases: &mut HashSet<String>) {
    let tail = tail.trim_start();
    if let Some(alias) = tail
        .strip_prefix("as")
        .filter(|rest| rest.starts_with(char::is_whitespace))
        .map(str::trim_start)
    {
        let len = identifier_len(alias);
        if len > 0 {
            bases.insert(format!("{}.store", &alias[..len]));
        }
        return;
    }
    let Some(imports) = tail.strip_prefix(':') else {
        bases.insert(format!("{RUNTIME_DEFAULT_ALIAS}.store"));
        return;
    };
    for import in imports.split(',') {
        let import = import.trim();
        if import.starts_with('*') {
            bases.insert("store".to_string());
            continue;
        }
        let name_len = identifier_len(import);
        if name_len == 0 || &import[..name_len] != "store" {
            continue;
        }
        let rest = import[name_len..].trim_start();
        if let Some(alias) = rest
            .strip_prefix("as")
            .filter(|tail| tail.starts_with(char::is_whitespace))
            .map(str::trim_start)
        {
            let alias_len = identifier_len(alias);
            if alias_len > 0 {
                bases.insert(alias[..alias_len].to_string());
                continue;
            }
        }
        bases.insert("store".to_string());
    }
}

fn rewrite_store_set_calls_in_line(
    line: &str,
    lex: &mut StoreSetLexState,
    bases: &HashSet<String>,
) -> String {
    let mut out = String::with_capacity(line.len());
    let mut idx = 0;

    while idx < line.len() {
        if let Some(delimiter_len) = lex.raw_delimiter_len {
            let delimiter = "`".repeat(delimiter_len);
            if line[idx..].starts_with(&delimiter) {
                out.push_str(&delimiter);
                idx += delimiter_len;
                lex.raw_delimiter_len = None;
            } else {
                let ch = line[idx..].chars().next().expect("valid char index");
                out.push(ch);
                idx += ch.len_utf8();
            }
            continue;
        }
        if lex.comments.in_block_comment() {
            if line[idx..].starts_with("/*") {
                lex.comments.enter_block_comment();
                out.push_str("/*");
                idx += 2;
            } else if line[idx..].starts_with("*/") {
                lex.comments.exit_block_comment();
                out.push_str("*/");
                idx += 2;
            } else {
                let ch = line[idx..].chars().next().expect("valid char index");
                out.push(ch);
                idx += ch.len_utf8();
            }
            continue;
        }
        if line[idx..].starts_with("//") {
            out.push_str(&line[idx..]);
            break;
        }
        if line[idx..].starts_with("/*") {
            lex.comments.enter_block_comment();
            out.push_str("/*");
            idx += 2;
            continue;
        }
        if line[idx..].starts_with('"') {
            let len = string_literal_source_len(&line[idx..]);
            out.push_str(&line[idx..idx + len]);
            idx += len;
            continue;
        }
        if line[idx..].starts_with('`') {
            let delimiter_len = leading_backtick_count(&line[idx..]);
            let delimiter = "`".repeat(delimiter_len);
            out.push_str(&delimiter);
            idx += delimiter_len;
            lex.raw_delimiter_len = Some(delimiter_len);
            continue;
        }

        let candidate = &line[idx..];
        if identifier_len(candidate) > 0 {
            let follows_call = candidate
                .split_once(".set")
                .is_some_and(|(_, tail)| tail.trim_start().starts_with('('));
            let has_boundary = idx == 0
                || line[..idx]
                    .chars()
                    .next_back()
                    .is_none_or(|ch| !is_ident_char(ch) && ch != '.');
            if follows_call && has_boundary {
                if let Some(path) = bases
                    .iter()
                    .map(|base| format!("{base}.set"))
                    .find(|path| candidate.starts_with(path))
                {
                    out.push_str(&path);
                    out.push('_');
                    idx += path.len();
                    continue;
                }
            }
        }

        let ch = candidate.chars().next().expect("valid char index");
        out.push(ch);
        idx += ch.len_utf8();
    }
    out
}

struct RawBlock {
    fence_len: usize,
    lang: Option<String>,
    segments: Vec<String>,
    in_calepin_chunk: bool,
    opening_prefix: String,
}

impl RawBlock {
    fn original_source(&self) -> String {
        format!("{}{}", self.opening_prefix, self.segments.concat())
    }
}

#[derive(Default)]
struct LexState {
    block_comment_depth: usize,
}

impl LexState {
    fn in_block_comment(&self) -> bool {
        self.block_comment_depth > 0
    }

    fn enter_block_comment(&mut self) {
        self.block_comment_depth += 1;
    }

    fn exit_block_comment(&mut self) {
        self.block_comment_depth = self.block_comment_depth.saturating_sub(1);
    }

    /// Advances `idx` past a line comment, block comment marker, or
    /// in-progress block comment. Returns `None` when `line[idx..]` is not a
    /// comment, leaving `idx` for the caller to interpret.
    fn skip_comment(&mut self, line: &str, idx: usize) -> Option<CommentStep> {
        if self.in_block_comment() {
            return Some(if line[idx..].starts_with("/*") {
                self.enter_block_comment();
                CommentStep::Continue(idx + 2)
            } else if line[idx..].starts_with("*/") {
                self.exit_block_comment();
                CommentStep::Continue(idx + 2)
            } else {
                CommentStep::Continue(idx + next_char_len(&line[idx..]))
            });
        }

        if line[idx..].starts_with("//") {
            return Some(CommentStep::Break);
        }
        if line[idx..].starts_with("/*") {
            self.enter_block_comment();
            return Some(CommentStep::Continue(idx + 2));
        }

        None
    }
}

enum CommentStep {
    Continue(usize),
    Break,
}

#[derive(Default)]
struct StoreSetLexState {
    comments: LexState,
    raw_delimiter_len: Option<usize>,
}

#[derive(Default)]
struct TypstParseState {
    brackets: Vec<BracketContext>,
    paren_depth: usize,
    pending_chunk_call: Option<PendingChunkCall>,
    raw_fence_len: Option<usize>,
    lex: LexState,
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
        self.brackets.contains(&BracketContext::CalepinChunk)
    }

    fn scan_line(&mut self, line: &str) -> Option<usize> {
        if let Some(fence_len) = self.raw_fence_len {
            let trimmed = line.trim_start();
            let closing_len = leading_backtick_count(trimmed);
            if closing_len >= fence_len {
                self.raw_fence_len = None;
                let tail = trimmed[closing_len..].trim_start();
                if !tail.is_empty() {
                    let _ = self.scan_line(tail);
                }
            }
            return None;
        }

        let mut idx = 0;
        while idx < line.len() {
            match self.lex.skip_comment(line, idx) {
                Some(CommentStep::Continue(new_idx)) => {
                    idx = new_idx;
                    continue;
                }
                Some(CommentStep::Break) => break,
                None => {}
            }

            let ch = line[idx..].chars().next().expect("valid char index");

            if ch == '`' {
                let fence_len = leading_backtick_count(&line[idx..]);
                if fence_len >= 3 {
                    self.raw_fence_len = Some(fence_len);
                    return Some(idx);
                }
            }

            if ch == '"' {
                idx += string_literal_source_len(&line[idx..]);
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
            idx += ch.len_utf8();
        }
        None
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

fn closing_fence_tail(line: &str, fence_len: usize) -> &str {
    let trimmed = line.trim_start();
    let closing_len = leading_backtick_count(trimmed);
    if closing_len < fence_len {
        return "";
    }
    &trimmed[closing_len..]
}

struct RewrittenRawBlock {
    source: String,
    needs_runtime_alias: bool,
}

fn rewrite_raw_block(mut block: RawBlock) -> RewrittenRawBlock {
    let dotted_lang = block.lang.as_deref().is_some_and(|lang| lang.contains('.'));
    let should_rewrite_as_chunk = !block.in_calepin_chunk
        && (dotted_lang || is_source_rewritten_chunk_lang(block.lang.as_deref()));
    let should_rewrite_as_raw = block.in_calepin_chunk && dotted_lang;
    if !should_rewrite_as_chunk
        && !should_rewrite_as_raw
        && !is_executable_label_candidate_lang(block.lang.as_deref())
    {
        return RewrittenRawBlock {
            source: block.original_source(),
            needs_runtime_alias: false,
        };
    }
    let had_trailing_label = rewrite_trailing_fence_label(&mut block, should_rewrite_as_chunk);
    if should_rewrite_as_chunk {
        return RewrittenRawBlock {
            source: rewrite_raw_block_as_chunk_from_raw_plain(&block, had_trailing_label),
            needs_runtime_alias: true,
        };
    }
    if should_rewrite_as_raw {
        return RewrittenRawBlock {
            source: rewrite_raw_block_as_raw_call(&block, had_trailing_label),
            needs_runtime_alias: false,
        };
    }
    RewrittenRawBlock {
        source: block.original_source(),
        needs_runtime_alias: false,
    }
}

fn rewrite_trailing_fence_label(block: &mut RawBlock, rewrite_plain_labels: bool) -> bool {
    let Some(last) = block.segments.last() else {
        return false;
    };
    let (line, newline) = split_segment(last);
    let parsed = match trailing_fence_label(line) {
        Ok(Some(parsed)) => parsed,
        Ok(None) | Err(_) => return false,
    };
    let prefix = parsed.prefix;
    let label = parsed.label;
    if !rewrite_plain_labels && !has_crossref_prefix(label) {
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
        return block.original_source();
    };
    let code = raw_block_code(block);
    let label_metadata = raw_block_label_metadata(block, had_trailing_label);
    format!(
        "{}#{RUNTIME_ALIAS}.chunk_from_raw_plain({}, raw({}, block: true, lang: {})){}{}",
        block.opening_prefix,
        qmd_string_literal(lang),
        qmd_string_literal(&code),
        qmd_string_literal(lang),
        label_metadata,
        raw_block_closing_suffix(block)
    )
}

fn rewrite_raw_block_as_raw_call(block: &RawBlock, had_trailing_label: bool) -> String {
    let Some(lang) = block.lang.as_deref() else {
        return block.original_source();
    };
    let code = raw_block_code(block);
    let label_metadata = raw_block_label_metadata(block, had_trailing_label);
    format!(
        "{}#raw({}, block: true, lang: {}){}{}",
        block.opening_prefix,
        qmd_string_literal(&code),
        qmd_string_literal(lang),
        label_metadata,
        raw_block_closing_suffix(block)
    )
}

fn raw_block_closing_suffix(block: &RawBlock) -> String {
    let Some(last) = block.segments.last() else {
        return String::new();
    };
    let (line, newline) = split_segment(last);
    if block.opening_prefix.is_empty() {
        return newline.to_string();
    }
    format!("{}{}", closing_fence_tail(line, block.fence_len), newline)
}

fn raw_block_code(block: &RawBlock) -> String {
    if block.segments.len() > 2 {
        block.segments[1..block.segments.len() - 1].concat()
    } else {
        String::new()
    }
}

fn raw_block_label_metadata(block: &RawBlock, had_trailing_label: bool) -> String {
    if had_trailing_label {
        return String::new();
    }
    trailing_label_metadata(&block.segments)
        .map(|label| {
            format!(
                " #metadata((label: {})) {}",
                qmd_string_literal(label),
                FENCE_LABEL_METADATA_LABEL
            )
        })
        .unwrap_or_default()
}

fn trailing_label_metadata(segments: &[String]) -> Option<&str> {
    let last = segments.last()?;
    let (line, _) = split_segment(last);
    Some(trailing_fence_label_candidate(line)?.label)
}

fn is_source_rewritten_chunk_lang(raw_lang: Option<&str>) -> bool {
    let Some(lang) = raw_lang else {
        return false;
    };
    if matches!(lang, "typ" | "typst") {
        return false;
    }
    SOURCE_REWRITTEN_CHUNK_LANGS.contains(&lang)
        || crate::engines::diagram::is_known_diagram_engine_name(lang)
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

fn line_suffix_after_trimmed_end(line: &str) -> &str {
    let trimmed_len = line.trim_end().len();
    &line[trimmed_len..]
}

fn is_executable_label_candidate_lang(raw_lang: Option<&str>) -> bool {
    !matches!(raw_lang, None | Some("typ" | "typst"))
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

fn rewrite_calepin_imports_in_line(
    line: &str,
    lex: &mut LexState,
    runtime_import: &str,
    saw_runtime_import: &mut bool,
) -> String {
    let mut out = String::with_capacity(line.len());
    let mut idx = 0;

    while idx < line.len() {
        if lex.in_block_comment() {
            if line[idx..].starts_with("/*") {
                lex.enter_block_comment();
                out.push_str("/*");
                idx += 2;
            } else if line[idx..].starts_with("*/") {
                lex.exit_block_comment();
                out.push_str("*/");
                idx += 2;
            } else {
                let ch = line[idx..].chars().next().expect("valid char index");
                out.push(ch);
                idx += ch.len_utf8();
            }
            continue;
        }

        if line[idx..].starts_with("//") {
            out.push_str(&line[idx..]);
            break;
        }
        if line[idx..].starts_with("/*") {
            lex.enter_block_comment();
            out.push_str("/*");
            idx += 2;
            continue;
        }
        if line[idx..].starts_with('"') {
            let len = string_literal_source_len(&line[idx..]);
            out.push_str(&line[idx..idx + len]);
            idx += len;
            continue;
        }

        let candidate = &line[idx..];
        if candidate.starts_with("#import") {
            if let Some((rewritten, tail)) = rewrite_import_candidate(candidate, runtime_import) {
                *saw_runtime_import = true;
                let consumed = candidate.len() - tail.len();
                out.push_str(&rewritten);
                idx += consumed;
                continue;
            }
            out.push_str("#import");
            idx += "#import".len();
            continue;
        }

        let ch = candidate.chars().next().expect("valid char index");
        out.push(ch);
        idx += ch.len_utf8();
    }

    out
}

fn scan_line_for_imports<T>(
    line: &str,
    lex: &mut LexState,
    mut handle_import: impl FnMut(&str) -> Option<T>,
) -> Option<T> {
    let mut idx = 0;
    while idx < line.len() {
        match lex.skip_comment(line, idx) {
            Some(CommentStep::Continue(new_idx)) => {
                idx = new_idx;
                continue;
            }
            Some(CommentStep::Break) => break,
            None => {}
        }
        if line[idx..].starts_with('"') {
            idx += string_literal_source_len(&line[idx..]);
            continue;
        }

        let candidate = &line[idx..];
        if candidate.starts_with("#import") {
            if let Some(result) = handle_import(candidate) {
                return Some(result);
            }
            idx += "#import".len();
            continue;
        }

        idx += next_char_len(candidate);
    }

    None
}

fn next_char_len(input: &str) -> usize {
    input.chars().next().map(char::len_utf8).unwrap_or(0)
}

fn string_literal_source_len(input: &str) -> usize {
    debug_assert!(input.starts_with('"'));
    let mut escaped = false;
    for (idx, ch) in input[1..].char_indices() {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return idx + 2;
        }
    }
    input.len()
}

fn import_keyword_boundary(candidate: &str) -> bool {
    candidate["#import".len()..]
        .chars()
        .next()
        .is_none_or(|ch| ch.is_whitespace() || ch == '"')
}

fn rewrite_import_candidate<'a>(
    candidate: &'a str,
    runtime_import: &str,
) -> Option<(String, &'a str)> {
    let import = parse_import_candidate(candidate)?;
    if !is_calepin_runtime_import(&import.literal.value, runtime_import) {
        return None;
    }

    Some((
        format!("#import{}\"{}\"", import.whitespace, runtime_import),
        import.tail,
    ))
}

struct ImportCandidate<'a> {
    whitespace: &'a str,
    literal: StringLiteral,
    tail: &'a str,
}

fn parse_import_candidate(candidate: &str) -> Option<ImportCandidate<'_>> {
    if !import_keyword_boundary(candidate) {
        return None;
    }
    let after_keyword = &candidate["#import".len()..];
    let whitespace_len = after_keyword
        .char_indices()
        .find_map(|(idx, ch)| if ch.is_whitespace() { None } else { Some(idx) })
        .unwrap_or(after_keyword.len());
    let whitespace = &after_keyword[..whitespace_len];
    let after_whitespace = &after_keyword[whitespace_len..];
    let literal = parse_string_literal(after_whitespace)?;

    let tail = &after_whitespace[literal.source_len..];
    Some(ImportCandidate {
        whitespace,
        literal,
        tail,
    })
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

fn is_calepin_runtime_import(value: &str, runtime_import: &str) -> bool {
    if matches!(
        value,
        ".calepin/calepin.typ"
            | RUNTIME_IMPORT_LEGACY
            | "/.calepin/calepin.typ"
            | "/_calepin/calepin.typ"
    ) || value == runtime_import
    {
        return true;
    }

    let Some(runtime_dir) = runtime_import.strip_suffix("calepin.typ") else {
        return false;
    };
    let absolute_value;
    let value = if value.starts_with('/') {
        value
    } else {
        absolute_value = format!("/{value}");
        &absolute_value
    };
    let Some(notebook_path) = value.strip_prefix(runtime_dir) else {
        return false;
    };
    let components = notebook_path.split('/').collect::<Vec<_>>();
    components.len() >= 2
        && components.last() == Some(&"calepin.typ")
        && components[..components.len() - 1]
            .iter()
            .all(|component| !component.is_empty() && !matches!(*component, "." | ".."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::testfixtures;

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
    fn rewrites_legacy_asset_dir_import() {
        assert_eq!(
            rewrite_calepin_imports(r#"#import "/_calepin/calepin.typ""#),
            r#"#import "/.calepin/calepin.typ""#
        );
    }

    #[test]
    fn rewrites_to_custom_runtime_import() {
        let source = r#"#import "/.calepin/calepin.typ" as calepin"#;
        assert_eq!(
            rewrite_source(source, "/_calepin/calepin.typ").source,
            r#"#import "/_calepin/calepin.typ" as calepin"#,
        );
    }

    #[test]
    fn rewrites_notebook_specific_runtime_import() {
        let source = r#"#import "/.calepin/chapters/intro/calepin.typ" as calepin"#;
        assert_eq!(
            rewrite_source(source, "/.calepin/calepin.typ").source,
            r#"#import "/.calepin/calepin.typ" as calepin"#,
        );
    }

    #[test]
    fn rewrites_notebook_specific_custom_asset_import() {
        let source = r#"#import "/_calepin/paper/calepin.typ": chunk"#;
        assert_eq!(
            rewrite_source(source, "/_calepin/calepin.typ").source,
            r#"#import "/_calepin/calepin.typ": chunk"#,
        );
    }

    #[test]
    fn notebook_specific_runtime_import_rejects_traversal() {
        assert!(!is_calepin_runtime_import(
            "/.calepin/../other/calepin.typ",
            "/.calepin/calepin.typ",
        ));
        assert!(!is_calepin_runtime_import(
            "/.calepin/paper//calepin.typ",
            "/.calepin/calepin.typ",
        ));
    }

    #[test]
    fn rewrites_import_after_url_string_on_same_line() {
        assert_eq!(
            rewrite_calepin_imports(
                r#"#metadata("https://example.com") #import ".calepin/calepin.typ""#
            ),
            r#"#metadata("https://example.com") #import "/.calepin/calepin.typ""#
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
    fn ignores_imports_inside_block_comments() {
        let source = r#"/*
#import ".calepin/calepin.typ"
#import "@preview/calepin:0.0.1"
*/
"#;

        reject_preview_calepin_imports(source).unwrap();
        assert_eq!(rewrite_calepin_imports(source), source);
    }

    #[test]
    fn does_not_rewrite_raw_fences_inside_block_comments() {
        let source = r#"/*
```python
print("comment")
```
*/
"#;

        assert_eq!(rewrite_calepin_imports(source), source);
    }

    #[test]
    fn staged_source_only_imports_runtime_alias_for_actual_rewrites() {
        let dir = tempfile::tempdir().unwrap();
        let layout = testfixtures::layout(dir.path());
        std::fs::write(
            &layout.input,
            r##"#let marker = "#calepin_runtime.chunk_from_raw_plain(\"python\")"
"##,
        )
        .unwrap();

        let staged_relative = write_staged_source(&layout, "/.calepin/calepin.typ").unwrap();
        let staged = std::fs::read_to_string(layout.root.join(staged_relative)).unwrap();

        assert!(!staged.contains("as calepin_runtime"), "{staged}");
    }

    fn runtime_import_count(staged: &str) -> usize {
        staged.matches("/.calepin/calepin.typ\" as calepin").count()
    }

    #[test]
    fn injects_default_runtime_import_when_source_lacks_one() {
        let staged = stage_user_source("#calepin.setup()\nHello\n", "/.calepin/calepin.typ");

        assert!(
            staged
                .trim_start()
                .starts_with("#import \"/.calepin/calepin.typ\" as calepin"),
            "{staged}"
        );
    }

    #[test]
    fn does_not_duplicate_runtime_import_authors_already_wrote() {
        let staged = stage_user_source(
            "#import \"/.calepin/calepin.typ\" as calepin\n#calepin.setup()\n",
            "/.calepin/calepin.typ",
        );

        assert_eq!(runtime_import_count(&staged), 1, "{staged}");
    }

    #[test]
    fn skips_injection_when_author_glob_imports_runtime() {
        let staged = stage_user_source(
            "#import \"/.calepin/calepin.typ\": *\n#setup()\n",
            "/.calepin/calepin.typ",
        );

        assert!(!staged.contains("as calepin"), "{staged}");
    }

    #[test]
    fn skips_injection_when_author_uses_custom_runtime_alias() {
        let staged = stage_user_source(
            "#import \"/.calepin/calepin.typ\" as cp\n#cp.setup()\n",
            "/.calepin/calepin.typ",
        );

        assert_eq!(runtime_import_count(&staged), 0, "{staged}");
        assert!(staged.contains("as cp"), "{staged}");
    }

    #[test]
    fn rewrites_store_set_for_default_and_custom_runtime_aliases() {
        let default = stage_user_source(
            "#calepin.store.set(\"region\", \"NY\")\n",
            "/.calepin/calepin.typ",
        );
        assert!(
            default.contains("#calepin.store.set_(\"region\", \"NY\")"),
            "{default}"
        );

        let custom = stage_user_source(
            "#import \"/.calepin/calepin.typ\" as cp\n#cp.store.set(\"region\", \"NY\")\n",
            "/.calepin/calepin.typ",
        );
        assert!(
            custom.contains("#cp.store.set_(\"region\", \"NY\")"),
            "{custom}"
        );

        let bare = stage_user_source(
            "#import \".calepin/calepin.typ\"\n#calepin.store.set(\"region\", \"NY\")\n",
            "/.calepin/calepin.typ",
        );
        assert!(
            bare.contains("#calepin.store.set_(\"region\", \"NY\")"),
            "{bare}"
        );

        let named = stage_user_source(
            "#import \"/.calepin/calepin.typ\": store\n#store.set(\"region\", \"NY\")\n",
            "/.calepin/calepin.typ",
        );
        assert!(named.contains("#store.set_(\"region\", \"NY\")"), "{named}");
    }

    #[test]
    fn store_set_rewrite_ignores_strings_comments_and_raw_blocks() {
        let source = r##"#let literal = "#calepin.store.set(\"wrong\", 1)"
// #calepin.store.set("wrong", 2)
#let inline-raw = `#calepin.store.set("wrong", 3)`
```typ
#calepin.store.set("wrong", 4)
```
#calepin.store.set("right", 5)
"##;
        let staged = stage_user_source(source, "/.calepin/calepin.typ");

        assert!(
            staged.contains(r##""#calepin.store.set(\"wrong\", 1)""##),
            "{staged}"
        );
        assert!(
            staged.contains("// #calepin.store.set(\"wrong\", 2)"),
            "{staged}"
        );
        assert!(
            staged.contains("`#calepin.store.set(\"wrong\", 3)`"),
            "{staged}"
        );
        assert!(
            staged.contains("#calepin.store.set(\"wrong\", 4)"),
            "{staged}"
        );
        assert!(
            staged.contains("#calepin.store.set_(\"right\", 5)"),
            "{staged}"
        );
    }

    #[test]
    fn injects_default_import_when_only_commented_runtime_import_present() {
        let staged = stage_user_source(
            "// #import \"/.calepin/calepin.typ\" as calepin\n#calepin.setup()\n",
            "/.calepin/calepin.typ",
        );

        assert!(
            staged
                .trim_start()
                .starts_with("#import \"/.calepin/calepin.typ\" as calepin\n"),
            "{staged}"
        );
    }

    #[test]
    fn default_and_chunk_alias_imports_coexist_without_author_import() {
        let staged = stage_user_source(
            "#calepin.setup()\n```python\nprint(1)\n```\n",
            "/.calepin/calepin.typ",
        );

        assert!(staged.contains("as calepin\n"), "{staged}");
        assert!(staged.contains("as calepin_runtime\n"), "{staged}");
    }

    #[test]
    fn rewrites_first_class_jupyter_and_shell_fences() {
        let source = "```julia\nprintln(1)\n```\n```bash\necho ok\n```\n```sh\necho sh\n```\n";
        let rewritten = rewrite_calepin_imports(source);

        assert!(
            rewritten.contains("chunk_from_raw_plain(\"julia\""),
            "{rewritten}"
        );
        assert!(
            rewritten.contains("chunk_from_raw_plain(\"bash\""),
            "{rewritten}"
        );
        assert!(
            rewritten.contains("chunk_from_raw_plain(\"sh\""),
            "{rewritten}"
        );
    }

    #[test]
    fn rewrites_dotted_fence_language_before_typst_parses_it() {
        let source = "```julia-1.2\nprintln(1)\n```\n";
        let rewritten = rewrite_calepin_imports(source);

        assert_eq!(
            rewritten,
            "#calepin_runtime.chunk_from_raw_plain(\"julia-1.2\", raw(\"println(1)\\n\", block: true, lang: \"julia-1.2\"))\n"
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
    fn preserves_malformed_executable_fence_label_for_strict_query_validation() {
        let source = "```r\nplot(1)\n```< fig-plot>\n";
        let rewritten = rewrite_calepin_imports(source);

        assert!(rewritten.contains("chunk_from_raw_plain"), "{rewritten}");
        assert!(
            rewritten.contains(r#"#metadata((label: " fig-plot")) <calepin-fence-label>"#),
            "{rewritten}"
        );
        assert!(!rewritten.contains("#| label"), "{rewritten}");
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
    fn preserves_dotted_language_and_decimal_code_inside_explicit_chunks() {
        let source = "#calepin.chunk()[\n```julia-1.2\n.2\nprintln(1)\n```\n]\n";
        let rewritten = rewrite_calepin_imports(source);

        assert_eq!(
            rewritten,
            "#calepin.chunk()[\n#raw(\".2\\nprintln(1)\\n\", block: true, lang: \"julia-1.2\")\n]\n"
        );
    }

    #[test]
    fn preserves_inline_dotted_fence_inside_explicit_chunks() {
        let source = "#calepin.chunk(label: \"versioned\")[```julia-1.2\n.2\nprintln(1)\n```]\n";
        let rewritten = rewrite_calepin_imports(source);

        assert_eq!(
            rewritten,
            "#calepin.chunk(label: \"versioned\")[#raw(\".2\\nprintln(1)\\n\", block: true, lang: \"julia-1.2\")]\n"
        );
    }

    #[test]
    fn recovers_after_inline_raw_fence_body_in_custom_chunk_wrapper() {
        let source =
            "#python_figure()[```python\nplot()\n```]\n\n```python\nprint(\"after\")\n```\n";
        let rewritten = rewrite_calepin_imports(source);
        assert_eq!(
            rewritten,
            "#python_figure()[```python\nplot()\n```]\n\n#calepin_runtime.chunk_from_raw_plain(\"python\", raw(\"print(\\\"after\\\")\\n\", block: true, lang: \"python\"))\n"
        );
    }

    #[test]
    fn does_not_rewrite_nested_fences_inside_typst_examples() {
        let source = "````typ\n```r\nplot(1)\n```<fig-example>\n````\n";
        let rewritten = rewrite_calepin_imports(source);
        assert_eq!(rewritten, source);
    }
}
