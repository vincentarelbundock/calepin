use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::TocConfig;
use crate::typst::preprocess::read_page_meta_in_dir;

use super::util::clean_optional_string;

/// Per-page metadata exposed through the `<website-metadata>` Typst label,
/// extracted during preprocessing and persisted under `.calepin/`. `title`,
/// `pdf`, `layout`, `translation_key`, `slug`, `url`, and `toc` are the keys
/// calepin interprets; `raw` carries the author's whole dictionary verbatim
/// for the pages index.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct PageMeta {
    pub(super) title: Option<String>,
    pub(super) pdf: Option<bool>,
    pub(super) layout: Option<String>,
    pub(super) translation_key: Option<String>,
    pub(super) slug: Option<String>,
    pub(super) url: Option<String>,
    /// Social-card image for this page: an absolute URL, or a path relative to
    /// the output root.
    pub(super) image: Option<String>,
    /// Old routes that should redirect to this page.
    pub(super) redirect_from: Vec<String>,
    pub(super) toc: Option<TocConfig>,
    pub(super) raw: serde_json::Value,
    /// First prose paragraph of the page source, derived when the author did
    /// not supply `summary` or `description` in `<website-metadata>`.
    pub(super) excerpt: Option<String>,
}

pub(super) type PageMetaMap = BTreeMap<PathBuf, PageMeta>;

/// Reads the page metadata persisted by preprocessing. Missing or stale
/// entries degrade to an empty `PageMeta` rather than failing the build.
pub(super) fn load_page_meta(
    src_dir: &Path,
    typ_files: &[PathBuf],
    asset_dir: &Path,
) -> PageMetaMap {
    typ_files
        .iter()
        .map(|path| {
            let mut meta = read_page_meta_in_dir(path, Some(src_dir), asset_dir)
                .map(|value| page_meta_from_value(&value))
                .unwrap_or_default();
            let source = fs::read_to_string(path).ok();
            if meta.title.is_none() {
                meta.title = source.as_deref().and_then(extract_document_title);
            }
            if meta.excerpt.is_none() {
                meta.excerpt = source.as_deref().and_then(extract_excerpt);
            }
            (path.clone(), meta)
        })
        .collect()
}

pub(super) fn extract_document_title(source: &str) -> Option<String> {
    let mut offset = 0;
    while let Some(start) = find_next_visible_set(source, offset) {
        let mut rest_start = start + "#set".len();
        rest_start = skip_ws(source, rest_start);
        if !source[rest_start..].starts_with("document") {
            offset = rest_start;
            continue;
        }
        let after_document = rest_start + "document".len();
        if source[after_document..]
            .chars()
            .next()
            .is_some_and(is_identifier_char)
        {
            offset = after_document;
            continue;
        }
        let open = skip_ws(source, after_document);
        if !source[open..].starts_with('(') {
            offset = after_document;
            continue;
        }
        let close = find_matching_delimiter(source, open, '(', ')')?;
        let args = &source[open + 1..close];
        if let Some(title) = title_argument(args).and_then(title_value_to_text) {
            return Some(title);
        }
        offset = close + 1;
    }
    None
}

fn find_next_visible_set(source: &str, mut index: usize) -> Option<usize> {
    while index < source.len() {
        if source[index..].starts_with("#set") {
            return Some(index);
        }
        if let Some(next) = ignored_source_span_end(source, index) {
            index = next;
            continue;
        }
        let ch = source[index..].chars().next()?;
        index += ch.len_utf8();
    }
    None
}

fn ignored_source_span_end(source: &str, index: usize) -> Option<usize> {
    let rest = &source[index..];
    if rest.starts_with("//") {
        return Some(line_comment_end(source, index));
    }
    if rest.starts_with("/*") {
        return Some(block_comment_end(source, index));
    }
    match rest.chars().next()? {
        '"' => find_string_end(source, index).map(|end| end + 1),
        '`' => raw_span_end(source, index),
        _ => None,
    }
}

fn line_comment_end(source: &str, index: usize) -> usize {
    source[index..]
        .find('\n')
        .map(|relative| index + relative + 1)
        .unwrap_or(source.len())
}

fn block_comment_end(source: &str, index: usize) -> usize {
    source[index + 2..]
        .find("*/")
        .map(|relative| index + 2 + relative + "*/".len())
        .unwrap_or(source.len())
}

