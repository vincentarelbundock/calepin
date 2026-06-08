use anyhow::{anyhow, Result};
use serde_json::Value;

pub fn parse_chunk_body_with_qmd_header(
    body: &Value,
    label: &str,
) -> Result<(String, Vec<(String, Value)>, Vec<String>)> {
    let (raw, _) = extract_raw_node_and_fence_label(body, label)?;
    parse_chunk_source_with_qmd_header(
        raw.get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("chunk `{}` raw element is missing text", label))?,
        label,
    )
}

pub fn parse_chunk_source_with_qmd_header(
    source: &str,
    label: &str,
) -> Result<(String, Vec<(String, Value)>, Vec<String>)> {
    let mut code = String::new();
    let mut overrides = Vec::new();
    let mut warnings = Vec::new();
    let mut reading_header = true;

    for (line_num, line) in source.split_inclusive('\n').enumerate() {
        if !reading_header {
            code.push_str(line);
            continue;
        }

        let trimmed = line.trim();
        if !trimmed.starts_with("#|") {
            reading_header = false;
            code.push_str(line);
            continue;
        }

        let directive = trimmed.trim_start_matches("#|").trim();
        if directive.is_empty() {
            continue;
        }

        let (raw_key, raw_value) = directive
            .split_once(':')
            .ok_or_else(|| {
                anyhow!(
                    "chunk `{}` header line {}: malformed option declaration `{}` (expected `#| key: value`)",
                    label,
                    line_num + 1,
                    trimmed,
                )
            })?;
        let (key, did_translate) = resolve_chunk_option_name(raw_key.trim(), label, line_num + 1)?;
        if did_translate {
            warnings.push(format!(
                "chunk `{}` option `{}` was translated to `{}`",
                label,
                raw_key.trim(),
                key
            ));
        }
        let value = parse_qmd_value(raw_value.trim())?;
        overrides.push((key, value));
    }

    Ok((code, overrides, warnings))
}

pub fn validate_chunk_arguments(value: &Value, label: &str) -> Result<()> {
    let Some(value_obj) = value.as_object() else {
        return Err(anyhow!("chunk `{}` metadata is not an object", label));
    };

    for key in value_obj.keys() {
        if !is_supported_chunk_key(key) {
            return Err(anyhow!(
                "chunk `{}` has unsupported argument `{}` in calepin.chunk() arguments. Supported arguments: {}",
                label,
                key,
                supported_chunk_argument_names(),
            ));
        }
    }

    Ok(())
}

pub fn fence_label_from_chunk_body(body: &Value, label: &str) -> Result<Option<String>> {
    let (_, fence_label) = extract_raw_node_and_fence_label(body, label)?;
    Ok(fence_label)
}

fn extract_raw_node_and_fence_label<'a>(
    node: &'a Value,
    label: &str,
) -> Result<(&'a Value, Option<String>)> {
    if node.get("func").and_then(Value::as_str) == Some("raw") {
        return Ok((node, raw_node_label(node)?));
    }

    let Some(children) = node.get("children").and_then(Value::as_array) else {
        return Err(anyhow!(
            "chunk `{}` body must contain exactly one raw element",
            label
        ));
    };

    let raw_children: Vec<&Value> = children
        .iter()
        .filter(|child| child.get("func").and_then(Value::as_str) == Some("raw"))
        .collect();
    if raw_children.len() != 1 {
        return Err(anyhow!(
            "chunk `{}` body must contain exactly one raw element",
            label
        ));
    }

    let mut fence_label = None;
    for child in children {
        if child.get("func").and_then(Value::as_str) == Some("raw") {
            if let Some(raw_label) = raw_node_label(child)? {
                set_fence_label(&mut fence_label, raw_label, label)?;
            }
            continue;
        }
        if let Some(metadata_label) = calepin_fence_label_metadata(child)? {
            set_fence_label(&mut fence_label, metadata_label, label)?;
            continue;
        }
        if !is_whitespace_node(child) {
            return Err(anyhow!(
                "chunk `{}` body contains extra non-whitespace markup",
                label
            ));
        }
    }

    Ok((raw_children[0], fence_label))
}

fn raw_node_label(node: &Value) -> Result<Option<String>> {
    node.get("label")
        .and_then(Value::as_str)
        .map(query_label_name)
        .transpose()
}

fn query_label_name(value: &str) -> Result<String> {
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

fn calepin_fence_label_metadata(node: &Value) -> Result<Option<String>> {
    if node.get("func").and_then(Value::as_str) != Some("metadata")
        || node.get("label").and_then(Value::as_str) != Some("<calepin-fence-label>")
    {
        return Ok(None);
    }
    let value = node
        .get("value")
        .and_then(|value| value.get("label"))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("calepin fence label metadata is missing `label`"))?;
    Ok(Some(query_label_name(value)?))
}

