use anyhow::{anyhow, Context, Result};
use serde_json::Value;

pub(super) fn parse_query_values(query_json: &str) -> Result<Vec<Value>> {
    serde_json::from_str(query_json).context("failed to parse typst query output")
}

pub(super) fn parse_metadata_values(query_json: &str) -> Result<Vec<Value>> {
    let root: Value = serde_json::from_str(query_json)?;
    let array = root
        .as_array()
        .ok_or_else(|| anyhow!("typst query output must be an array"))?;
    array
        .iter()
        .map(|item| {
            item.get("value")
                .cloned()
                .ok_or_else(|| anyhow!("metadata item is missing `value`"))
        })
        .collect()
}

pub(super) fn is_calepin_chunk_metadata(value: &Value) -> bool {
    value.get("func") == Some(&Value::String("metadata".into()))
        && value
            .get("label")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "<calepin-chunk>")
}

pub(super) fn is_raw_code_block(value: &Value) -> bool {
    value.get("func") == Some(&Value::String("raw".into()))
        && value.get("block").and_then(Value::as_bool) == Some(true)
}

pub(super) fn is_calepin_fence_label_metadata(value: &Value) -> bool {
    value.get("func") == Some(&Value::String("metadata".into()))
        && value
            .get("label")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "<calepin-fence-label>")
}

pub(super) fn value_for<'a>(object: &'a Value, key: &str) -> Option<&'a Value> {
    let value = object.get(key)?;
    if is_auto(value) || value.is_null() {
        None
    } else {
        Some(value)
    }
}

pub(super) fn is_auto(value: &Value) -> bool {
    value.as_str() == Some("auto")
}

pub(super) fn extract_text(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(children) = value.get("children").and_then(Value::as_array) {
        let mut text = String::new();
        for child in children {
            if let Some(child_text) = extract_text(child) {
                text.push_str(&child_text);
            }
        }
        return Some(text);
    }
    None
}