fn raw_span_end(source: &str, index: usize) -> Option<usize> {
    let tick_count = source[index..]
        .chars()
        .take_while(|candidate| *candidate == '`')
        .count();
    let marker = "`".repeat(tick_count);
    let search_start = index + tick_count;
    source[search_start..]
        .find(&marker)
        .map(|relative| search_start + relative + tick_count)
}

fn title_argument(args: &str) -> Option<&str> {
    let mut index = 0;
    while index < args.len() {
        index = skip_ws(args, index);
        let ch = args[index..].chars().next()?;
        if ch == '"' {
            index = find_string_end(args, index)? + 1;
            continue;
        }
        if matches!(ch, '(' | '[' | '{') {
            let close = match ch {
                '(' => ')',
                '[' => ']',
                '{' => '}',
                _ => unreachable!(),
            };
            index = find_matching_delimiter(args, index, ch, close)? + 1;
            continue;
        }
        if args[index..].starts_with("title") && is_left_identifier_boundary(args, index) {
            let after_name = index + "title".len();
            if !args[after_name..]
                .chars()
                .next()
                .is_some_and(is_identifier_char)
            {
                let colon = skip_ws(args, after_name);
                if args[colon..].starts_with(':') {
                    let value_start = skip_ws(args, colon + 1);
                    return Some(args[value_start..].trim());
                }
            }
        }
        index += ch.len_utf8();
    }
    None
}

fn is_left_identifier_boundary(value: &str, index: usize) -> bool {
    index == 0
        || !value[..index]
            .chars()
            .next_back()
            .is_some_and(is_identifier_char)
}

fn title_value_to_text(value: &str) -> Option<String> {
    if value.starts_with('[') {
        let close = find_matching_delimiter(value, 0, '[', ']')?;
        return clean_optional_string(Some(&typst_content_to_plain_text(&value[1..close])));
    }
    if value.starts_with('"') {
        let close = find_string_end(value, 0)?;
        let raw = &value[..=close];
        let parsed = serde_json::from_str::<String>(raw).ok()?;
        return clean_optional_string(Some(&parsed));
    }
    let value = value.split(',').next().unwrap_or(value);
    clean_optional_string(Some(&typst_content_to_plain_text(value)))
}

/// Upper bound on a derived excerpt, in characters. Long enough for a feed
/// blurb or a listing subtitle, short enough that a whole paragraph never
/// lands in a `<summary>` element.
const EXCERPT_MAX_CHARS: usize = 280;

/// Derives a one-paragraph excerpt from a page's Typst source: the first run
/// of prose lines, with markup, code, headings, labels, and comments removed.
/// Returns `None` when the page has no prose before its first non-prose block.
pub(super) fn extract_excerpt(source: &str) -> Option<String> {
    let mut paragraph = String::new();
    let mut lines = source.lines();
    let mut in_block_comment = false;
    // Depth of a code expression left open by an earlier line. A multi-line
    // `#metadata((...))` or `#calepin.setup(...)` call continues with lines
    // like `title: "Post",` that read as prose on their own.
    let mut open_code_depth = 0usize;

    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if in_block_comment {
            in_block_comment = !trimmed.contains("*/");
            continue;
        }
        if trimmed.starts_with("/*") {
            in_block_comment = !trimmed.contains("*/");
            continue;
        }
        if open_code_depth > 0 {
            open_code_depth = code_depth_after(trimmed, open_code_depth);
            continue;
        }
        if let Some(fence) = raw_block_fence(trimmed) {
            // Skip the whole fenced block, including code chunks.
            for line in lines.by_ref() {
                if line.trim().starts_with(&fence) {
                    break;
                }
            }
            if paragraph.is_empty() {
                continue;
            }
            break;
        }
        if is_prose_line(trimmed) {
            if !paragraph.is_empty() {
                paragraph.push(' ');
            }
            paragraph.push_str(trimmed);
            continue;
        }
        // A blank line or a non-prose block ends the first paragraph; before
        // one has started, it is just more preamble to skip.
        if paragraph.is_empty() {
            open_code_depth = code_depth_after(trimmed, 0);
            continue;
        }
        break;
    }

    let text = inline_prose_to_plain_text(&paragraph);
    clean_optional_string(Some(&text)).map(|text| truncate_excerpt(&text, EXCERPT_MAX_CHARS))
}

/// Tracks how many code delimiters a line leaves open, so a call spanning
/// several lines is skipped whole. String contents are ignored.
fn code_depth_after(line: &str, depth: usize) -> usize {
    let mut depth = depth;
    let mut chars = line.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                let mut escaped = false;
                for ch in chars.by_ref() {
                    if escaped {
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == '"' {
                        break;
                    }
                }
            }
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}