fn set_fence_label(slot: &mut Option<String>, next: String, label: &str) -> Result<()> {
    if let Some(existing) = slot {
        return Err(anyhow!(
            "chunk `{}` has more than one trailing fence label (`{}` and `{}`)",
            label,
            existing,
            next
        ));
    }
    *slot = Some(next);
    Ok(())
}

fn is_whitespace_node(node: &Value) -> bool {
    matches!(
        node.get("func").and_then(Value::as_str),
        Some("space") | Some("linebreak")
    ) || node
        .get("text")
        .and_then(Value::as_str)
        .is_some_and(|s| s.trim().is_empty())
}

fn supported_chunk_argument_names() -> String {
    let mut names: Vec<&str> = vec![
        "body",
        "code",
        "crossref-labels",
        "engine",
        "label",
        "kind",
        "lang",
    ];
    names.extend_from_slice(native_chunk_option_names());
    names.sort_unstable();
    names.dedup();
    names.join(", ")
}

fn is_supported_chunk_key(name: &str) -> bool {
    matches!(
        name,
        "body" | "code" | "crossref-labels" | "engine" | "label" | "kind" | "lang"
    ) || is_native_chunk_option(name)
}

fn resolve_chunk_option_name(raw_key: &str, label: &str, line_no: usize) -> Result<(String, bool)> {
    if raw_key == "label" {
        return Ok((raw_key.to_string(), false));
    }
    if let Some(canonical) = translate_chunk_option_name(raw_key) {
        return Ok((canonical.to_string(), canonical != raw_key));
    }
    if is_native_chunk_option(raw_key) {
        return Ok((raw_key.to_string(), false));
    }

    let dashed = raw_key.replace('.', "-");
    if dashed != raw_key && is_native_chunk_option(&dashed) {
        return Ok((dashed, true));
    }

    Err(anyhow!(
        "chunk `{}` header line {}: unsupported option `{}`. Supported options: {}",
        label,
        line_no,
        raw_key,
        supported_qmd_options(),
    ))
}

const CHUNK_OPTION_ALIASES: [(&str, &str); 13] = [
    ("out-width", "fig-width"),
    ("out-height", "fig-height"),
    ("out-align", "fig-align"),
    ("fig-dpi", "fig-device-dpi"),
    ("fig-format", "fig-device-format"),
    ("fig-asp", "fig-device-aspect"),
    ("fig.cap", "fig-caption"),
    ("fig.align", "fig-align"),
    ("fig-alt", "fig-alt-text"),
    ("fig-subcap", "fig-subcaptions"),
    ("fig-scap", "fig-caption"),
    ("layout-ncol", "fig-layout-columns"),
    ("layout-nrow", "fig-layout-rows"),
];

fn native_chunk_option_names() -> &'static [&'static str] {
    &[
        "echo",
        "eval",
        "error",
        "output",
        "results",
        "warning",
        "message",
        "placeholder",
        "fig-device-format",
        "fig-device-dpi",
        "fig-device-width",
        "fig-device-height",
        "fig-device-aspect",
        "fig-width",
        "fig-height",
        "fig-align",
        "fig-responsive",
        "fig-link",
        "fig-caption",
        "fig-cap-location",
        "fig-alt-text",
        "fig-subcaptions",
        "fig-layout-columns",
        "fig-layout-rows",
    ]
}

fn supported_qmd_options() -> String {
    let mut names: Vec<&str> = native_chunk_option_names().to_vec();
    names.push("label");

    names.extend(CHUNK_OPTION_ALIASES.iter().map(|(alias, _)| *alias));
    names.sort_unstable();
    names.dedup();
    names.join(", ")
}

pub(crate) fn parse_qmd_value(value: &str) -> Result<Value> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("true") {
        return Ok(Value::Bool(true));
    }
    if value.eq_ignore_ascii_case("false") {
        return Ok(Value::Bool(false));
    }
    if value.eq_ignore_ascii_case("null") {
        return Ok(Value::Null);
    }
    if let Ok(int) = value.parse::<i64>() {
        return Ok(Value::from(int));
    }
    if let Ok(float) = value.parse::<f64>() {
        return Ok(Value::from(float));
    }
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        return Ok(Value::String(value[1..value.len() - 1].to_string()));
    }
    if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
        return Ok(Value::String(value[1..value.len() - 1].to_string()));
    }
    if value.starts_with('[') && value.ends_with(']') {
        let inner = value[1..value.len() - 1].trim();
        if inner.is_empty() {
            return Ok(Value::Array(vec![]));
        }

        let items = inner
            .split(',')
            .map(|item| parse_qmd_value(item.trim()))
            .collect::<Result<Vec<_>>>()?;
        return Ok(Value::Array(items));
    }
    Ok(Value::String(value.to_string()))
}

fn translate_chunk_option_name(name: &str) -> Option<&'static str> {
    CHUNK_OPTION_ALIASES
        .iter()
        .find_map(|(from, to)| if *from == name { Some(*to) } else { None })
}

fn is_native_chunk_option(name: &str) -> bool {
    native_chunk_option_names().contains(&name)
}
