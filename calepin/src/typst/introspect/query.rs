use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;

use super::{split_page_meta, PreprocessMetadata, PAGE_META_LABEL, PAGE_SYNC_SELECTOR};
use crate::typst::model::LayoutPaths;
use crate::typst::run::{push_calepin_inputs, run_typst_capture, CalepinMode, CalepinTarget};

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
    let mut args: Vec<OsString> = vec![
        "query".into(),
        input.as_os_str().into(),
        selector.into(),
        "--root".into(),
        layout.root.as_os_str().into(),
    ];
    push_calepin_inputs(
        &mut args,
        CalepinMode::Query,
        results_input,
        CalepinTarget::Paged,
    );
    // Keep the fallback `typst query` path compatible with documents that
    // reference Typst's HTML module while Calepin is collecting metadata.
    args.push("--features=html".into());
    run_typst_capture(
        typst,
        "run typst query",
        &args,
        &layout.root,
        |stderr| format!("typst query {selector} failed:\n{stderr}"),
        "typst query output was not UTF-8",
    )
}

fn query_page_anchors(typst: &Path, layout: &LayoutPaths) -> Result<String> {
    let results_input = super::results_input(layout);
    let mut args: Vec<OsString> = vec![
        "query".into(),
        layout.render_input.as_os_str().into(),
        PAGE_SYNC_SELECTOR.into(),
        "--root".into(),
        layout.root.as_os_str().into(),
    ];
    push_calepin_inputs(
        &mut args,
        CalepinMode::Render,
        &results_input,
        CalepinTarget::Paged,
    );
    run_typst_capture(
        typst,
        "run typst page sync query",
        &args,
        &layout.root,
        |stderr| format!("typst query {PAGE_SYNC_SELECTOR} failed:\n{stderr}"),
        "typst page sync query output was not UTF-8",
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