/// Returns the fence marker when the line opens a fenced raw block.
fn raw_block_fence(line: &str) -> Option<String> {
    let ticks = line.chars().take_while(|ch| *ch == '`').count();
    (ticks >= 3).then(|| "`".repeat(ticks))
}

/// Prose is everything that is not blank, a comment, or the start of a Typst
/// construct that carries no readable sentence (code, headings, labels, math,
/// lists, and table or figure markup).
fn is_prose_line(line: &str) -> bool {
    if line.is_empty() || line.starts_with("//") {
        return false;
    }
    !matches!(
        line.chars().next(),
        Some('#' | '=' | '<' | '$' | '-' | '+' | '/' | '*' | '|' | ')' | ']' | '}')
    )
}

/// Strips inline Typst markup down to readable text: code expressions and
/// their arguments go away, bracketed content bodies stay.
fn inline_prose_to_plain_text(value: &str) -> String {
    let mut out = String::new();
    let mut index = 0;
    while index < value.len() {
        let Some(ch) = value[index..].chars().next() else {
            break;
        };
        match ch {
            '\\' => {
                // Escapes keep their literal character.
                index += ch.len_utf8();
                if let Some(next) = value[index..].chars().next() {
                    out.push(next);
                    index += next.len_utf8();
                }
                continue;
            }
            '#' => {
                index += ch.len_utf8();
                while value[index..].chars().next().is_some_and(is_call_path_char) {
                    index += value[index..].chars().next().map_or(0, char::len_utf8);
                }
                // Drop the argument list; a trailing content block is prose
                // and is left for the next iteration to keep.
                if value[index..].starts_with('(') {
                    match find_matching_delimiter(value, index, '(', ')') {
                        Some(close) => index = close + 1,
                        None => break,
                    }
                }
                continue;
            }
            '<' => {
                // `<label>` attaches to prose without being part of it.
                if let Some(close) = label_end(value, index) {
                    index = close + 1;
                    continue;
                }
                out.push(ch);
            }
            '`' => {
                // Inline raw is literal text: `#let` must survive as `#let`
                // rather than being read as a code expression.
                let ticks = value[index..].chars().take_while(|ch| *ch == '`').count();
                let marker = "`".repeat(ticks);
                let body_start = index + ticks;
                match value[body_start..].find(&marker) {
                    Some(relative) => {
                        out.push_str(&value[body_start..body_start + relative]);
                        index = body_start + relative + ticks;
                    }
                    None => index = body_start,
                }
                continue;
            }
            '[' | ']' | '*' | '_' => {}
            _ => out.push(ch),
        }
        index += ch.len_utf8();
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_call_path_char(ch: char) -> bool {
    is_identifier_char(ch) || ch == '.'
}

fn label_end(value: &str, open_index: usize) -> Option<usize> {
    let mut index = open_index + 1;
    let mut saw_identifier = false;
    while let Some(ch) = value[index..].chars().next() {
        if ch == '>' {
            return saw_identifier.then_some(index);
        }
        if !is_identifier_char(ch) && ch != ':' {
            return None;
        }
        saw_identifier = true;
        index += ch.len_utf8();
    }
    None
}

/// Truncates on a word boundary so an excerpt never ends mid-word.
fn truncate_excerpt(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut kept = String::new();
    for word in text.split_whitespace() {
        let projected = kept.chars().count() + word.chars().count() + usize::from(!kept.is_empty());
        if projected > max_chars {
            break;
        }
        if !kept.is_empty() {
            kept.push(' ');
        }
        kept.push_str(word);
    }
    if kept.is_empty() {
        kept = text.chars().take(max_chars).collect();
    }
    let trimmed = kept.trim_end_matches(['.', ',', ';', ':', ' ']);
    format!("{trimmed}…")
}

fn typst_content_to_plain_text(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '#' => {
                while chars.peek().is_some_and(|next| is_identifier_char(*next)) {
                    chars.next();
                }
            }
            '[' | ']' => {}
            '\n' | '\r' | '\t' => out.push(' '),
            _ => out.push(ch),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn find_matching_delimiter(
    value: &str,
    open_index: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = open_index;
    while index < value.len() {
        let ch = value[index..].chars().next()?;
        if ch == '"' {
            index = find_string_end(value, index)? + 1;
            continue;
        }
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(index);
            }
        }
        index += ch.len_utf8();
    }
    None
}

fn find_string_end(value: &str, quote_index: usize) -> Option<usize> {
    let mut escaped = false;
    let mut index = quote_index + 1;
    while index < value.len() {
        let ch = value[index..].chars().next()?;
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(index);
        }
        index += ch.len_utf8();
    }
    None
}

