mod commands;
mod eval;

use anyhow::{anyhow, Result};
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
    pub chunk_queries: Vec<String>,
    pub store_initializer_queries: Vec<String>,
}

pub fn preprocess_metadata(
    typst: &Path,
    layout: &LayoutPaths,
    input: &Path,
    results_input: &str,
    store_input: &str,
) -> Result<PreprocessMetadata> {
    eval::preprocess_metadata(typst, layout, input, results_input, store_input)
}

pub fn page_anchors(typst: &Path, layout: &LayoutPaths) -> Result<HashMap<String, usize>> {
    eval::page_anchors(typst, layout)
}

fn results_input(layout: &LayoutPaths) -> Result<String> {
    crate::typst::paths::artifact_reference(&layout.root, &layout.results_path)
}

fn parse_page_anchor_entries(root: &Value) -> Result<HashMap<String, usize>> {
    let array = root
        .as_array()
        .ok_or_else(|| anyhow!("typst page sync output must be an array"))?;
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

fn split_page_meta(items: &[Value]) -> Result<(String, Option<Value>)> {
    let page_meta_label = format!("<{PAGE_META_LABEL}>");
    let mut rest = Vec::new();
    let mut page_meta = None;
    for item in items {
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
    fn parses_page_anchor_entries() {
        let pages = parse_page_anchor_entries(&serde_json::json!([
            {"label": "chunk-1", "page": 3}
        ]))
        .unwrap();

        assert_eq!(pages.get("chunk-1"), Some(&3));
    }

    #[test]
    fn split_page_meta_separates_setup_entries_from_page_metadata() {
        let setup = serde_json::json!([
            {"func":"metadata","value":{"echo":true},"label":"<calepin-config>"},
            {"func":"metadata","value":{"title":"T","pdf":false},"label":"<website-metadata>"}
        ]);

        let (setup_json, page_meta) = split_page_meta(setup.as_array().unwrap()).unwrap();

        let setup: Value = serde_json::from_str(&setup_json).unwrap();
        assert_eq!(setup.as_array().unwrap().len(), 1);
        assert_eq!(setup[0]["label"], "<calepin-config>");
        assert_eq!(
            page_meta,
            Some(serde_json::json!({"title": "T", "pdf": false}))
        );
    }
}
