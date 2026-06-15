use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::typst::preprocess::read_page_meta_with_root;

/// Per-page metadata exposed through the `<website-metadata>` Typst label,
/// extracted during preprocessing and persisted under `.calepin/`. `title`,
/// `pdf`, and `layout` are the keys calepin interprets; `raw` carries the
/// author's whole dictionary verbatim for the pages index.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct PageMeta {
    pub(super) title: Option<String>,
    pub(super) pdf: Option<bool>,
    pub(super) layout: Option<String>,
    pub(super) translation_key: Option<String>,
    pub(super) slug: Option<String>,
    pub(super) url: Option<String>,
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
    while let Some(relative) = source[offset..].find("#set") {
        let start = offset + relative;
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
        if source[open..].chars().next() != Some('(') {
            offset = after_document;
            continue;
        }
        let close = find_matching_delimiter(source, open, '(', ')')?;
        let args = &source[open + 1..close];
        return title_argument(args).and_then(title_value_to_text);
    }
    None
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
                if args[colon..].chars().next() == Some(':') {
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
        title: value
            .get("title")
            .and_then(|title| title.as_str())
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(str::to_string),
        pdf: value.get("pdf").and_then(|pdf| pdf.as_bool()),
        layout: value
            .get("layout")
            .and_then(|layout| layout.as_str())
            .map(str::trim)
            .filter(|layout| !layout.is_empty())
            .map(str::to_string),
        translation_key: value
            .get("translation_key")
            .or_else(|| value.get("translationKey"))
            .and_then(|key| key.as_str())
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(str::to_string),
        slug: value
            .get("slug")
            .and_then(|slug| slug.as_str())
            .map(str::trim)
            .filter(|slug| !slug.is_empty())
            .map(str::to_string),
        url: value
            .get("url")
            .and_then(|url| url.as_str())
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(str::to_string),
        raw: if value.is_object() {
            value.clone()
        } else {
            serde_json::json!({})
        },
    }
}

fn clean_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
