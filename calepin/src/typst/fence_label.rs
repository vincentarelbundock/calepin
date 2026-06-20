use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::typst::crossref::has_crossref_prefix;

pub(crate) const FENCE_LABEL_METADATA_LABEL: &str = "<calepin-fence-label>";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TrailingFenceLabel<'a> {
    pub(crate) prefix: &'a str,
    pub(crate) label: &'a str,
}

pub(crate) fn label_name(value: &str) -> Result<String> {
    if value.starts_with('<') && value.ends_with('>') && value.len() >= 2 {
        let name = &value[1..value.len() - 1];
        if name.is_empty() {
            return Err(anyhow!("fence label must not be empty"));
        }
        Ok(name.to_string())
    } else if value.is_empty() {
        Err(anyhow!("fence label must not be empty"))
    } else {
        Ok(value.to_string())
    }
}

pub(crate) fn raw_node_label(node: &Value) -> Result<Option<String>> {
    node.get("label")
        .and_then(Value::as_str)
        .map(label_name)
        .transpose()
}

pub(crate) fn metadata_node_label(node: &Value) -> Result<Option<String>> {
    if node.get("func").and_then(Value::as_str) != Some("metadata")
        || node.get("label").and_then(Value::as_str) != Some(FENCE_LABEL_METADATA_LABEL)
    {
        return Ok(None);
    }
    let value = node
        .get("value")
        .and_then(|value| value.get("label"))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("calepin fence label metadata is missing `label`"))?;
    Ok(Some(label_name(value)?))
}

pub(crate) fn trailing_fence_label(line: &str) -> Option<TrailingFenceLabel<'_>> {
    let trimmed_end = line.trim_end();
    if !trimmed_end.ends_with('>') {
        return None;
    }
    let label_start = trimmed_end.rfind('<')?;
    let label = &trimmed_end[label_start + 1..trimmed_end.len() - 1];
    let before_label = &trimmed_end[..label_start];
    let fence = before_label.trim();
    if fence.len() < 3 || !fence.chars().all(|ch| ch == '`') {
        return None;
    }
    if label.is_empty() {
        return None;
    }
    Some(TrailingFenceLabel {
        prefix: &line[..label_start],
        label,
    })
}

pub(crate) fn is_routed_crossref_label(label: &str) -> bool {
    has_crossref_prefix(label)
}

#[cfg(test)]
mod tests {
    use super::{label_name, trailing_fence_label};

    #[test]
    fn label_name_accepts_query_and_plain_labels() {
        assert_eq!(label_name("<fig-demo>").unwrap(), "fig-demo");
        assert_eq!(label_name("plain-id").unwrap(), "plain-id");
        assert!(label_name("<>").is_err());
        assert!(label_name("").is_err());
    }

    #[test]
    fn trailing_fence_label_parses_closing_fence_label() {
        let parsed = trailing_fence_label("``` <fig-demo>  ").unwrap();
        assert_eq!(parsed.prefix, "``` ");
        assert_eq!(parsed.label, "fig-demo");
        assert!(trailing_fence_label("``` <>").is_none());
        assert!(trailing_fence_label("`` <fig-demo>").is_none());
    }
}
