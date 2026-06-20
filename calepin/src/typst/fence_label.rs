use anyhow::{anyhow, Result};
use serde_json::Value;

pub(crate) const FENCE_LABEL_METADATA_LABEL: &str = "<calepin-fence-label>";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TrailingFenceLabel<'a> {
    pub(crate) prefix: &'a str,
    pub(crate) label: &'a str,
}

pub(crate) fn label_name(value: &str) -> Result<String> {
    Ok(parse_label_name(value)?.to_string())
}

fn parse_label_name(value: &str) -> Result<&str> {
    let name = if value.starts_with('<') && value.ends_with('>') && value.len() >= 2 {
        &value[1..value.len() - 1]
    } else if value.contains('<') || value.contains('>') {
        return Err(anyhow!("malformed fence label `{value}`"));
    } else {
        value
    };

    if name.is_empty() {
        Err(anyhow!("fence label must not be empty"))
    } else if name.trim() != name {
        Err(anyhow!(
            "fence label must not contain leading or trailing whitespace"
        ))
    } else {
        Ok(name)
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
    let label = parse_label_name(label).ok()?;
    Some(TrailingFenceLabel {
        prefix: &line[..label_start],
        label,
    })
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
    fn label_name_rejects_whitespace_labels() {
        for value in [
            "   ",
            "<   >",
            " fig-demo",
            "fig-demo ",
            "< fig-demo>",
            "<fig-demo >",
        ] {
            assert!(label_name(value).is_err(), "{value:?} should be invalid");
        }
    }

    #[test]
    fn label_name_rejects_malformed_angle_labels() {
        for value in [
            "<fig-demo",
            "fig-demo>",
            "<fig-demo> extra",
            "prefix <fig-demo>",
        ] {
            assert!(label_name(value).is_err(), "{value:?} should be invalid");
        }
    }

    #[test]
    fn trailing_fence_label_parses_closing_fence_label() {
        let parsed = trailing_fence_label("``` <fig-demo>  ").unwrap();
        assert_eq!(parsed.prefix, "``` ");
        assert_eq!(parsed.label, "fig-demo");
        assert!(trailing_fence_label("``` <>").is_none());
        assert!(trailing_fence_label("`` <fig-demo>").is_none());
    }

    #[test]
    fn trailing_fence_label_rejects_invalid_label_names() {
        for line in ["``` <   >", "``` < fig-demo>", "``` <fig-demo >"] {
            assert!(
                trailing_fence_label(line).is_none(),
                "{line:?} should be invalid"
            );
        }
    }
}
