use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use super::{commands, split_page_meta, PreprocessMetadata};
use crate::typst::model::LayoutPaths;
use crate::typst::run::{TypstInput, INPUT_MODE, INPUT_RESULTS, INPUT_TARGET};

static EVAL_AVAILABLE: OnceLock<Mutex<HashMap<PathBuf, bool>>> = OnceLock::new();

pub fn is_available(typst: &Path) -> bool {
    let cache = EVAL_AVAILABLE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(available) = cache
        .lock()
        .ok()
        .and_then(|cache| cache.get(typst).copied())
    {
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
    let output = commands::typst_eval(
        typst,
        layout,
        input,
        r#"(
  setup: query(selector(<calepin-config>).or(<website-metadata>)),
  chunks: query(raw.where(block: true).or(<calepin-fence-label>).or(<calepin-chunk>)),
)"#,
        &[
            TypstInput::new(INPUT_MODE, "query"),
            TypstInput::new(INPUT_RESULTS, results_input),
            TypstInput::new(INPUT_TARGET, "paged"),
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
    let setup_array = setup
        .as_array()
        .ok_or_else(|| anyhow!("typst eval setup output must be an array"))?;
    let (setup_json, page_meta) = split_page_meta(setup_array)?;

    Ok(PreprocessMetadata {
        setup_json,
        page_meta,
        chunks_json: serde_json::to_string(&chunks)?,
    })
}

pub fn page_anchors(typst: &Path, layout: &LayoutPaths) -> Result<HashMap<String, usize>> {
    let results_input = super::results_input(layout);
    let output = commands::typst_eval(
        typst,
        layout,
        &layout.render_input,
        r#"query(<calepin-page>).map(it => (
  label: it.value.label,
  page: it.value.page,
))"#,
        &[
            TypstInput::new(INPUT_MODE, "render"),
            TypstInput::new(INPUT_RESULTS, results_input),
            TypstInput::new(INPUT_TARGET, "paged"),
        ],
    )?;
    let root: Value =
        serde_json::from_str(&output).context("failed to parse typst eval page sync output")?;
    super::parse_page_anchor_entries(&root)
}

#[cfg(test)]
mod tests {}
