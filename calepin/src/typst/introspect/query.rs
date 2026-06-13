use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use super::{PreprocessMetadata, PAGE_META_LABEL, PAGE_SYNC_SELECTOR};
use crate::typst::model::LayoutPaths;
use crate::utils::{process, tools};

pub fn preprocess_metadata(
    typst: &Path,
    layout: &LayoutPaths,
    input: &Path,
    results_input: &str,
) -> Result<PreprocessMetadata> {
    let setup_json = typst_query(
        typst,
        layout,
        input,
        &format!("selector(<calepin-config>).or(<{PAGE_META_LABEL}>)"),
        results_input,
    )?;
    let (setup_json, page_meta) = split_page_meta(&setup_json)?;
    let chunks_json = typst_query(
        typst,
        layout,
        input,
        "raw.where(block: true).or(<calepin-fence-label>).or(<calepin-chunk>)",
        results_input,
    )?;

    Ok(PreprocessMetadata {
        setup_json,
        page_meta,
        chunks_json,
    })
}

pub fn page_anchors(typst: &Path, layout: &LayoutPaths) -> Result<HashMap<String, usize>> {
    let page_json = query_page_anchors(typst, layout)?;
    parse_page_anchors(&page_json)
}

fn typst_query(
    typst: &Path,
    layout: &LayoutPaths,
    input: &Path,
    selector: &str,
    results_input: &str,
) -> Result<String> {
    process::validate_executable(typst, "run typst query", Some(&tools::TYPST))?;
    let output = Command::new(typst)
        .arg("query")
        .arg(input)
        .arg(selector)
        .arg("--root")
        .arg(&layout.root)
        .arg("--input")
        .arg("calepin-mode=query")
        .arg("--input")
        .arg(format!("calepin-results={results_input}"))
        .arg("--input")
        .arg("calepin-target=paged")
        // Keep the fallback `typst query` path compatible with documents that
        // reference Typst's HTML module while Calepin is collecting metadata.
        .arg("--features=html")
        .current_dir(&layout.root)
        .output()
        .map_err(|error| {
            process::spawn_error(typst, "run typst query", error, Some(&tools::TYPST))
        })?;

    if !output.status.success() {
        return Err(anyhow!(
            "typst query {} failed:\n{}",
            selector,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    String::from_utf8(output.stdout).context("typst query output was not UTF-8")
}

/// Splits the combined setup query output into the `<calepin-config>` entries
/// (re-serialized for `parse_setup_config`) and the first `<website-metadata>`
/// value, if any.
fn split_page_meta(query_json: &str) -> Result<(String, Option<Value>)> {
    let root: Value =
        serde_json::from_str(query_json).context("failed to parse typst query output")?;
    let array = root
        .as_array()
        .ok_or_else(|| anyhow!("typst query output must be an array"))?;
    let page_meta_label = format!("<{PAGE_META_LABEL}>");
    let mut rest = Vec::new();
    let mut page_meta = None;
    for item in array {
        let label = item.get("label").and_then(Value::as_str);
        if label == Some(page_meta_label.as_str()) {
            if page_meta.is_none() {
                page_meta = item.get("value").cloned();
            }
        } else {
            rest.push(item.clone());
        }
    }
    Ok((serde_json::to_string(&rest)?, page_meta))
}

fn query_page_anchors(typst: &Path, layout: &LayoutPaths) -> Result<String> {
    let results_input = super::results_input(layout);
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
    let entries = array
        .iter()
        .filter_map(|item| {
            let value = item.get("value")?;
            Some(serde_json::json!({
                "label": value.get("label")?,
                "page": value.get("page")?,
            }))
        })
        .collect::<Vec<_>>();
    super::parse_page_anchor_entries(&Value::Array(entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_page_meta_separates_setup_entries_from_page_metadata() {
        let combined = r#"[
            {"func":"metadata","value":{"echo":true},"label":"<calepin-config>"},
            {"func":"metadata","value":{"title":"T","pdf":false},"label":"<website-metadata>"}
        ]"#;

        let (setup_json, page_meta) = split_page_meta(combined).unwrap();

        let setup: Value = serde_json::from_str(&setup_json).unwrap();
        assert_eq!(setup.as_array().unwrap().len(), 1);
        assert_eq!(setup[0]["label"], "<calepin-config>");
        assert_eq!(
            page_meta,
            Some(serde_json::json!({"title": "T", "pdf": false}))
        );
    }

    #[test]
    fn parses_page_anchor_query_output() {
        let pages = parse_page_anchors(
            r#"[{"func":"metadata","value":{"label":"chunk-1","page":3},"label":"<calepin-page>"}]"#,
        )
        .unwrap();

        assert_eq!(pages.get("chunk-1"), Some(&3));
    }
}
