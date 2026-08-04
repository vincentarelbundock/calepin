use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::typst::paths::is_generated_entry_file;
use crate::utils::path::normalize_path;
use crate::utils::static_files::path_has_common_skip_dir;

pub(super) fn collect_typst_files(root: &Path, max_depth: Option<usize>) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect_typst_files_in(root, root, 0, max_depth, &mut out)?;
    out.sort();
    Ok(out)
}

fn collect_typst_files_in(
    root: &Path,
    dir: &Path,
    depth: usize,
    max_depth: Option<usize>,
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        if path_has_common_skip_dir(rel) || is_generated_entry_file(rel) {
            continue;
        }
        if path.is_dir() {
            if max_depth.is_none_or(|limit| depth < limit) {
                collect_typst_files_in(root, &path, depth + 1, max_depth, out)?;
            }
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("typ") {
            out.push(path);
        }
    }
    Ok(())
}

pub(super) fn parse_string_literal(source: &str, quote: usize) -> Option<(String, usize)> {
    let mut out = String::new();
    let mut escaped = false;
    let mut index = quote + 1;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        if escaped {
            out.push(match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some((out, index + ch.len_utf8()));
        } else {
            out.push(ch);
        }
        index += ch.len_utf8();
    }
    None
}

pub(super) fn mask_raw_spans(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut index = 0usize;
    while index < source.len() {
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        if ch != '`' {
            out.push(ch);
            index += ch.len_utf8();
            continue;
        }

        let tick_count = source[index..]
            .chars()
            .take_while(|candidate| *candidate == '`')
            .count();
        let marker = "`".repeat(tick_count);
        let search_start = index + tick_count;
        let Some(relative_end) = source[search_start..].find(&marker) else {
            out.push(ch);
            index += ch.len_utf8();
            continue;
        };
        let end = search_start + relative_end + tick_count;
        push_masked_preserving_lines(&mut out, &source[index..end]);
        index = end;
    }
    out
}

fn push_masked_preserving_lines(out: &mut String, value: &str) {
    for ch in value.chars() {
        if ch == '\n' {
            out.push('\n');
        } else {
            out.push(' ');
        }
    }
}

pub(super) fn find_matching_delimiter(
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

pub(super) fn find_string_end(value: &str, quote_index: usize) -> Option<usize> {
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

pub(super) fn skip_ws(value: &str, mut index: usize) -> usize {
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

pub(super) fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'
}

pub(super) fn is_left_identifier_boundary(value: &str, index: usize) -> bool {
    index == 0
        || !value[..index]
            .chars()
            .next_back()
            .is_some_and(is_identifier_char)
}

pub(super) fn line_number(source: &str, offset: usize) -> usize {
    source[..offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

pub(super) fn display_rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

pub(super) fn is_external_or_special_target(target: &str) -> bool {
    target.starts_with('#')
        || target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("//")
        || target.starts_with("mailto:")
        || target.starts_with("tel:")
        || target.starts_with("data:")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LocalReferenceError {
    EscapesRoot,
}

pub(super) fn resolve_local_reference_target(
    root: &Path,
    source: &Path,
    target: &str,
) -> Option<Result<PathBuf, LocalReferenceError>> {
    let target = target.trim();
    if target.is_empty() || is_external_or_special_target(target) {
        return None;
    }
    let path_part = target
        .split_once(['#', '?'])
        .map(|(path, _)| path)
        .unwrap_or(target)
        .trim();
    if path_part.is_empty() {
        return None;
    }

    let base = if path_part.starts_with('/') {
        root.to_path_buf()
    } else {
        source.parent().unwrap_or(root).to_path_buf()
    };
    let candidate = normalize_path(&base.join(path_part.trim_start_matches('/')));
    if candidate.starts_with(root) {
        Some(Ok(candidate))
    } else {
        Some(Err(LocalReferenceError::EscapesRoot))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_local_reference_target_handles_project_and_source_relative_paths() {
        let root = Path::new("/project");
        let source = root.join("notes").join("paper.typ");

        assert_eq!(
            resolve_local_reference_target(root, &source, "/assets/logo.svg")
                .unwrap()
                .unwrap(),
            root.join("assets").join("logo.svg")
        );
        assert_eq!(
            resolve_local_reference_target(root, &source, "figures/plot.svg")
                .unwrap()
                .unwrap(),
            root.join("notes").join("figures").join("plot.svg")
        );
        assert!(resolve_local_reference_target(root, &source, "https://example.com").is_none());
        assert!(
            resolve_local_reference_target(root, &source, "../../secret.txt")
                .unwrap()
                .is_err()
        );
    }

    #[test]
    fn raw_span_mask_preserves_line_numbers_and_hides_code() {
        let masked = mask_raw_spans("one `#link(\"x\")`\ntwo ```typ\n#image(\"x\")\n``` three");

        assert!(!masked.contains("#link"));
        assert!(!masked.contains("#image"));
        assert_eq!(masked.lines().count(), 4);
        assert!(masked.contains("one"));
        assert!(masked.contains("two"));
        assert!(masked.contains("three"));
    }
}
