mod options;
mod value;

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashSet;

use crate::typst::chunk_options::{
    parse_chunk_body_with_qmd_header, parse_chunk_source_with_qmd_header, validate_chunk_arguments,
    ParsedChunkSource,
};
use crate::typst::crossref::parse_prefixed_label_docs;
use crate::typst::fence_label::{label_name, metadata_node_label};
use crate::typst::model::{ChunkSpec, CrossrefLabelDoc, EngineName};

use options::{parse_chunk_options, parse_script_destination};
pub use options::{parse_setup_config, SetupConfig};
use value::{
    extract_text, is_auto, is_calepin_chunk_metadata, is_calepin_fence_label_metadata,
    is_raw_code_block, parse_query_values, value_for,
};

#[cfg(test)]
fn parse_chunks(query_json: &str, defaults: Option<SetupConfig>) -> Result<Vec<ChunkSpec>> {
    Ok(parse_chunks_with_warnings(query_json, defaults)?.chunks)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkParseResult {
    pub chunks: Vec<ChunkSpec>,
    pub warnings: Vec<String>,
}

pub fn parse_chunks_with_warnings(
    query_json: &str,
    defaults: Option<SetupConfig>,
) -> Result<ChunkParseResult> {
    let config = defaults.unwrap_or_default();
    let values = parse_query_values(query_json)
        .context("failed to parse calepin chunk metadata from typst query output")?;
    let has_chunk_metadata = values.iter().any(is_calepin_chunk_metadata);
    let mut seen = HashSet::new();
    let mut chunks = Vec::with_capacity(values.len());
    let mut auto_label_index = 1usize;
    let mut warnings = Vec::new();

    let mut index = 0usize;
    while index < values.len() {
        let ordinal = index;
        let value = &values[index];
        if is_raw_code_block(value) {
            if raw_observation_has_matching_metadata(&values, index) {
                // A broad theme show rule can leave the original raw element
                // one or more times immediately before the metadata emitted
                // by `_fenced-chunk`. Treat that adjacent observation
                // run as one chunk, while preserving repeated authored blocks
                // when no matching metadata follows.
                index += 1;
                continue;
            }
            let lookahead_fence_label = values
                .get(index + 1)
                .filter(|value| is_calepin_fence_label_metadata(value))
                .map(parse_fence_label_metadata)
                .transpose()?;
            let mut state = ChunkParseState {
                seen: &mut seen,
                auto_label_index: &mut auto_label_index,
                warnings: &mut warnings,
            };
            if let Some(chunk) = parse_chunk_raw_block(
                value,
                &config,
                &mut state,
                ordinal,
                has_chunk_metadata,
                lookahead_fence_label.clone(),
            )? {
                chunks.push(chunk);
            }
            if lookahead_fence_label.is_some() {
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }

        if is_calepin_fence_label_metadata(value) {
            index += 1;
            continue;
        }

        if is_calepin_chunk_metadata(value) {
            let value = value
                .get("value")
                .context("chunk metadata is missing `value` field")?;
            let label = parse_label(value)?;
            let label = normalize_chunk_label(label.as_str(), &mut seen, &mut auto_label_index)?;
            if !seen.insert(label.clone()) {
                return Err(anyhow!("duplicate label `{}`", label));
            }
            parse_chunk_metadata(value, &config, &label, &mut chunks, ordinal, &mut warnings)?;
            bump_auto_label(&mut auto_label_index, &label)?;
        }
        index += 1;
    }

    Ok(ChunkParseResult { chunks, warnings })
}

fn raw_observation_has_matching_metadata(values: &[Value], index: usize) -> bool {
    let raw = &values[index];
    let mut next = index + 1;
    while values
        .get(next)
        .is_some_and(|candidate| same_raw_observation(raw, candidate))
    {
        next += 1;
    }
    values
        .get(next)
        .is_some_and(|metadata| raw_matches_chunk_metadata(raw, metadata))
}

fn same_raw_observation(left: &Value, right: &Value) -> bool {
    is_raw_code_block(right)
        && left.get("lang") == right.get("lang")
        && left.get("text") == right.get("text")
}

fn raw_matches_chunk_metadata(raw: &Value, metadata: &Value) -> bool {
    if !is_calepin_chunk_metadata(metadata) {
        return false;
    }
    let Some(raw_lang) = raw.get("lang").and_then(Value::as_str) else {
        return false;
    };
    let Some(chunk) = metadata.get("value") else {
        return false;
    };
    let Some(engine) = chunk.get("engine").and_then(Value::as_str) else {
        return false;
    };
    let Some(raw_text) = raw.get("text").and_then(Value::as_str) else {
        return false;
    };
    let Some(body_text) = chunk.get("body").and_then(extract_text) else {
        return false;
    };

    raw_lang == engine && raw_text == body_text
}

struct ChunkParseState<'a> {
    seen: &'a mut HashSet<String>,
    auto_label_index: &'a mut usize,
    warnings: &'a mut Vec<String>,
}

fn parse_fence_label_metadata(value: &Value) -> Result<String> {
    metadata_node_label(value)?
        .ok_or_else(|| anyhow!("calepin fence label metadata is missing `label`"))
}

fn raw_block_query_label(value: &Value) -> Result<Option<String>> {
    value
        .get("label")
        .and_then(Value::as_str)
        .map(label_name)
        .transpose()
}

fn parse_chunk_metadata(
    value: &Value,
    config: &SetupConfig,
    label: &str,
    chunks: &mut Vec<ChunkSpec>,
    ordinal: usize,
    warnings: &mut Vec<String>,
) -> Result<()> {
    validate_chunk_arguments(value, label)?;

    let ParsedChunkSource {
        code,
        overrides: chunk_options,
        warnings: mut header_warnings,
        fence_label,
    } = parse_chunk_body_with_qmd_header(value.get("body").unwrap_or(&Value::Null), label)?;
    warnings.append(&mut header_warnings);
    let mut value_with_options = value
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow!("chunk metadata is not an object"))?;
    for (key, value) in chunk_options {
        value_with_options.insert(key, value);
    }

    let value = Value::Object(value_with_options);
    let engine = parse_engine(&value)?;
    let defaults = &config.defaults;
    let (exec_options, display_options) = parse_chunk_options(&value, defaults)?;
    let script = parse_script_destination(&value, &defaults.script)?;
    let mut crossref_labels = parse_crossref_labels(&value)
        .map_err(|err| anyhow!("invalid cross-reference labels for chunk `{label}`: {err}"))?;
    if let Some(fence_label) = fence_label {
        let names = vec![fence_label];
        let routed_fence_labels = parse_prefixed_label_docs(&names)
            .map_err(|err| anyhow!("invalid trailing fence label for chunk `{label}`: {err}"))?;
        // A non-prefixed trailing label is a plain id (already reflected in the
        // chunk's label); only prefixed labels route to cross-references.
        if !routed_fence_labels.is_empty() {
            if crossref_labels.is_empty() {
                crossref_labels = routed_fence_labels;
            } else if crossref_labels != routed_fence_labels {
                return Err(anyhow!(
                    "chunk `{label}` supplied a trailing fence label and another label channel"
                ));
            }
        }
    }
    chunks.push(ChunkSpec {
        label: label.to_string(),
        engine,
        code,
        script,
        exec_options,
        display_options,
        ordinal,
        crossref_labels,
    });
    Ok(())
}

