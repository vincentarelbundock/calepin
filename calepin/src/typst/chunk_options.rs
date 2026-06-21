use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::typst::fence_label::{metadata_node_label, raw_node_label};

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedChunkSource {
    pub code: String,
    pub overrides: Vec<(String, Value)>,
    pub warnings: Vec<String>,
    pub fence_label: Option<String>,
}

pub fn parse_chunk_body_with_qmd_header(body: &Value, label: &str) -> Result<ParsedChunkSource> {
    let (raw, fence_label) = extract_raw_node_and_fence_label(body, label)?;
    let mut parsed = parse_chunk_source_with_qmd_header(
        raw.get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("chunk `{}` raw element is missing text", label))?,
        label,
    )?;
    parsed.fence_label = fence_label;
    Ok(parsed)
}

pub fn parse_chunk_source_with_qmd_header(source: &str, label: &str) -> Result<ParsedChunkSource> {
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

    Ok(ParsedChunkSource {
        code,
        overrides,
        warnings,
        fence_label: None,
    })
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

fn extract_raw_node_and_fence_label<'a>(
    node: &'a Value,
    label: &str,
) -> Result<(&'a Value, Option<String>)> {
    if is_raw_node(node) {
        return Ok((node, raw_node_label(node)?));
    }

    let Some(children) = node.get("children").and_then(Value::as_array) else {
        return Err(anyhow!(
            "chunk `{}` body must contain exactly one raw element",
            label
        ));
    };

    let mut raw_child = None;
    let mut fence_label = None;
    for child in children {
        if is_raw_node(child) {
            if raw_child.replace(child).is_some() {
                return Err(anyhow!(
                    "chunk `{}` body must contain exactly one raw element",
                    label
                ));
            }
            if let Some(raw_label) = raw_node_label(child)? {
                set_fence_label(&mut fence_label, raw_label, label)?;
            }
            continue;
        }
        if let Some(metadata_label) = metadata_node_label(child)? {
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

    let Some(raw_child) = raw_child else {
        return Err(anyhow!(
            "chunk `{}` body must contain exactly one raw element",
            label
        ));
    };

    Ok((raw_child, fence_label))
}

fn is_raw_node(node: &Value) -> bool {
    node.get("func").and_then(Value::as_str) == Some("raw")
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

const BASE_CHUNK_KEYS: [&str; 7] = [
    "body",
    "code",
    "crossref-labels",
    "engine",
    "label",
    "kind",
    "lang",
];

fn supported_chunk_argument_names() -> String {
    let mut names: Vec<&str> = BASE_CHUNK_KEYS.to_vec();
    names.extend_from_slice(native_chunk_option_names());
    names.sort_unstable();
    names.dedup();
    names.join(", ")
}

fn is_supported_chunk_key(name: &str) -> bool {
    BASE_CHUNK_KEYS.contains(&name) || is_native_chunk_option(name)
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
        "kind",
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
        return Ok(Value::String(decode_double_quoted_string(value)));
    }
    if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
        return Ok(Value::String(value[1..value.len() - 1].to_string()));
    }
    if value.starts_with('[') && value.ends_with(']') {
        let inner = value[1..value.len() - 1].trim();
        if inner.is_empty() {
            return Ok(Value::Array(vec![]));
        }

        let items = split_qmd_array_items(inner)?
            .into_iter()
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

fn split_qmd_array_items(inner: &str) -> Result<Vec<&str>> {
    let mut items = Vec::new();
    let mut item_start = 0usize;
    let mut bracket_depth = 0usize;
    let mut quote = None;
    let mut escaped = false;

    for (idx, ch) in inner.char_indices() {
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
                continue;
            }
            if quote_char == '"' && ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote_char {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '[' => bracket_depth += 1,
            ']' => {
                if bracket_depth == 0 {
                    return Err(anyhow!("unmatched `]` in array value `{inner}`"));
                }
                bracket_depth -= 1;
            }
            ',' if bracket_depth == 0 => {
                items.push(inner[item_start..idx].trim());
                item_start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }

    if let Some(quote_char) = quote {
        return Err(anyhow!(
            "unterminated `{quote_char}` string in array value `{inner}`"
        ));
    }
    if bracket_depth != 0 {
        return Err(anyhow!(
            "unterminated nested array in array value `{inner}`"
        ));
    }

    items.push(inner[item_start..].trim());
    Ok(items)
}

fn decode_double_quoted_string(value: &str) -> String {
    let inner = &value[1..value.len() - 1];
    let mut decoded = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            decoded.push(ch);
            continue;
        }

        let Some(escaped) = chars.next() else {
            decoded.push('\\');
            break;
        };
        match escaped {
            '"' => decoded.push('"'),
            '\\' => decoded.push('\\'),
            'n' => decoded.push('\n'),
            'r' => decoded.push('\r'),
            't' => decoded.push('\t'),
            other => {
                decoded.push('\\');
                decoded.push(other);
            }
        }
    }
    decoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn override_value<'a>(parsed: &'a ParsedChunkSource, key: &str) -> &'a Value {
        parsed
            .overrides
            .iter()
            .find_map(|(name, value)| (name == key).then_some(value))
            .unwrap_or_else(|| panic!("missing override `{key}`"))
    }

    #[test]
    fn parsed_source_exposes_named_fields() {
        let parsed =
            parse_chunk_source_with_qmd_header("#| echo: false\nprint(1)", "chunk-1").unwrap();

        assert_eq!(parsed.code, "print(1)");
        assert_eq!(override_value(&parsed, "echo"), &Value::Bool(false));
        assert!(parsed.warnings.is_empty());
        assert_eq!(parsed.fence_label, None);
    }

    #[test]
    fn parsed_body_carries_trailing_fence_label() {
        let body = json!({
            "func": "sequence",
            "children": [
                {"func": "raw", "text": "#| echo: false\nplot(1)", "block": true},
                {"func": "space"},
                {
                    "func": "metadata",
                    "label": "<calepin-fence-label>",
                    "value": {"label": "fig-trailing"}
                }
            ]
        });

        let parsed = parse_chunk_body_with_qmd_header(&body, "fig-trailing").unwrap();

        assert_eq!(parsed.code, "plot(1)");
        assert_eq!(override_value(&parsed, "echo"), &Value::Bool(false));
        assert_eq!(parsed.fence_label.as_deref(), Some("fig-trailing"));
    }

    #[test]
    fn qmd_arrays_split_only_top_level_commas() {
        let parsed = parse_qmd_value(r#"["A, with comma", ["B, nested", C], "D"]"#).unwrap();

        assert_eq!(parsed, json!(["A, with comma", ["B, nested", "C"], "D"]));
    }

    #[test]
    fn qmd_double_quoted_strings_decode_escapes() {
        let parsed = parse_qmd_value(r#""A \"quoted\" label""#).unwrap();

        assert_eq!(parsed, Value::String("A \"quoted\" label".to_string()));
    }

    #[test]
    fn qmd_double_quoted_strings_preserve_unknown_escapes() {
        let parsed = parse_qmd_value(r#""C:\path\figure.svg""#).unwrap();

        assert_eq!(parsed, Value::String(r#"C:\path\figure.svg"#.to_string()));
    }

    #[test]
    fn qmd_header_accepts_kind_option() {
        let parsed = parse_chunk_source_with_qmd_header("#| kind: fig\nplot(1)", "fig-1").unwrap();

        assert_eq!(
            override_value(&parsed, "kind"),
            &Value::String("fig".to_string())
        );
    }
}
