use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

use super::{commands, split_page_meta, PreprocessMetadata, PAGE_META_LABEL, PAGE_SYNC_SELECTOR};
use crate::typst::model::LayoutPaths;
use crate::typst::run::{
    source_dir_input, CalepinMode, CalepinTarget, TypstInput, INPUT_MODE, INPUT_RESULTS,
    INPUT_SOURCE_DIR, INPUT_STORE, INPUT_TARGET,
};

pub fn preprocess_metadata(
    typst: &Path,
    layout: &LayoutPaths,
    input: &Path,
    results_input: &str,
    store_input: &str,
) -> Result<PreprocessMetadata> {
    // Chunk discovery uses Typst eval rather than Rust text scanning because
    // executable chunks are part of the evaluated Typst document: they can come
    // from includes, functions, loops, Calepin chunk calls, setup defaults, and
    // target conditionals. A Rust scanner would only see literal source text
    // and would miss valid Typst-generated chunks.
    //
    // Discovery also needs both target-specific branches. A document may put
    // executable chunks inside `if target == "html"` UI, while another may put
    // chunks inside `if target == "paged"` fallbacks. Calepin executes chunks
    // before the final render, so a single-target scan can miss chunks that the
    // later render will ask for.
    let paged = preprocess_metadata_for_target(
        typst,
        layout,
        input,
        results_input,
        store_input,
        CalepinTarget::Paged,
    )?;
    let html = preprocess_metadata_for_target(
        typst,
        layout,
        input,
        results_input,
        store_input,
        CalepinTarget::Html,
    )?;
    let setup = paged
        .get("setup")
        .cloned()
        .ok_or_else(|| anyhow!("typst eval metadata output is missing `setup`"))?;
    let setup_array = setup
        .as_array()
        .ok_or_else(|| anyhow!("typst eval setup output must be an array"))?;
    let (setup_json, page_meta) = split_page_meta(setup_array)?;
    let paged_chunks = paged
        .get("chunks")
        .cloned()
        .ok_or_else(|| anyhow!("typst eval metadata output is missing `chunks`"))?;
    let html_chunks = html
        .get("chunks")
        .cloned()
        .ok_or_else(|| anyhow!("typst eval metadata output is missing `chunks`"))?;

    Ok(PreprocessMetadata {
        setup_json,
        page_meta,
        chunk_queries: vec![
            serde_json::to_string(&paged_chunks)?,
            serde_json::to_string(&html_chunks)?,
        ],
        store_initializer_queries: vec![
            serde_json::to_string(paged.get("initializers").unwrap_or(&Value::Null))?,
            serde_json::to_string(html.get("initializers").unwrap_or(&Value::Null))?,
        ],
    })
}

fn preprocess_metadata_for_target(
    typst: &Path,
    layout: &LayoutPaths,
    input: &Path,
    results_input: &str,
    store_input: &str,
    target: CalepinTarget,
) -> Result<Value> {
    let output = commands::typst_eval(
        typst,
        layout,
        input,
        &format!(
            r#"(
  setup: query(selector(<calepin-config>).or(<{PAGE_META_LABEL}>)),
  chunks: query(raw.where(block: true).or(<calepin-fence-label>).or(<calepin-chunk>)),
  initializers: query(<calepin-store-initializer>),
)"#
        ),
        target,
        &[
            TypstInput::new(INPUT_MODE, CalepinMode::Query.as_str()),
            TypstInput::new(INPUT_RESULTS, results_input),
            TypstInput::new(INPUT_STORE, store_input),
            TypstInput::new(INPUT_SOURCE_DIR, source_dir_input(layout)),
            TypstInput::new(INPUT_TARGET, target.as_str()),
        ],
    )?;
    serde_json::from_str(&output).context("failed to parse typst eval metadata output")
}

pub fn page_anchors(typst: &Path, layout: &LayoutPaths) -> Result<HashMap<String, usize>> {
    let results_input = super::results_input(layout)?;
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
        CalepinTarget::Paged,
        &[
            TypstInput::new(INPUT_MODE, CalepinMode::Render.as_str()),
            TypstInput::new(INPUT_RESULTS, results_input),
            TypstInput::new(INPUT_SOURCE_DIR, source_dir_input(layout)),
            TypstInput::new(INPUT_TARGET, CalepinTarget::Paged.as_str()),
        ],
    )?;
    let root: Value =
        serde_json::from_str(&output).context("failed to parse typst eval page sync output")?;
    super::parse_page_anchor_entries(&root)
}

#[cfg(test)]
mod tests {}