fn parse_chunk_label_index(label: &str) -> Option<usize> {
    let suffix = label.strip_prefix("chunk-")?;
    suffix.parse::<usize>().ok()
}

fn normalize_chunk_label(
    label: &str,
    seen: &mut HashSet<String>,
    auto_label_index: &mut usize,
) -> Result<String> {
    let Some(label_index) = parse_chunk_label_index(label) else {
        return Ok(label.to_string());
    };

    if label_index < *auto_label_index {
        let next_label = format!("chunk-{auto_label_index}");
        if seen.contains(&next_label) {
            return next_available_label(seen, auto_label_index);
        }
        return Ok(next_label);
    }

    Ok(label.to_string())
}

fn parse_chunk_raw_block(
    value: &Value,
    config: &SetupConfig,
    state: &mut ChunkParseState<'_>,
    ordinal: usize,
    has_chunk_metadata: bool,
    lookahead_fence_label: Option<String>,
) -> Result<Option<ChunkSpec>> {
    let Some(lang) = value.get("lang").and_then(Value::as_str) else {
        return Ok(None);
    };
    if is_typst_fence(lang) {
        return Ok(None);
    }
    let engine = EngineName::parse(lang)?;
    let raw_fence_label = raw_block_query_label(value)?;
    let raw_text = value.get("text").and_then(Value::as_str).unwrap_or("");
    let label_hint = format!("chunk-{}", *state.auto_label_index);
    let ParsedChunkSource {
        code: raw_code,
        overrides: chunk_options,
        warnings: mut header_warnings,
        fence_label: _,
    } = parse_chunk_source_with_qmd_header(raw_text, &label_hint)?;
    state.warnings.append(&mut header_warnings);
    let mut value_with_options = value
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow!("fenced chunk is not an object"))?;
    value_with_options.remove("label");
    for (key, value) in chunk_options {
        value_with_options.insert(key, value);
    }
    let value = Value::Object(value_with_options);
    let code = raw_code;
    let defaults = &config.defaults;
    if !defaults.fenced_chunks.allows(lang) {
        return Ok(None);
    }
    if has_chunk_metadata && !matches!(engine, EngineName::Jupyter(_)) {
        return Ok(None);
    }
    let fence_label = match (lookahead_fence_label, raw_fence_label) {
        (Some(a), Some(b)) => {
            return Err(anyhow!(
                "fenced chunk supplied more than one trailing fence label (`{a}` and `{b}`)"
            ));
        }
        (Some(label), None) | (None, Some(label)) => Some(label),
        (None, None) => None,
    };
    let (label, crossref_labels) = if let Some(label_value) = value_for(&value, "label") {
        if let Some(fence_label) = fence_label {
            return Err(anyhow!(
                "fenced chunk supplied both `#| label:` and trailing fence label `{fence_label}`"
            ));
        }
        let names = label_names_from_value(label_value)?;
        resolve_named_label(
            names,
            state.seen,
            state.auto_label_index,
            "invalid cross-reference labels for fenced chunk",
        )?
    } else if let Some(fence_label) = fence_label {
        resolve_named_label(
            vec![fence_label],
            state.seen,
            state.auto_label_index,
            "invalid trailing fence label for fenced chunk",
        )?
    } else {
        let label = next_available_label(state.seen, state.auto_label_index)?;
        state.seen.insert(label.clone());
        (label, vec![])
    };
    let (exec_options, display_options) = parse_chunk_options(&value, defaults)?;
    let script = parse_script_destination(&value, &defaults.script)?;
    Ok(Some(ChunkSpec {
        label,
        engine,
        code,
        script,
        exec_options,
        display_options,
        ordinal,
        crossref_labels,
    }))
}

fn resolve_named_label(
    names: Vec<String>,
    seen: &mut HashSet<String>,
    auto_label_index: &mut usize,
    error_context: &str,
) -> Result<(String, Vec<CrossrefLabelDoc>)> {
    let label = names
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("fenced chunk label list is empty"))?;
    validate_chunk_label(&label)?;
    let crossref_labels =
        parse_prefixed_label_docs(&names).map_err(|err| anyhow!("{error_context}: {err}"))?;
    if !seen.insert(label.clone()) {
        return Err(anyhow!("duplicate label `{}`", label));
    }
    bump_auto_label(auto_label_index, &label)?;
    Ok((label, crossref_labels))
}

fn is_typst_fence(lang: &str) -> bool {
    matches!(lang, "typ" | "typst")
}

fn bump_auto_label(auto_label_index: &mut usize, label: &str) -> Result<()> {
    let Some(suffix) = label.strip_prefix("chunk-") else {
        return Ok(());
    };
    if let Ok(idx) = suffix.parse::<usize>() {
        let next = idx
            .checked_add(1)
            .ok_or_else(|| anyhow!("chunk label `{label}` exceeds the supported numeric range"))?;
        *auto_label_index = (*auto_label_index).max(next);
    }
    Ok(())
}

fn next_available_label(seen: &mut HashSet<String>, counter: &mut usize) -> Result<String> {
    while seen.contains(&format!("chunk-{counter}")) {
        *counter = (*counter)
            .checked_add(1)
            .ok_or_else(|| anyhow!("automatic chunk label counter exhausted"))?;
    }
    let label = format!("chunk-{counter}");
    *counter = (*counter)
        .checked_add(1)
        .ok_or_else(|| anyhow!("automatic chunk label counter exhausted"))?;
    Ok(label)
}

fn parse_label(value: &Value) -> Result<String> {
    let Some(label) = value.get("label").and_then(Value::as_str) else {
        return Err(anyhow!("missing label"));
    };
    if label.trim().is_empty() {
        return Err(anyhow!("missing label"));
    }
    validate_chunk_label(label)?;
    Ok(label.to_string())
}

fn validate_chunk_label(label: &str) -> Result<()> {
    if label.is_empty() {
        return Err(anyhow!("chunk label must not be empty"));
    }
    if label.trim() != label {
        return Err(anyhow!(
            "chunk label `{label}` must not contain leading or trailing whitespace"
        ));
    }
    if label.chars().any(char::is_control) {
        return Err(anyhow!(
            "chunk label `{label}` must not contain control characters"
        ));
    }
    Ok(())
}

