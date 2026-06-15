use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

use super::{commands, split_page_meta, PreprocessMetadata, PAGE_META_LABEL, PAGE_SYNC_SELECTOR};
use crate::typst::model::LayoutPaths;
use crate::typst::run::{CalepinMode, CalepinTarget};

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
    let setup_root: Value =
        serde_json::from_str(&setup_json).context("failed to parse typst query output")?;
    let setup_array = setup_root
        .as_array()
        .ok_or_else(|| anyhow!("typst query output must be an array"))?;
    let (setup_json, page_meta) = split_page_meta(setup_array)?;
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
    commands::typst_query(
        typst,
        layout,
        input,
        selector,
        results_input,
        CalepinMode::Query,
        CalepinTarget::Paged,
    )
}

fn query_page_anchors(typst: &Path, layout: &LayoutPaths) -> Result<String> {
    let results_input = super::results_input(layout);
    commands::query_page_anchors(
        typst,
        layout,
        PAGE_SYNC_SELECTOR,
        &results_input,
        CalepinMode::Render,
        CalepinTarget::Paged,
    )
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
    fn parses_page_anchor_query_output() {
        let pages = parse_page_anchors(
            r#"[{"func":"metadata","value":{"label":"chunk-1","page":3},"label":"<calepin-page>"}]"#,
        )
        .unwrap();

        assert_eq!(pages.get("chunk-1"), Some(&3));
    }
}
