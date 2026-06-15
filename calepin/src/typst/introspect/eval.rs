use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use super::{root_relative, split_page_meta, PreprocessMetadata};
use crate::typst::model::LayoutPaths;
use crate::typst::run::{run_typst_capture, TypstInput, INPUT_MODE, INPUT_RESULTS, INPUT_TARGET};

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
    let output = typst_eval(
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
    let output = typst_eval(
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

fn typst_eval(
    typst: &Path,
    layout: &LayoutPaths,
    input: &Path,
    expression: &str,
    inputs: &[TypstInput],
) -> Result<String> {
    let input = root_relative(input, &layout.root);
    let mut args: Vec<OsString> = vec![
        "eval".into(),
        expression.into(),
        "--in".into(),
        input.as_os_str().into(),
        "--root".into(),
        layout.root.as_os_str().into(),
        "--format".into(),
        "json".into(),
        // Documents may use Typst's HTML module even during metadata
        // introspection; enable the feature just as the final HTML compile does.
        "--features=html".into(),
    ];
    for input in inputs {
        input.push_to(&mut args);
    }
    run_typst_capture(
        typst,
        "run typst eval",
        &args,
        &layout.root,
        |stderr| format!("typst eval failed:\n{stderr}"),
        "typst eval output was not UTF-8",
    )
}

#[cfg(test)]
mod tests {}