fn parse_engine(value: &Value) -> Result<EngineName> {
    let engine = value
        .get("engine")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing engine"))?;
    EngineName::parse(engine)
}

fn parse_crossref_labels(value: &Value) -> Result<Vec<CrossrefLabelDoc>> {
    let Some(raw) = value.get("crossref-labels") else {
        return Ok(Vec::new());
    };
    if is_auto(raw) || raw.is_null() {
        return Ok(Vec::new());
    }

    let names = match raw {
        Value::String(name) => vec![name.clone()],
        Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| anyhow!("`crossref-labels` entries must be strings"))
            })
            .collect::<Result<Vec<_>>>()?,
        _ => return Err(anyhow!("`crossref-labels` must be a string or an array")),
    };
    if names.is_empty() {
        return Ok(Vec::new());
    }

    parse_prefixed_label_docs(&names)
}

fn label_names_from_value(value: &Value) -> Result<Vec<String>> {
    match value {
        Value::String(name) => Ok(vec![name.clone()]),
        Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| anyhow!("label entries must be strings"))
            })
            .collect::<Result<Vec<_>>>(),
        _ => Err(anyhow!("label must be a string or an array")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::model::{EngineName, FencedChunks, ResultsLocation, ResultsMode, SetupDefaults};

    fn metadata(value: &str) -> String {
        format!(r#"[{{"func":"metadata","value":{value},"label":"<calepin-chunk>"}}]"#)
    }

    fn setup_metadata(value: &str) -> String {
        format!(r#"[{{"func":"metadata","value":{value},"label":"<calepin-config>"}}]"#)
    }

    fn setup_config_with(defaults: SetupDefaults) -> SetupConfig {
        SetupConfig { defaults }
    }

    #[test]
    fn parse_valid_chunk() {
        let json = metadata(
            r#"{
              "body":{"func":"raw","text":"x <- 1","block":false},
              "code":"x <- 1",
              "engine":"r",
              "label":"setup",
              "echo":false,
              "eval":"auto",
              "output":"auto",
              "results":"render",
              "warning":"auto",
              "message":"auto",
              "error":"auto",
              "placeholder":"auto",
              "fig-device-format":"auto",
              "fig-device-dpi":"auto",
              "fig-device-width":"auto",
              "fig-device-height":"auto",
              "fig-device-aspect":"auto",
              "fig-width":"70%",
              "fig-height":"auto",
              "fig-align":"center",
              "fig-responsive":true,
              "fig-link":"https://example.com",
              "fig-caption":{"func":"text","text":"Caption"},
              "fig-cap-location":"top",
              "fig-alt-text":null,
              "fig-subcaptions":["A","B"],
              "fig-layout-columns":[1,1],
              "fig-layout-rows":"auto",
              "kind":"auto"
            }"#,
        );
        let chunks = parse_chunks(&json, None).unwrap();
        assert_eq!(chunks.len(), 1);
        let chunk = &chunks[0];
        assert_eq!(chunk.label, "setup");
        assert_eq!(chunk.engine, EngineName::R);
        assert_eq!(chunk.code, "x <- 1");
        assert!(!chunk.display_options.echo);
        assert_eq!(chunk.exec_options.fig_device_format, "svg");
        assert_eq!(chunk.exec_options.fig_device_width, 6.0);
        assert_eq!(chunk.exec_options.fig_device_aspect, 0.618);
        assert_eq!(
            chunk.display_options.fig_caption.as_deref(),
            Some("Caption")
        );
        assert_eq!(
            chunk.display_options.fig_subcaptions.as_ref().unwrap(),
            &vec!["A".to_string(), "B".to_string()]
        );
        assert_eq!(chunk.ordinal, 0);
    }

    #[test]
    fn parses_qmd_style_headers_in_metadata_chunks() {
        let json = metadata(
            &serde_json::json!({
                "body":{"func":"raw","text":"#| echo: false\n#| out-width: 80%\nprint(1)","block":false},
                "engine":"r",
                "label":"from-header"
            })
            .to_string(),
        );
        let parsed = parse_chunks_with_warnings(&json, None).unwrap();
        let chunk = &parsed.chunks[0];
        assert_eq!(chunk.label, "from-header");
        assert_eq!(chunk.code, "print(1)");
        assert!(!chunk.display_options.echo);
        assert_eq!(
            chunk
                .display_options
                .fig_width
                .as_ref()
                .and_then(|v| v.as_str()),
            Some("80%")
        );
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.contains("translated to `fig-width`")));
    }

    #[test]
    fn parses_crossref_labels_from_metadata_chunks() {
        let json = metadata(
            &serde_json::json!({
                "body":{"func":"raw","text":"plot(1)","block":false},
                "engine":"r",
                "label":"fig-plot",
                "crossref-labels":["fig-plot","lst-plot"]
            })
            .to_string(),
        );
        let chunks = parse_chunks(&json, None).unwrap();
        assert_eq!(chunks[0].label, "fig-plot");
        assert_eq!(chunks[0].crossref_labels.len(), 2);
        assert_eq!(chunks[0].crossref_labels[0].kind, "fig");
        assert_eq!(chunks[0].crossref_labels[0].name, "fig-plot");
        assert_eq!(chunks[0].crossref_labels[1].kind, "lst");
    }

    #[test]
    fn accepts_unprefixed_label_as_plain_id_from_metadata_chunks() {
        let json = metadata(
            &serde_json::json!({
                "body":{"func":"raw","text":"plot(1)","block":false},
                "engine":"r",
                "label":"plot",
                "crossref-labels":["plot"]
            })
            .to_string(),
        );
        // A plain (non-prefixed) label is a valid chunk id, not a
        // cross-reference, so it is accepted with no cross-reference labels.
        let chunks = parse_chunks(&json, None).unwrap();
        assert_eq!(chunks[0].label, "plot");
        assert!(chunks[0].crossref_labels.is_empty());
    }

    #[test]
    fn rejects_empty_prefixed_crossref_label_from_metadata_chunks() {
        let json = metadata(
            &serde_json::json!({
                "body":{"func":"raw","text":"plot(1)","block":false},
                "engine":"r",
                "label":"plot",
                "crossref-labels":["fig-"]
            })
            .to_string(),
        );
        let err = parse_chunks(&json, None).unwrap_err().to_string();
        assert!(err.contains("fig-"), "{err}");
        assert!(err.contains("no label name"), "{err}");
    }

    #[test]
    fn parses_qmd_style_headers_in_fenced_chunks() {
        let json = serde_json::json!([
          {"func":"raw","text":"#| out-width: 90%\n#| fig-align: right\nprint(1)","block":true,"lang":"python"}
        ])
        .to_string();
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::All,
            ..SetupDefaults::default()
        };
        let parsed = parse_chunks_with_warnings(&json, Some(setup_config_with(defaults))).unwrap();
        let chunk = &parsed.chunks[0];
        assert_eq!(chunk.label, "chunk-1");
        assert_eq!(chunk.code, "print(1)");
        assert_eq!(
            chunk.display_options.fig_width.as_ref().unwrap(),
            &Value::String("90%".to_string())
        );
        assert_eq!(
            chunk.display_options.fig_align.as_ref().unwrap(),
            &Value::String("right".to_string())
        );
        assert!(parsed
            .warnings
            .iter()
            .any(|warning| warning.contains("translated to `fig-width`")));
    }

    #[test]
    fn parses_typst_array_store_options_in_fenced_chunks() {
        let json = serde_json::json!([
          {"func":"raw","text":"#| store-get: (\"region\", \"year\")\nprint(region, year)","block":true,"lang":"python"}
        ])
        .to_string();
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::All,
            ..SetupDefaults::default()
        };

        let chunks = parse_chunks(&json, Some(setup_config_with(defaults))).unwrap();

        assert_eq!(chunks[0].exec_options.store_get, ["region", "year"]);
    }

    #[test]
    fn parses_qmd_label_in_fenced_chunks() {
        let json = serde_json::json!([
          {"func":"raw","text":"#| label: fig-fenced\n#| fig-caption: Fenced plot\nplot(1)","block":true,"lang":"r"}
        ])
        .to_string();
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::All,
            ..SetupDefaults::default()
        };
        let parsed = parse_chunks_with_warnings(&json, Some(setup_config_with(defaults))).unwrap();
        let chunk = &parsed.chunks[0];
        assert_eq!(chunk.label, "fig-fenced");
        assert_eq!(chunk.crossref_labels.len(), 1);
        assert_eq!(chunk.crossref_labels[0].kind, "fig");
        assert_eq!(chunk.crossref_labels[0].name, "fig-fenced");
        assert_eq!(chunk.code, "plot(1)");
        assert_eq!(
            chunk.display_options.fig_caption.as_deref(),
            Some("Fenced plot")
        );
    }

    #[test]
    fn parses_trailing_label_metadata_in_fenced_chunks() {
        let json = serde_json::json!([
          {"func":"raw","text":"plot(1)","block":true,"lang":"r"},
          {"func":"metadata","value":{"label":"fig-trailing"},"label":"<calepin-fence-label>"}
        ])
        .to_string();
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::All,
            ..SetupDefaults::default()
        };
        let parsed = parse_chunks_with_warnings(&json, Some(setup_config_with(defaults))).unwrap();
        let chunk = &parsed.chunks[0];
        assert_eq!(chunk.label, "fig-trailing");
        assert_eq!(chunk.crossref_labels.len(), 1);
        assert_eq!(chunk.crossref_labels[0].kind, "fig");
        assert_eq!(chunk.crossref_labels[0].name, "fig-trailing");
        assert_eq!(chunk.code, "plot(1)");
    }

    #[test]
    fn parses_trailing_label_metadata_in_wrapped_chunks() {
        let json = metadata(
            &serde_json::json!({
                "body":{
                    "func":"sequence",
                    "children":[
                        {"func":"raw","text":"plot(1)","block":true,"lang":"r"},
                        {"func":"space"},
                        {"func":"metadata","value":{"label":"fig-wrapped"},"label":"<calepin-fence-label>"}
                    ]
                },
                "engine":"r",
                "label":"fig-wrapped",
                "crossref-labels":["fig-wrapped"]
            })
            .to_string(),
        );
        let chunks = parse_chunks(&json, None).unwrap();
        let chunk = &chunks[0];
        assert_eq!(chunk.label, "fig-wrapped");
        assert_eq!(chunk.crossref_labels.len(), 1);
        assert_eq!(chunk.crossref_labels[0].kind, "fig");
        assert_eq!(chunk.crossref_labels[0].name, "fig-wrapped");
        assert_eq!(chunk.code, "plot(1)");
    }

    #[test]
    fn accepts_unprefixed_trailing_label_as_plain_id_in_fenced_chunks() {
        let json = serde_json::json!([
          {"func":"raw","text":"plot(1)","block":true,"lang":"r","label":"<plot>"}
        ])
        .to_string();
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::All,
            ..SetupDefaults::default()
        };
        // A plain trailing fence label is a chunk id, not a cross-reference.
        let chunks = parse_chunks_with_warnings(&json, Some(setup_config_with(defaults)))
            .unwrap()
            .chunks;
        assert_eq!(chunks[0].label, "plot");
        assert!(chunks[0].crossref_labels.is_empty());
    }

    #[test]
    fn rejects_qmd_and_trailing_label_conflict_in_fenced_chunks() {
        let json = serde_json::json!([
          {"func":"raw","text":"#| label: fig-qmd\nplot(1)","block":true,"lang":"r"},
          {"func":"metadata","value":{"label":"fig-trailing"},"label":"<calepin-fence-label>"}
        ])
        .to_string();
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::All,
            ..SetupDefaults::default()
        };
        let err = parse_chunks_with_warnings(&json, Some(setup_config_with(defaults)))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("both `#| label:` and trailing fence label"),
            "{err}"
        );
    }

    #[test]
    fn routes_unknown_fences_to_jupyter_when_all_fenced_chunks_enabled() {
        let json = serde_json::json!([
          {"func":"raw","text":"not executable","block":true,"lang":"text"}
        ])
        .to_string();
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::All,
            ..SetupDefaults::default()
        };
        let parsed = parse_chunks_with_warnings(&json, Some(setup_config_with(defaults))).unwrap();
        assert_eq!(parsed.chunks.len(), 1);
        assert_eq!(
            parsed.chunks[0].engine,
            EngineName::Jupyter("text".to_string())
        );
        assert_eq!(parsed.chunks[0].code, "not executable");
    }

    #[test]
    fn maps_additional_quarto_figure_options() {
        let json = metadata(
            &serde_json::json!({
                "body":{"func":"raw","text":"#| fig-device-width: 8\n#| fig-device-height: 6\n#| fig-dpi: 200\n#| fig-format: \"svg\"\n#| fig-asp: 1.5\n#| fig-link: https://example.com\n#| fig.cap: \"Figure title\"\n#| fig-alt: This is alt text\n#| fig-subcap: [A, B]\n#| fig-cap-location: top\n#| fig-align: center\n#| layout-ncol: 2\n#| layout-nrow: 1\nprint(1)","block":false},
                "engine":"python",
                "label":"mapped-quarto-options"
            })
            .to_string(),
        );
        let parsed = parse_chunks(&json, None).unwrap();
        let chunk = &parsed[0];
        assert_eq!(
            chunk.display_options.fig_caption.as_deref(),
            Some("Figure title")
        );
        assert_eq!(
            chunk.display_options.fig_alt_text.as_deref(),
            Some("This is alt text")
        );
        assert_eq!(
            chunk.display_options.fig_subcaptions.as_ref().unwrap(),
            &vec!["A".to_string(), "B".to_string()]
        );
        assert_eq!(
            chunk.display_options.fig_cap_location,
            Some(Value::String("top".to_string()))
        );
        assert_eq!(
            chunk.display_options.fig_align,
            Some(Value::String("center".to_string()))
        );
        assert_eq!(
            chunk.display_options.fig_layout_columns.as_ref().unwrap(),
            &Value::from(2)
        );
        assert_eq!(
            chunk.display_options.fig_layout_rows.as_ref().unwrap(),
            &Value::from(1)
        );
        assert_eq!(chunk.exec_options.fig_device_width, 8.0);
        assert_eq!(chunk.exec_options.fig_device_height, Some(6.0));
        assert_eq!(chunk.exec_options.fig_device_dpi, 200);
        assert_eq!(chunk.exec_options.fig_device_format, "svg");
        assert_eq!(chunk.exec_options.fig_device_aspect, 1.5);
        assert_eq!(
            chunk.display_options.fig_link,
            Some(Value::String("https://example.com".to_string()))
        );
    }

    #[test]
    fn rejects_unknown_qmd_header_option() {
        let json = metadata(
            &serde_json::json!({
                "body":{"func":"raw","text":"#| not-a-real-option: true\nprint(1)","block":false},
                "engine":"r",
                "label":"bad-header"
            })
            .to_string(),
        );
        let err = parse_chunks(&json, None).unwrap_err().to_string();
        assert!(err.contains("chunk `bad-header` header line 1"));
        assert!(err.contains("unsupported option `not-a-real-option`"));
    }

    #[test]
    fn rejects_malformed_qmd_header_option() {
        let json = metadata(
            &serde_json::json!({
                "body":{"func":"raw","text":"#| not-a-real-option true\nprint(1)","block":false},
                "engine":"r",
                "label":"bad-header-syntax"
            })
            .to_string(),
        );
        let err = parse_chunks(&json, None).unwrap_err().to_string();
        assert!(err.contains("bad-header-syntax"));
        assert!(err.contains("header line 1"));
        assert!(err.contains("malformed option declaration"));
    }

    #[test]
    fn rejects_unknown_calepin_chunk_argument() {
        let json = metadata(
            r#"{
              "body":{"func":"raw","text":"print(1)","block":false},
              "engine":"r",
              "label":"bad-argument",
              "not-a-real-argument":true
            }"#,
        );
        let err = parse_chunks(&json, None).unwrap_err().to_string();
        assert!(err.contains("chunk `bad-argument` has unsupported argument `not-a-real-argument`"));
    }

    #[test]
    fn rejects_blank_engine_name() {
        let json = metadata(
            r#"{
              "body":{"func":"raw","text":"print(1)","block":false},
              "engine":" ",
              "label":"bad-engine"
            }"#,
        );

        let err = parse_chunks(&json, None).unwrap_err().to_string();

        assert!(err.contains("engine name"), "{err}");
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn merges_setup_defaults_and_chunk_overrides() {
        let json = metadata(
            r#"{
              "body":{"func":"raw","text":"print(x)","block":false},
              "engine":"python",
              "label":"show",
              "echo":"auto",
              "eval":false,
              "output":"auto",
              "results":"render",
              "warning":"auto",
              "message":"auto",
              "error":"auto",
              "placeholder":"auto",
              "fig-device-format":"auto",
              "fig-device-dpi":"auto",
              "fig-device-width":"auto",
              "fig-device-height":"auto",
              "fig-device-aspect":"auto",
              "fig-width":"auto",
              "fig-height":"auto",
              "fig-align":"auto",
              "fig-responsive":"auto",
              "fig-link":"auto",
              "fig-caption":null,
              "fig-cap-location":"auto",
              "fig-alt-text":null,
              "fig-subcaptions":null,
              "fig-layout-columns":"auto",
              "fig-layout-rows":"auto",
              "kind":"auto"
            }"#,
        );
        let defaults = SetupDefaults {
            script: Default::default(),
            echo: false,
            eval: true,
            output: true,
            results: ResultsMode::Typst,
            results_location: ResultsLocation::Statement,
            warning: false,
            message: false,
            error: true,
            placeholder: true,
            fig_device_format: "png".to_string(),
            fig_device_dpi: 300,
            fig_device_width: 8.0,
            fig_device_height: Some(4.0),
            fig_device_aspect: 0.5,
            fig_width: Some(Value::String("70%".to_string())),
            fig_height: Some(Value::String("12cm".to_string())),
            fig_align: Some(Value::String("center".to_string())),
            fig_responsive: Some(true),
            fig_link: Some(Value::String("https://example.com/full".to_string())),
            fig_caption: Some("Default caption".to_string()),
            fig_cap_location: Some(Value::String("top".to_string())),
            fig_alt_text: Some("Default alt text".to_string()),
            fig_subcaptions: Some(vec!["Left".to_string(), "Right".to_string()]),
            fig_layout_columns: Some(Value::from(2)),
            fig_layout_rows: Some(Value::from(1)),
            tbl_caption: Some("Default table caption".to_string()),
            lst_caption: Some("Default listing caption".to_string()),
            kind: Some("figure".to_string()),
            fenced_chunks: FencedChunks::Off,
            theme: None,
        };
        let chunks = parse_chunks(&json, Some(setup_config_with(defaults))).unwrap();
        let chunk = &chunks[0];
        assert_eq!(chunk.engine, EngineName::Python);
        assert!(!chunk.exec_options.eval);
        // The chunk's explicit `results: "render"` overrides the setup default
        // (`"typst"`), matching how its explicit `eval: false` overrides above.
        assert_eq!(chunk.display_options.results, ResultsMode::Render);
        assert_eq!(chunk.exec_options.fig_device_format, "png");
        assert_eq!(chunk.exec_options.fig_device_dpi, 300);
        assert_eq!(chunk.exec_options.fig_device_width, 8.0);
        assert_eq!(chunk.exec_options.fig_device_height, Some(4.0));
        assert_eq!(chunk.exec_options.fig_device_aspect, 0.5);
        assert_eq!(chunk.display_options.fig_height.as_ref().unwrap(), "12cm");
        assert_eq!(
            chunk.display_options.fig_link.as_ref().unwrap(),
            "https://example.com/full"
        );
        // Explicit `none` in the chunk clears optional setup defaults.
        assert_eq!(chunk.display_options.fig_caption, None);
        assert_eq!(chunk.display_options.fig_alt_text, None);
        assert_eq!(chunk.display_options.fig_subcaptions, None);
        assert_eq!(
            chunk.display_options.fig_cap_location.as_ref().unwrap(),
            "top"
        );
        assert_eq!(
            chunk.display_options.fig_layout_columns,
            Some(Value::from(2))
        );
        assert_eq!(chunk.display_options.fig_layout_rows, Some(Value::from(1)));
        assert_eq!(chunk.display_options.kind.as_deref(), Some("figure"));
    }

    #[test]
    fn parses_figure_defaults_from_setup_metadata() {
        let json = setup_metadata(
            r#"{
              "fig-height":"10cm",
              "fig-link":"https://example.com/figure",
              "fig-caption":{"func":"text","text":"Overview"},
              "fig-cap-location":"bottom",
              "fig-alt-text":"Accessible overview",
              "fig-subcaptions":["First","Second"],
              "fig-layout-columns":2,
              "fig-layout-rows":[1,2],
              "kind":"figure"
            }"#,
        );

        let config = parse_setup_config(&json).unwrap().unwrap();
        let defaults = config.defaults;
        assert_eq!(defaults.fig_height.as_ref().unwrap(), "10cm");
        assert_eq!(
            defaults.fig_link.as_ref().unwrap(),
            "https://example.com/figure"
        );
        assert_eq!(defaults.fig_caption.as_deref(), Some("Overview"));
        assert_eq!(defaults.fig_cap_location.as_ref().unwrap(), "bottom");
        assert_eq!(
            defaults.fig_alt_text.as_deref(),
            Some("Accessible overview")
        );
        assert_eq!(
            defaults.fig_subcaptions.unwrap(),
            vec!["First".to_string(), "Second".to_string()]
        );
        assert_eq!(defaults.fig_layout_columns, Some(Value::from(2)));
        assert_eq!(defaults.fig_layout_rows, Some(serde_json::json!([1, 2])));
        assert_eq!(defaults.kind.as_deref(), Some("figure"));
    }

    #[test]
    fn auto_device_width_scales_from_display_width() {
        let json = metadata(
            r#"{
              "body":{"func":"raw","text":"plot(x)","block":false},
              "engine":"r",
              "label":"wide",
              "fig-device-width":"auto",
              "fig-width":"95%"
            }"#,
        );

        let chunks = parse_chunks(&json, None).unwrap();
        let chunk = &chunks[0];

        assert_eq!(chunk.display_options.fig_width.as_ref().unwrap(), "95%");
        assert_eq!(chunk.display_options.fig_align.as_ref().unwrap(), "center");
        assert_eq!(chunk.display_options.fig_responsive, Some(true));
        assert!((chunk.exec_options.fig_device_width - 8.142_857_142).abs() < 0.000_001);
    }

    #[test]
    fn setup_config_is_merged_in_order() {
        let json = r#"[
          {"func":"metadata","value":{"echo":false,"eval":false},"label":"<calepin-config>"},
          {"func":"metadata","value":{"echo":true},"label":"<calepin-config>"}
        ]"#;
        let config = parse_setup_config(json).unwrap().unwrap();

        assert!(config.defaults.echo);
        assert!(!config.defaults.eval);
    }

    #[test]
    fn rejects_setup_vars_object() {
        let json = setup_metadata(
            r#"{
              "echo":true,
              "vars":{"region":"NY","min_count":25,"active":true}
            }"#,
        );
        let err = parse_setup_config(&json).unwrap_err().to_string();
        assert!(err.contains("vars"), "{err}");
    }

    #[test]
    fn setup_without_vars_is_valid() {
        let json = setup_metadata(r#"{"echo":true}"#);
        let config = parse_setup_config(&json).unwrap().unwrap();
        assert!(config.defaults.echo);
    }

    #[test]
    fn rejects_non_object_setup_vars() {
        let json = setup_metadata(r#"{"vars":[1,2,3]}"#);
        let err = parse_setup_config(&json).unwrap_err().to_string();
        assert!(err.contains("vars"), "{err}");
    }

    #[test]
    fn rejects_non_string_fenced_chunks_list_entries() {
        let json = setup_metadata(r#"{"fenced-chunks":["python",1]}"#);
        let err = parse_setup_config(&json).unwrap_err().to_string();
        assert!(err.contains("fenced-chunks"), "{err}");
    }

    #[test]
    fn rejects_zero_fig_device_dpi() {
        let json = setup_metadata(r#"{"fig-device-dpi":0}"#);
        let err = parse_setup_config(&json).unwrap_err().to_string();
        assert!(err.contains("fig-device-dpi"), "{err}");
        assert!(err.contains("positive integer"), "{err}");
    }

    #[test]
    fn rejects_setup_lang_option() {
        let json = r#"[
          {"func":"metadata","value":{"lang":"python","echo":true},"label":"<calepin-config>"}
        ]"#;
        let err = parse_setup_config(json).unwrap_err();
        assert!(err.to_string().contains("`lang` is no longer supported"));
    }

    #[test]
    fn parses_setup_fenced_chunks_option() {
        let json = setup_metadata(
            r#"{
              "echo":true,
              "eval":true,
              "output":true,
              "results":"verbatim",
              "warning":true,
              "message":true,
              "error":false,
              "placeholder":false,
              "fig-device-format":"svg",
              "fig-device-dpi":150,
              "fig-device-width":6,
              "fig-device-height":"auto",
              "fig-device-aspect":0.618,
              "fenced-chunks":true
            }"#,
        );

        let config = parse_setup_config(&json).unwrap().unwrap();
        assert_eq!(config.defaults.fenced_chunks, FencedChunks::All);
    }

    #[test]
    fn parses_setup_fenced_chunks_single_engine() {
        let json = setup_metadata(
            r#"{
              "echo":true,
              "eval":true,
              "output":true,
              "results":"verbatim",
              "warning":true,
              "message":true,
              "error":false,
              "placeholder":false,
              "fig-device-format":"svg",
              "fig-device-dpi":150,
              "fig-device-width":6,
              "fig-device-height":"auto",
              "fig-device-aspect":0.618,
              "fenced-chunks":"python"
            }"#,
        );

        let config = parse_setup_config(&json).unwrap().unwrap();
        assert_eq!(
            config.defaults.fenced_chunks,
            FencedChunks::Only(vec!["python".to_string()])
        );
    }

    #[test]
    fn parses_setup_fenced_chunks_engine_list() {
        let json = setup_metadata(
            r#"{
              "echo":true,
              "eval":true,
              "output":true,
              "results":"verbatim",
              "warning":true,
              "message":true,
              "error":false,
              "placeholder":false,
              "fig-device-format":"svg",
              "fig-device-dpi":150,
              "fig-device-width":6,
              "fig-device-height":"auto",
              "fig-device-aspect":0.618,
              "fenced-chunks":["python","r"]
            }"#,
        );

        let config = parse_setup_config(&json).unwrap().unwrap();
        assert_eq!(
            config.defaults.fenced_chunks,
            FencedChunks::Only(vec!["python".to_string(), "r".to_string()])
        );
    }

    #[test]
    fn parses_diagram_chunk_engine() {
        let json = metadata(
            r#"{
              "body":{"func":"raw","text":"graph TD\n  A --> B","block":true},
              "engine":"mermaid",
              "label":"fig-flow"
            }"#,
        );
        let chunks = parse_chunks(&json, None).unwrap();
        assert_eq!(chunks[0].engine.as_str(), "mermaid");
        assert_eq!(chunks[0].code, "graph TD\n  A --> B");
    }

    #[test]
    fn parses_julia_chunk_engine() {
        let json = metadata(
            r#"{
              "body":{"func":"raw","text":"println(42)","block":true},
              "engine":"julia",
              "label":"julia-answer"
            }"#,
        );
        let chunks = parse_chunks(&json, None).unwrap();
        assert_eq!(chunks[0].engine, EngineName::Jupyter("julia".to_string()));
        assert_eq!(chunks[0].engine.as_str(), "julia");
        assert_eq!(chunks[0].code, "println(42)");
    }

    #[test]
    fn rejects_missing_label() {
        let json = metadata(
            r#"{"body":{"func":"raw","text":"x","block":false},"engine":"r","label":null}"#,
        );
        let err = parse_chunks(&json, None).unwrap_err().to_string();
        assert!(err.contains("missing label"));
    }

    #[test]
    fn rejects_duplicate_labels() {
        let json = r#"[
          {"func":"metadata","value":{"body":{"func":"raw","text":"x","block":false},"engine":"r","label":"dup"},"label":"<calepin-chunk>"},
          {"func":"metadata","value":{"body":{"func":"raw","text":"y","block":false},"engine":"r","label":"dup"},"label":"<calepin-chunk>"}
        ]"#;
        let err = parse_chunks(json, None).unwrap_err().to_string();
        assert!(err.contains("duplicate label `dup`"));
    }

    #[test]
    fn setup_config_rejects_invalid_results_default() {
        let json = r#"[
          {"func":"metadata","value":{"results":"bogus"},"label":"<calepin-config>"}
        ]"#;
        let err = parse_setup_config(json).unwrap_err().to_string();
        assert!(err.contains("unsupported results mode `bogus`"), "{err}");
    }

    #[test]
    fn unknown_engine_routes_to_jupyter() {
        let json = metadata(
            r#"{"body":{"func":"raw","text":"x","block":false},"engine":"ruby","label":"ch1"}"#,
        );
        let chunks = parse_chunks(&json, None).unwrap();
        assert_eq!(chunks[0].engine, EngineName::Jupyter("ruby".to_string()));
    }

    #[test]
    fn accepts_plain_language_raw_block_when_enabled() {
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::All,
            ..SetupDefaults::default()
        };
        let json = r#"[
          {"func":"raw","text":"x","block":true,"lang":"r"}
        ]"#;
        let chunks = parse_chunks(json, Some(setup_config_with(defaults))).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].engine, EngineName::R);
        assert_eq!(chunks[0].code, "x");
    }

    #[test]
    fn accepts_plain_language_raw_block_when_lang_configured() {
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::Only(vec!["python".to_string()]),
            ..SetupDefaults::default()
        };
        let json = r#"[
          {"func":"raw","text":"x","block":true,"lang":"r"},
          {"func":"raw","text":"print(1)","block":true,"lang":"python"}
        ]"#;
        let chunks = parse_chunks(json, Some(setup_config_with(defaults))).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].engine, EngineName::Python);
        assert_eq!(chunks[0].code, "print(1)");
    }

    #[test]
    fn uses_language_specific_defaults_for_raw_blocks() {
        let chunks = parse_chunks(
            r#"[
              {"func":"raw","text":"x","block":true,"lang":"r"},
              {"func":"raw","text":"print(1)","block":true,"lang":"python"}
            ]"#,
            Some(SetupConfig {
                defaults: SetupDefaults {
                    fenced_chunks: FencedChunks::Only(vec!["python".to_string()]),
                    ..SetupDefaults::default()
                },
            }),
        )
        .unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].engine, EngineName::Python);
        assert_eq!(chunks[0].code, "print(1)");
    }

    #[test]
    fn normalizes_implicit_chunk_labels_when_fenced_chunks_configured() {
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::Only(vec!["ir".to_string()]),
            ..SetupDefaults::default()
        };
        let json = r#"[
          {"func":"raw","text":"x <- 1","block":true,"lang":"ir"},
          {"func":"metadata","value":{"body":{"func":"raw","text":"print(1)","block":false},"engine":"r","label":"chunk-1"},"label":"<calepin-chunk>"}
        ]"#;
        let chunks = parse_chunks(json, Some(setup_config_with(defaults))).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].label, "chunk-1");
        assert_eq!(chunks[1].label, "chunk-2");
    }

    #[test]
    fn rejects_plain_language_raw_blocks_when_disabled() {
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::Off,
            ..SetupDefaults::default()
        };
        let json = r#"[
          {"func":"raw","text":"x","block":true,"lang":"r"}
        ]"#;
        let chunks = parse_chunks(json, Some(setup_config_with(defaults))).unwrap();
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn ignores_typst_fences_even_when_fenced_chunks_enabled() {
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::All,
            ..SetupDefaults::default()
        };
        let json = r##"[
          {"func":"raw","text":"#let x = 1","block":true,"lang":"typ"},
          {"func":"raw","text":"#let y = 2","block":true,"lang":"typst"},
          {"func":"raw","text":"print(1)","block":true,"lang":"python"}
        ]"##;
        let chunks = parse_chunks(json, Some(setup_config_with(defaults))).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].engine, EngineName::Python);
        assert_eq!(chunks[0].code, "print(1)");
    }

    #[test]
    fn parses_chunked_raw_blocks() {
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::Only(vec!["julia".to_string()]),
            ..SetupDefaults::default()
        };
        let json = r#"[
          {"func":"raw","text":"println(1)","block":true,"lang":"julia"},
          {"func":"metadata","value":{"body":{"func":"raw","text":"print(2)","block":true},"engine":"r","label":"answer"},"label":"<calepin-chunk>"}
        ]"#;
        let chunks = parse_chunks(json, Some(setup_config_with(defaults))).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].engine, EngineName::Jupyter("julia".to_string()));
        assert_eq!(chunks[0].label, "chunk-1");
        assert_eq!(chunks[0].code, "println(1)");
        assert_eq!(chunks[1].engine, EngineName::R);
        assert_eq!(chunks[1].label, "answer");
        assert_eq!(chunks[1].code, "print(2)");
    }

    #[test]
    fn metadata_query_skips_wrapper_managed_raw_builtin_blocks() {
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::All,
            ..SetupDefaults::default()
        };
        let json = r#"[
          {"func":"raw","text":"plot(1)","block":true,"lang":"r"},
          {"func":"metadata","value":{"body":{"func":"raw","text":"plot(2)","block":true},"engine":"r","label":"chunk-1"},"label":"<calepin-chunk>"}
        ]"#;
        let chunks = parse_chunks(json, Some(setup_config_with(defaults))).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].engine, EngineName::R);
        assert_eq!(chunks[0].label, "chunk-1");
        assert_eq!(chunks[0].code, "plot(2)");
    }

    #[test]
    fn metadata_query_collapses_adjacent_raw_and_jupyter_metadata() {
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::All,
            ..SetupDefaults::default()
        };
        let json = r##"[
          {"func":"raw","text":"previous = true","block":true,"lang":"toml"},
          {"func":"raw","text":"previous = true","block":true,"lang":"toml"},
          {"func":"metadata","value":{"body":{"func":"raw","text":"previous = true","block":true,"lang":"toml"},"engine":"toml","label":"chunk-1"},"label":"<calepin-chunk>"},
          {"func":"metadata","value":{"body":{"func":"raw","text":"#| eval: false\necho current","block":true,"lang":"sh"},"engine":"sh","label":"chunk-2"},"label":"<calepin-chunk>"}
        ]"##;

        let chunks = parse_chunks(json, Some(setup_config_with(defaults))).unwrap();

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].label, "chunk-1");
        assert_eq!(chunks[0].engine, EngineName::Jupyter("toml".to_string()));
        assert_eq!(chunks[0].code, "previous = true");
        assert_eq!(chunks[1].label, "chunk-2");
        assert_eq!(chunks[1].engine, EngineName::Jupyter("sh".to_string()));
        assert_eq!(chunks[1].code, "echo current");
    }

    #[test]
    fn advances_auto_labels_around_explicit_chunk_labels() {
        // Built-in fenced blocks are not auto-run alongside metadata chunks, so
        // this coexistence scenario uses a Jupyter kernel (opted in by name).
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::Only(vec!["julia".to_string()]),
            ..SetupDefaults::default()
        };
        let json = r#"[
          {"func":"raw","text":"println(1)","block":true,"lang":"julia"},
          {"func":"metadata","value":{"body":{"func":"raw","text":"println(2)","block":true},"engine":"julia","label":"chunk-1"},"label":"<calepin-chunk>"},
          {"func":"raw","text":"println(3)","block":true,"lang":"julia"}
        ]"#;
        let chunks = parse_chunks(json, Some(setup_config_with(defaults))).unwrap();
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].label, "chunk-1");
        assert_eq!(chunks[1].label, "chunk-2");
        assert_eq!(chunks[2].label, "chunk-3");
    }

    #[test]
    fn extracts_single_raw_child_from_sequence() {
        let json = metadata(
            r#"{"body":{"func":"sequence","children":[{"func":"space"},{"func":"raw","text":"x <- 1","block":false},{"func":"space"}]},"engine":"r","label":"ok"}"#,
        );
        let chunks = parse_chunks(&json, None).unwrap();
        assert_eq!(chunks[0].code, "x <- 1");
    }

    #[test]
    fn rejects_multiple_raw_children() {
        let json = metadata(
            r#"{"body":{"func":"sequence","children":[{"func":"raw","text":"x","block":false},{"func":"raw","text":"y","block":false}]},"engine":"r","label":"bad"}"#,
        );
        let err = parse_chunks(&json, None).unwrap_err().to_string();
        assert!(err.contains("exactly one raw element"));
    }

    #[test]
    fn preserves_raw_code_that_starts_with_a_decimal() {
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::Only(vec!["python3".to_string()]),
            ..SetupDefaults::default()
        };
        let json = r#"[
          {"func":"raw","text":".2\nprint(1)","block":true,"lang":"python3"}
        ]"#;

        let chunks = parse_chunks(json, Some(setup_config_with(defaults))).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].engine, EngineName::Jupyter("python3".into()));
        assert_eq!(chunks[0].code, ".2\nprint(1)");
    }

    #[test]
    fn preserves_metadata_code_that_starts_with_a_decimal() {
        let json = metadata(
            r#"{
              "body":{"func":"raw","text":".2\nprint(1)","block":true,"lang":"python3"},
              "engine":"python3",
              "label":"decimal",
              "eval":false
            }"#,
        );

        let chunks = parse_chunks(&json, None).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].engine, EngineName::Jupyter("python3".into()));
        assert_eq!(chunks[0].code, ".2\nprint(1)");
    }

    #[test]
    fn rejects_duplicate_raw_and_metadata_labels() {
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::Only(vec!["julia".to_string()]),
            ..SetupDefaults::default()
        };
        let json = r##"[
          {"func":"raw","text":"#| label: dup\nprintln(1)","block":true,"lang":"julia"},
          {"func":"metadata","value":{"body":{"func":"raw","text":"println(2)","block":true},"engine":"julia","label":"dup"},"label":"<calepin-chunk>"}
        ]"##;

        let err = parse_chunks(json, Some(setup_config_with(defaults)))
            .unwrap_err()
            .to_string();
        assert!(err.contains("duplicate label `dup`"), "{err}");
    }

    #[test]
    fn rejects_labels_with_surrounding_whitespace_or_controls() {
        for label in [" leading", "trailing ", "line\nbreak"] {
            let json = metadata(
                &serde_json::json!({
                    "body":{"func":"raw","text":"x","block":false},
                    "engine":"r",
                    "label":label
                })
                .to_string(),
            );
            assert!(parse_chunks(&json, None).is_err(), "accepted {label:?}");
        }
    }

    #[test]
    fn rejects_chunk_label_numeric_overflow() {
        let json = metadata(
            &serde_json::json!({
                "body":{"func":"raw","text":"x","block":false},
                "engine":"r",
                "label":format!("chunk-{}", usize::MAX)
            })
            .to_string(),
        );

        let err = parse_chunks(&json, None).unwrap_err().to_string();
        assert!(err.contains("supported numeric range"), "{err}");
    }
}