fn skip_ws(value: &str, mut index: usize) -> usize {
    while index < value.len() {
        let Some(ch) = value[index..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'
}

pub(super) fn page_meta_from_value(value: &serde_json::Value) -> PageMeta {
    PageMeta {
        title: string_field(value, "title"),
        pdf: value.get("pdf").and_then(|pdf| pdf.as_bool()),
        layout: string_field(value, "layout"),
        translation_key: string_field(value, "translation_key")
            .or_else(|| string_field(value, "translationKey")),
        slug: string_field(value, "slug"),
        url: string_field(value, "url"),
        image: string_field(value, "image"),
        redirect_from: string_list_field(value, "redirect-from")
            .or_else(|| string_list_field(value, "redirect_from"))
            .unwrap_or_default(),
        toc: value.get("toc").and_then(toc_field),
        raw: if value.is_object() {
            value.clone()
        } else {
            serde_json::json!({})
        },
        // Derived from the page source by `load_page_meta`, not from
        // `<website-metadata>`.
        excerpt: None,
    }
}

/// Parses a `toc: (enabled: ..., depth: ..., floating: ...)` page-metadata entry leniently:
/// malformed or out-of-range fields are ignored (left `None`) rather than
/// failing the whole page, matching the rest of this lenient metadata parser.
fn toc_field(value: &serde_json::Value) -> Option<TocConfig> {
    let object = value.as_object()?;
    let enabled = object.get("enabled").and_then(serde_json::Value::as_bool);
    let floating = object.get("floating").and_then(serde_json::Value::as_bool);
    let depth = object
        .get("depth")
        .and_then(serde_json::Value::as_u64)
        .and_then(|depth| usize::try_from(depth).ok())
        .filter(|depth| {
            (crate::config::TOC_MIN_DEPTH..=crate::config::TOC_MAX_DEPTH).contains(depth)
        });
    if enabled.is_none() && depth.is_none() && floating.is_none() {
        return None;
    }
    Some(TocConfig {
        enabled,
        depth,
        floating,
    })
}

/// Reads a list-valued string field. A bare string is accepted as a one-element
/// list, and non-string entries are dropped rather than failing the page.
fn string_list_field(value: &serde_json::Value, key: &str) -> Option<Vec<String>> {
    let field = value.get(key)?;
    if let Some(single) = field.as_str() {
        return Some(clean_optional_string(Some(single)).into_iter().collect());
    }
    let entries = field
        .as_array()?
        .iter()
        .filter_map(|entry| entry.as_str())
        .filter_map(|entry| clean_optional_string(Some(entry)))
        .collect();
    Some(entries)
}

fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|field| field.as_str())
        .and_then(|field| clean_optional_string(Some(field)))
}

#[cfg(test)]
mod toc_field_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn page_meta_parses_toc_enabled_depth_and_floating() {
        let meta =
            page_meta_from_value(&json!({"toc": {"enabled": false, "depth": 2, "floating": true}}));

        assert_eq!(
            meta.toc,
            Some(TocConfig {
                enabled: Some(false),
                depth: Some(2),
                floating: Some(true),
            })
        );
    }

    #[test]
    fn page_meta_keeps_partial_toc_overrides() {
        let meta = page_meta_from_value(&json!({"toc": {"depth": 2}}));

        assert_eq!(
            meta.toc,
            Some(TocConfig {
                enabled: None,
                depth: Some(2),
                floating: None,
            })
        );
    }

    #[test]
    fn page_meta_ignores_out_of_range_toc_depth() {
        let meta = page_meta_from_value(&json!({"toc": {"depth": 99}}));

        assert_eq!(meta.toc, None);
    }

    #[test]
    fn page_meta_ignores_malformed_toc_value() {
        let meta = page_meta_from_value(&json!({"toc": "yes"}));

        assert_eq!(meta.toc, None);
    }

    #[test]
    fn page_meta_without_toc_key_is_none() {
        let meta = page_meta_from_value(&json!({"title": "Hello"}));

        assert_eq!(meta.toc, None);
    }
}
