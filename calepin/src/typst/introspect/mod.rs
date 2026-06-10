mod eval;
mod query;

use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::typst::model::LayoutPaths;

const PAGE_META_LABEL: &str = "website-metadata";
const PAGE_SYNC_SELECTOR: &str = "<calepin-page>";

#[derive(Debug, Clone)]
pub struct PreprocessMetadata {
    pub setup_json: String,
    pub page_meta: Option<Value>,
    pub chunks_json: String,
}

pub fn preprocess_metadata(
    typst: &Path,
    layout: &LayoutPaths,
    input: &Path,
    results_input: &str,
) -> Result<PreprocessMetadata> {
    if eval::is_available(typst) {
        eval::preprocess_metadata(typst, layout, input, results_input)
    } else {
        query::preprocess_metadata(typst, layout, input, results_input)
    }
}

pub fn page_anchors(typst: &Path, layout: &LayoutPaths) -> Result<HashMap<String, usize>> {
    if eval::is_available(typst) {
        eval::page_anchors(typst, layout)
    } else {
        query::page_anchors(typst, layout)
    }
}

fn results_input(layout: &LayoutPaths) -> String {
    crate::typst::paths::artifact_reference(&layout.root, &layout.results_path)
}

fn parse_page_anchor_entries(root: &Value) -> Result<HashMap<String, usize>> {
    let array = root
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("typst page sync output must be an array"))?;
    let mut pages = HashMap::new();

    for item in array {
        let Some(label) = item.get("label").and_then(Value::as_str) else {
            continue;
        };
        let Some(page) = item
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

fn root_relative(path: &Path, root: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_page_anchor_entries() {
        let pages = parse_page_anchor_entries(&serde_json::json!([
            {"label": "chunk-1", "page": 3}
        ]))
        .unwrap();

        assert_eq!(pages.get("chunk-1"), Some(&3));
    }
}
