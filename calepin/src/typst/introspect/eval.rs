use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use super::{root_relative, PreprocessMetadata, PAGE_META_LABEL};
use crate::typst::model::LayoutPaths;
use crate::utils::{process, tools};

static EVAL_AVAILABLE: OnceLock<Mutex<HashMap<PathBuf, bool>>> = OnceLock::new();

pub fn is_available(typst: &Path) -> bool {
    let cache = EVAL_AVAILABLE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(available) = cache.lock().ok().and_then(|cache| cache.get(typst).copied()) {
        return available;
    }

    let available = Command::new(typst)
        .arg("eval")
        .arg("1")
        .arg("--format")
        .arg("json")
        .output()
        .is_ok_and(|output| output.status.success());
    if let Ok(mut cache) = cache.lock() {
        cache.insert(typst.to_path_buf(), available);
    }
    available
}

pub fn preprocess_metadata(
    typst: &Path,
    layout: &LayoutPaths,
    input: &Path,
    results_input: &str,
) -> Result<PreprocessMetadata> {
    let output = typst_eval(
        typst,
        layout,
        input,
        r#"(
  setup: query(selector(<calepin-config>).or(<website-metadata>)),
  chunks: query(raw.where(block: true).or(<calepin-fence-label>).or(<calepin-chunk>)),
)"#,
        &[
            "calepin-mode=query".to_string(),
            format!("calepin-results={results_input}"),
            "calepin-target=paged".to_string(),
        ],
    )?;
    let root: Value =
        serde_json::from_str(&output).context("failed to parse typst eval metadata output")?;
    let setup = root
        .get("setup")
        .cloned()
        .ok_or_else(|| anyhow!("typst eval metadata output is missing `setup`"))?;
    let chunks = root
        .get("chunks")
        .cloned()
        .ok_or_else(|| anyhow!("typst eval metadata output is missing `chunks`"))?;
    let (setup_json, page_meta) = split_page_meta(setup)?;

    Ok(PreprocessMetadata {
        setup_json,
        page_meta,
        chunks_json: serde_json::to_string(&chunks)?,
    })
}

pub fn page_anchors(typst: &Path, layout: &LayoutPaths) -> Result<HashMap<String, usize>> {
    let results_input = super::results_input(layout);
    let output = typst_eval(
        typst,
        layout,
        &layout.render_input,
        r#"query(<calepin-page>).map(it => (
  label: it.value.label,
  page: it.value.page,
))"#,
        &[
            "calepin-mode=render".to_string(),
            format!("calepin-results={results_input}"),
            "calepin-target=paged".to_string(),
        ],
    )?;
    let root: Value =
        serde_json::from_str(&output).context("failed to parse typst eval page sync output")?;
    super::parse_page_anchor_entries(&root)
}

fn typst_eval(
    typst: &Path,
    layout: &LayoutPaths,
    input: &Path,
    expression: &str,
    inputs: &[String],
) -> Result<String> {
    process::validate_executable(typst, "run typst eval", Some(&tools::TYPST))?;
    let input = root_relative(input, &layout.root);
    let mut command = Command::new(typst);
    command
        .arg("eval")
        .arg(expression)
        .arg("--in")
        .arg(input)
        .arg("--root")
        .arg(&layout.root)
        .arg("--format")
        .arg("json");
    for input in inputs {
        command.arg("--input").arg(input);
    }
    let output = command
        .current_dir(&layout.root)
        .output()
        .map_err(|error| {
            process::spawn_error(typst, "run typst eval", error, Some(&tools::TYPST))
        })?;

    if !output.status.success() {
        return Err(anyhow!(
            "typst eval failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    String::from_utf8(output.stdout).context("typst eval output was not UTF-8")
}

fn split_page_meta(setup: Value) -> Result<(String, Option<Value>)> {
    let array = setup
        .as_array()
        .ok_or_else(|| anyhow!("typst eval setup output must be an array"))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_page_meta_separates_setup_entries_from_page_metadata() {
        let setup = serde_json::json!([
            {"func":"metadata","value":{"echo":true},"label":"<calepin-config>"},
            {"func":"metadata","value":{"title":"T","pdf":false},"label":"<website-metadata>"}
        ]);

        let (setup_json, page_meta) = split_page_meta(setup).unwrap();

        let setup: Value = serde_json::from_str(&setup_json).unwrap();
        assert_eq!(setup.as_array().unwrap().len(), 1);
        assert_eq!(setup[0]["label"], "<calepin-config>");
        assert_eq!(
            page_meta,
            Some(serde_json::json!({"title": "T", "pdf": false}))
        );
    }
}
