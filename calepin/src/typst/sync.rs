use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::typst::introspect::page_anchors;
use crate::typst::io::write_if_changed;
use crate::typst::model::{ChunkSpec, LayoutPaths};
use crate::typst::paths::slash_path;

const PAGE_SYNC_SCHEMA_VERSION: u8 = 1;

#[derive(Serialize)]
struct PageSyncDocument {
    schema: u8,
    input: String,
    entries: Vec<PageSyncEntry>,
}

#[derive(Serialize)]
struct PageSyncEntry {
    label: String,
    file: String,
    line: usize,
    page: usize,
}

pub fn page_sync_path(layout: &LayoutPaths) -> PathBuf {
    layout.sibling_path("pages.json")
}

pub fn write_page_sync(typst: &Path, layout: &LayoutPaths, chunks: &[ChunkSpec]) -> Result<()> {
    let pages = page_anchors(typst, layout)?;
    let lines = source_lines_for_chunks(layout, chunks)?;
    let input = slash_path(&layout.input_rel);
    let mut entries = Vec::new();

    for chunk in chunks {
        let Some(page) = pages.get(&chunk.label).copied() else {
            continue;
        };
        let Some(line) = lines.get(&chunk.label).copied() else {
            continue;
        };
        entries.push(PageSyncEntry {
            label: chunk.label.clone(),
            file: input.clone(),
            line,
            page,
        });
    }

    write_page_sync_document(
        &page_sync_path(layout),
        &PageSyncDocument {
            schema: PAGE_SYNC_SCHEMA_VERSION,
            input,
            entries,
        },
    )
}

fn source_lines_for_chunks(
    layout: &LayoutPaths,
    chunks: &[ChunkSpec],
) -> Result<HashMap<String, usize>> {
    let source = std::fs::read_to_string(&layout.input)
        .with_context(|| format!("failed to read {}", layout.input.display()))?;
    let code_ranges = source_code_ranges(&source);
    let mut lines = HashMap::new();
    let mut search_start = 0;

    for chunk in chunks {
        if chunk.code.is_empty() {
            continue;
        }
        let index = find_chunk_code_index(&source, &code_ranges, &chunk.code, search_start);
        let Some(index) = index else {
            continue;
        };
        lines.insert(chunk.label.clone(), byte_index_to_line(&source, index));
        search_start = index + chunk.code.len();
    }

    Ok(lines)
}

fn find_chunk_code_index(
    source: &str,
    code_ranges: &[Range<usize>],
    code: &str,
    search_start: usize,
) -> Option<usize> {
    for range in code_ranges {
        if range.end <= search_start {
            continue;
        }
        let start = range.start.max(search_start);
        if start > range.end {
            continue;
        }
        if let Some(offset) = source[start..range.end].find(code) {
            return Some(start + offset);
        }
    }

    source[search_start..]
        .find(code)
        .map(|offset| search_start + offset)
        .or_else(|| source.find(code))
}

fn source_code_ranges(source: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut open_fence: Option<(usize, usize)> = None;
    let mut offset = 0;

    for segment in source.split_inclusive('\n') {
        let (line, newline) = segment
            .strip_suffix('\n')
            .map(|line| (line, 1))
            .unwrap_or((segment, 0));
        let line_start = offset;
        let line_end = line_start + line.len();
        let segment_end = line_end + newline;
        let trimmed = line.trim_start();

        if let Some((fence_len, code_start)) = open_fence {
            if leading_backtick_count(trimmed) >= fence_len {
                ranges.push(code_start..line_start);
                open_fence = None;
            }
        } else {
            let fence_len = leading_backtick_count(trimmed);
            if fence_len >= 3 {
                open_fence = Some((fence_len, segment_end));
            } else {
                collect_inline_code_ranges(line, line_start, &mut ranges);
            }
        }

        offset = segment_end;
    }

    if let Some((_, code_start)) = open_fence {
        ranges.push(code_start..source.len());
    }

    ranges
}

fn collect_inline_code_ranges(line: &str, line_start: usize, ranges: &mut Vec<Range<usize>>) {
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut open = None;

    while index < bytes.len() {
        if bytes[index] != b'`' {
            index += 1;
            continue;
        }

        let mut end = index;
        while end < bytes.len() && bytes[end] == b'`' {
            end += 1;
        }
        let tick_count = end - index;
        if tick_count < 3 {
            if let Some(start) = open.take() {
                ranges.push(start..line_start + index);
            } else {
                open = Some(line_start + end);
            }
        }
        index = end;
    }
}

fn leading_backtick_count(value: &str) -> usize {
    value.chars().take_while(|ch| *ch == '`').count()
}

fn byte_index_to_line(source: &str, index: usize) -> usize {
    source[..index]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

fn write_page_sync_document(path: &Path, document: &PageSyncDocument) -> Result<()> {
    let json = serde_json::to_string_pretty(document)?;
    let json = format!("{}\n", json);
    write_if_changed(path, json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::model::ResultsMode;
    use crate::typst::testfixtures;

    #[test]
    fn maps_chunks_to_source_lines_in_document_order() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(
            &input,
            r#"#calepin.inline("python")[`print(1)`]

#calepin.chunk("python")[
```
print(2)
```
]
"#,
        )
        .unwrap();
        let mut layout = testfixtures::layout(dir.path());
        layout.input = input;
        let chunks = vec![
            test_chunk("inline-1", "print(1)"),
            test_chunk("chunk-1", "print(2)"),
        ];

        let lines = source_lines_for_chunks(&layout, &chunks).unwrap();

        assert_eq!(lines.get("inline-1"), Some(&1));
        assert_eq!(lines.get("chunk-1"), Some(&5));
    }

    #[test]
    fn maps_chunks_to_code_regions_before_matching_prose() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(
            &input,
            r#"The code text print(2) appears in prose first.

#calepin.chunk("python")[
```
print(2)
```
]
"#,
        )
        .unwrap();
        let mut layout = testfixtures::layout(dir.path());
        layout.input = input;
        let chunks = vec![test_chunk("chunk-1", "print(2)")];

        let lines = source_lines_for_chunks(&layout, &chunks).unwrap();

        assert_eq!(lines.get("chunk-1"), Some(&5));
    }

    fn test_chunk(label: &str, code: &str) -> ChunkSpec {
        testfixtures::chunk(label, code, ResultsMode::Verbatim)
    }
}
