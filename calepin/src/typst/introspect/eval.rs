use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

use super::{commands, split_page_meta, PreprocessMetadata, PAGE_META_LABEL, PAGE_SYNC_SELECTOR};
use crate::typst::model::LayoutPaths;
use crate::typst::run::{TypstInput, INPUT_MODE, INPUT_RESULTS, INPUT_TARGET};

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
        &format!(
            r#"(
  setup: query(selector(<calepin-config>).or(<{PAGE_META_LABEL}>)),
  chunks: query(raw.where(block: true).or(<calepin-fence-label>).or(<calepin-chunk>)),
)"#
        ),
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
        &format!(
            r#"query({PAGE_SYNC_SELECTOR}).map(it => (
  label: it.value.label,
  page: it.value.page,
))"#
        ),
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
