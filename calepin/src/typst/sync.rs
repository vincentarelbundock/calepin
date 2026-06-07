use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::typst::model::{ChunkSpec, LayoutPaths};
use crate::typst::paths::{artifact_reference, slash_path};
use crate::utils::{process, tools};

const PAGE_SYNC_SCHEMA_VERSION: u8 = 1;
const PAGE_SYNC_SELECTOR: &str = "<calepin-page>";

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
    layout
        .results_path
        .parent()
        .map(|parent| parent.join("pages.json"))
        .unwrap_or_else(|| layout.root.join(".calepin/pages.json"))
}

pub fn write_page_sync(typst: &Path, layout: &LayoutPaths, chunks: &[ChunkSpec]) -> Result<()> {
    let page_json = query_page_anchors(typst, layout)?;
    let pages = parse_page_anchors(&page_json)?;
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

fn query_page_anchors(typst: &Path, layout: &LayoutPaths) -> Result<String> {
    let results_input = artifact_reference(&layout.root, &layout.results_path);
    process::validate_executable(typst, "run typst page sync query", Some(&tools::TYPST))?;
    let output = Command::new(typst)
        .arg("query")
        .arg(&layout.render_input)
        .arg(PAGE_SYNC_SELECTOR)
        .arg("--root")
        .arg(&layout.root)
        .arg("--input")
        .arg("calepin-mode=render")
        .arg("--input")
        .arg(format!("calepin-results={results_input}"))
        .arg("--input")
        .arg("calepin-target=paged")
        .current_dir(&layout.root)
        .output()
        .map_err(|error| {
            process::spawn_error(
                typst,
                "run typst page sync query",
                error,
                Some(&tools::TYPST),
            )
        })?;

    if !output.status.success() {
        return Err(anyhow!(
            "typst query {} failed:\n{}",
            PAGE_SYNC_SELECTOR,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    String::from_utf8(output.stdout).context("typst page sync query output was not UTF-8")
}

fn parse_page_anchors(query_json: &str) -> Result<HashMap<String, usize>> {
    let root: Value = serde_json::from_str(query_json)?;
    let array = root
        .as_array()
        .ok_or_else(|| anyhow!("typst page sync query output must be an array"))?;
    let mut pages = HashMap::new();

    for item in array {
        let Some(value) = item.get("value") else {
            continue;
        };
        let Some(label) = value.get("label").and_then(Value::as_str) else {
            continue;
        };
        let Some(page) = value
            .get("page")
            .and_then(Value::as_u64)
            .and_then(|page| usize::try_from(page).ok())
        else {
            continue;
        };
        pages.entry(label.to_string()).or_insert(page);
    }

    Ok(pages)
}

fn source_lines_for_chunks(
    layout: &LayoutPaths,
    chunks: &[ChunkSpec],
) -> Result<HashMap<String, usize>> {
    let source = std::fs::read_to_string(&layout.input)
        .with_context(|| format!("failed to read {}", layout.input.display()))?;
    let mut lines = HashMap::new();
    let mut search_start = 0;

    for chunk in chunks {
        if chunk.code.is_empty() {
            continue;
        }
        let index = source[search_start..]
            .find(&chunk.code)
            .map(|offset| search_start + offset)
            .or_else(|| source.find(&chunk.code));
        let Some(index) = index else {
            continue;
        };
        lines.insert(chunk.label.clone(), byte_index_to_line(&source, index));
        search_start = index + chunk.code.len();
    }

    Ok(lines)
}

fn byte_index_to_line(source: &str, index: usize) -> usize {
    source[..index]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

fn write_page_sync_document(path: &Path, document: &PageSyncDocument) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(document)?;
    let json = format!("{}\n", json);
    if std::fs::read_to_string(path).is_ok_and(|existing| existing == json) {
        return Ok(());
    }
    std::fs::write(path, json).with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::model::{DisplayOptions, EngineName, ExecOptions, ResultsMode};

    #[test]
    fn parses_page_anchor_query_output() {
        let pages = parse_page_anchors(
            r#"[{"func":"metadata","value":{"label":"chunk-1","page":3},"label":"<calepin-page>"}]"#,
        )
        .unwrap();

        assert_eq!(pages.get("chunk-1"), Some(&3));
    }

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
        let layout = LayoutPaths {
            root: dir.path().to_path_buf(),
            input,
            input_rel: PathBuf::from("paper.typ"),
            render_input: PathBuf::from("paper.typ"),
            work_dir: dir.path().to_path_buf(),
            results_path: dir.path().join(".calepin/paper/results.json"),
            figures_dir: dir.path().join(".calepin/paper/figures"),
        };
        let chunks = vec![
            test_chunk("inline-1", "print(1)"),
            test_chunk("chunk-1", "print(2)"),
        ];

        let lines = source_lines_for_chunks(&layout, &chunks).unwrap();

        assert_eq!(lines.get("inline-1"), Some(&1));
        assert_eq!(lines.get("chunk-1"), Some(&5));
    }

    fn test_chunk(label: &str, code: &str) -> ChunkSpec {
        ChunkSpec {
            label: label.to_string(),
            engine: EngineName::Python,
            code: code.to_string(),
            exec_options: ExecOptions {
                eval: true,
                error: false,
                fig_device_format: "svg".to_string(),
                fig_device_dpi: 150,
                fig_device_width: 6.0,
                fig_device_height: None,
                fig_device_aspect: 0.618,
            },
            display_options: DisplayOptions {
                echo: true,
                output: true,
                results: ResultsMode::Verbatim,
                warning: true,
                message: true,
                placeholder: true,
                fig_width: None,
                fig_height: None,
                fig_align: None,
                fig_responsive: None,
                fig_link: None,
                fig_caption: None,
                fig_caption_position: None,
                fig_alt_text: None,
                fig_subcaptions: None,
                fig_layout_columns: None,
                fig_layout_rows: None,
                kind: None,
            },
            ordinal: 0,
        }
    }
}
