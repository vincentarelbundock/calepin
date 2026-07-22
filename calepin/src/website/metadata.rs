use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::TocConfig;
use crate::typst::preprocess::read_page_meta_with_root;

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
    pub(super) toc: Option<TocConfig>,
    pub(super) raw: serde_json::Value,
}

pub(super) type PageMetaMap = BTreeMap<PathBuf, PageMeta>;

/// Reads the page metadata persisted by preprocessing. Missing or stale
/// entries degrade to an empty `PageMeta` rather than failing the build.
pub(super) fn load_page_meta(src_dir: &Path, typ_files: &[PathBuf]) -> PageMetaMap {
    typ_files
        .iter()
        .map(|path| {
            let mut meta = read_page_meta_with_root(path, Some(src_dir))
                .map(|value| page_meta_from_value(&value))
                .unwrap_or_default();
            if meta.title.is_none() {
                meta.title = document_title_from_source(path);
            }
            (path.clone(), meta)
        })
        .collect()
}

fn document_title_from_source(path: &Path) -> Option<String> {
    let source = fs::read_to_string(path).ok()?;
    extract_document_title(&source)
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
        toc: value.get("toc").and_then(toc_field),
        raw: if value.is_object() {
            value.clone()
        } else {
            serde_json::json!({})
        },
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

fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|field| field.as_str())
        .and_then(|field| clean_optional_string(Some(field)))
}
