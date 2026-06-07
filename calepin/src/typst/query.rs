use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashSet;

use crate::typst::chunk_options::{
    parse_chunk_body_with_qmd_header, parse_chunk_source_with_qmd_header, validate_chunk_arguments,
};
use crate::typst::crossref::parse_label_names;
use crate::typst::model::{
    ChunkSpec, CrossrefLabelDoc, DisplayOptions, EngineName, ExecOptions, FencedChunks,
    ResultsMode, SetupDefaults,
};

#[allow(dead_code)]
pub fn parse_chunks(query_json: &str, defaults: Option<SetupConfig>) -> Result<Vec<ChunkSpec>> {
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
    let mut raw_labels = HashSet::new();
    let mut chunks = Vec::with_capacity(values.len());
    let mut auto_label_index = 1usize;
    let mut warnings = Vec::new();

    for (ordinal, value) in values.iter().enumerate() {
        if is_raw_code_block(value) {
            if let Some(chunk) = parse_chunk_raw_block(
                value,
                &config,
                &mut seen,
                &mut auto_label_index,
                ordinal,
                has_chunk_metadata,
                &mut warnings,
            )? {
                raw_labels.insert(chunk.label.clone());
                chunks.push(chunk);
            }
            continue;
        }

        if is_calepin_chunk_metadata(value) {
            let value = value
                .get("value")
                .context("chunk metadata is missing `value` field")?;
            let label = parse_label(value)?;
            let label = normalize_chunk_label(label.as_str(), &mut seen, &mut auto_label_index);
            if !seen.insert(label.clone()) {
                if raw_labels.remove(&label) {
                    chunks.retain(|chunk| chunk.label != label);
                } else {
                    return Err(anyhow!("duplicate label `{}`", label));
                }
            }
            parse_chunk_metadata(value, &config, &label, &mut chunks, ordinal, &mut warnings)?;
            bump_auto_label(&mut auto_label_index, &label);
        }
    }

    Ok(ChunkParseResult { chunks, warnings })
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetupConfig {
    pub defaults: SetupDefaults,
}

impl Default for SetupConfig {
    fn default() -> Self {
        Self {
            defaults: SetupDefaults::default(),
        }
    }
}

fn is_calepin_chunk_metadata(value: &Value) -> bool {
    value.get("func") == Some(&Value::String("metadata".into()))
        && value
            .get("label")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "<calepin-chunk>")
}

fn is_raw_code_block(value: &Value) -> bool {
    value.get("func") == Some(&Value::String("raw".into()))
        && value.get("block").and_then(Value::as_bool) == Some(true)
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

    let (code, chunk_options, mut header_warnings) =
        parse_chunk_body_with_qmd_header(value.get("body").unwrap_or(&Value::Null), label)?;
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
    let fig_width = raw_option(&value, "fig-width").or_else(|| defaults.fig_width.clone());
    let fig_align = raw_option(&value, "fig-align").or_else(|| defaults.fig_align.clone());
    let fig_responsive = opt_bool_option(&value, "fig-responsive")?.or(defaults.fig_responsive);
    let exec_options = parse_exec_options(&value, defaults, &fig_width)?;
    let display_options =
        parse_display_options(&value, defaults, fig_width, fig_align, fig_responsive)?;
    let crossref_labels = parse_crossref_labels(&value)
        .map_err(|err| anyhow!("invalid cross-reference labels for chunk `{label}`: {err}"))?;
    chunks.push(ChunkSpec {
        label: label.to_string(),
        engine,
        code,
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
) -> String {
    let Some(label_index) = parse_chunk_label_index(label) else {
        return label.to_string();
    };

    if label_index < *auto_label_index {
        let next_label = format!("chunk-{auto_label_index}");
        if seen.contains(&next_label) {
            return next_available_label(seen, auto_label_index);
        }
        return next_label;
    }

    label.to_string()
}

fn parse_chunk_raw_block(
    value: &Value,
    config: &SetupConfig,
    seen: &mut HashSet<String>,
    auto_label_index: &mut usize,
    ordinal: usize,
    has_chunk_metadata: bool,
    warnings: &mut Vec<String>,
) -> Result<Option<ChunkSpec>> {
    let Some(lang) = value.get("lang").and_then(Value::as_str) else {
        return Ok(None);
    };
    if is_typst_fence(lang) {
        return Ok(None);
    }
    let Some(engine) = EngineName::parse(lang).ok() else {
        return Ok(None);
    };
    let raw_text = value.get("text").and_then(Value::as_str).unwrap_or("");
    let label_hint = format!("chunk-{}", *auto_label_index);
    let (raw_code, chunk_options, mut header_warnings) =
        parse_chunk_source_with_qmd_header(raw_text, &label_hint)?;
    warnings.append(&mut header_warnings);
    let mut value_with_options = value
        .as_object()
        .cloned()
        .ok_or_else(|| anyhow!("fenced chunk is not an object"))?;
    for (key, value) in chunk_options {
        value_with_options.insert(key, value);
    }
    let value = Value::Object(value_with_options);
    let raw_text = raw_code;

    // Reattach any version suffix Typst split from the lang identifier
    // (e.g. ```julia-1.2 → lang="julia-1", text=".2\n...").
    let (engine, code) = reattach_version_suffix(engine, lang, &raw_text);
    // Allow if the original lang OR the reattached engine name is permitted.
    let defaults = &config.defaults;
    if !defaults.fenced_chunks.allows(lang) && !defaults.fenced_chunks.allows(engine.as_str()) {
        return Ok(None);
    }
    if matches!(defaults.fenced_chunks, FencedChunks::All)
        && matches!(engine, EngineName::Jupyter(_))
    {
        return Ok(None);
    }
    if has_chunk_metadata && !matches!(engine, EngineName::Jupyter(_)) {
        return Ok(None);
    }
    let (label, crossref_labels) = if let Some(label_value) = value_for(&value, "label") {
        let names = label_names_from_value(label_value)?;
        let labels = parse_label_names(&names)
            .map_err(|err| anyhow!("invalid cross-reference labels for fenced chunk: {err}"))?;
        let label = names
            .first()
            .cloned()
            .ok_or_else(|| anyhow!("fenced chunk label list is empty"))?;
        if !seen.insert(label.clone()) {
            return Err(anyhow!("duplicate label `{}`", label));
        }
        bump_auto_label(auto_label_index, &label);
        (
            label,
            labels
                .into_iter()
                .map(|label| label.to_doc())
                .collect::<Vec<_>>(),
        )
    } else {
        let label = next_available_label(seen, auto_label_index);
        seen.insert(label.clone());
        (label, vec![])
    };
    let fig_width = raw_option(&value, "fig-width").or_else(|| defaults.fig_width.clone());
    let fig_align = raw_option(&value, "fig-align").or_else(|| defaults.fig_align.clone());
    let fig_responsive = opt_bool_option(&value, "fig-responsive")?.or(defaults.fig_responsive);
    let exec_options = parse_exec_options(&value, defaults, &fig_width)?;
    let display_options =
        parse_display_options(&value, defaults, fig_width, fig_align, fig_responsive)?;
    Ok(Some(ChunkSpec {
        label,
        engine,
        code,
        exec_options,
        display_options,
        ordinal,
        crossref_labels,
    }))
}

fn is_typst_fence(lang: &str) -> bool {
    matches!(lang, "typ" | "typst")
}

fn bump_auto_label(auto_label_index: &mut usize, label: &str) {
    let Some(suffix) = label.strip_prefix("chunk-") else {
        return;
    };
    if let Ok(idx) = suffix.parse::<usize>() {
        *auto_label_index = (*auto_label_index).max(idx + 1);
    }
}

fn next_available_label(seen: &mut HashSet<String>, counter: &mut usize) -> String {
    while seen.contains(&format!("chunk-{counter}")) {
        *counter += 1;
    }
    let label = format!("chunk-{counter}");
    *counter += 1;
    label
}

fn parse_query_values(query_json: &str) -> Result<Vec<Value>> {
    serde_json::from_str(query_json).context("failed to parse typst query output")
}

pub fn parse_setup_config(query_json: &str) -> Result<Option<SetupConfig>> {
    let values = parse_metadata_values(query_json)
        .context("failed to parse calepin setup metadata from typst query output")?;
    if values.is_empty() {
        return Ok(None);
    }

    let mut config = SetupConfig::default();
    for value in values {
        if value.get("lang").is_some() {
            return Err(anyhow!(
                "`lang` is no longer supported in #calepin.setup; use a single setup call for document-wide defaults"
            ));
        }
        config.defaults = parse_setup_defaults(&value, &config.defaults)?;
    }

    Ok(Some(config))
}

fn parse_exec_options(
    value: &Value,
    defaults: &SetupDefaults,
    fig_width: &Option<Value>,
) -> Result<ExecOptions> {
    Ok(ExecOptions {
        eval: bool_option(value, "eval", defaults.eval)?,
        error: bool_option(value, "error", defaults.error)?,
        fig_device_format: string_option(value, "fig-device-format", &defaults.fig_device_format)?,
        fig_device_dpi: u32_option(value, "fig-device-dpi", defaults.fig_device_dpi)?,
        fig_device_width: fig_device_width_option(
            value,
            "fig-device-width",
            defaults.fig_device_width,
            fig_width,
            &defaults.fig_width,
        )?,
        fig_device_height: opt_f64_option(value, "fig-device-height", defaults.fig_device_height)?,
        fig_device_aspect: f64_option(value, "fig-device-aspect", defaults.fig_device_aspect)?,
    })
}

fn parse_display_options(
    value: &Value,
    defaults: &SetupDefaults,
    fig_width: Option<Value>,
    fig_align: Option<Value>,
    fig_responsive: Option<bool>,
) -> Result<DisplayOptions> {
    Ok(DisplayOptions {
        echo: bool_option(value, "echo", defaults.echo)?,
        output: bool_option(value, "output", defaults.output)?,
        results: results_option(value, "results", &defaults.results)?,
        warning: bool_option(value, "warning", defaults.warning)?,
        message: bool_option(value, "message", defaults.message)?,
        placeholder: bool_option(value, "placeholder", defaults.placeholder)?,
        fig_width,
        fig_height: raw_option(value, "fig-height"),
        fig_align,
        fig_responsive,
        fig_link: raw_option(value, "fig-link"),
        fig_caption: caption_option(value, "fig-caption")?,
        fig_cap_location: raw_option(value, "fig-cap-location"),
        fig_alt_text: caption_option(value, "fig-alt-text")?,
        fig_subcaptions: caption_list_option(value, "fig-subcaptions")?,
        fig_layout_columns: raw_option(value, "fig-layout-columns"),
        fig_layout_rows: raw_option(value, "fig-layout-rows"),
        kind: opt_string_option(value, "kind")?,
    })
}

fn parse_setup_defaults(value: &Value, base: &SetupDefaults) -> Result<SetupDefaults> {
    Ok(SetupDefaults {
        echo: bool_option(value, "echo", base.echo)?,
        eval: bool_option(value, "eval", base.eval)?,
        output: bool_option(value, "output", base.output)?,
        results: string_option(value, "results", &base.results)?,
        warning: bool_option(value, "warning", base.warning)?,
        message: bool_option(value, "message", base.message)?,
        error: bool_option(value, "error", base.error)?,
        placeholder: bool_option(value, "placeholder", base.placeholder)?,
        fig_device_format: string_option(value, "fig-device-format", &base.fig_device_format)?,
        fig_device_dpi: u32_option(value, "fig-device-dpi", base.fig_device_dpi)?,
        fig_device_width: f64_option(value, "fig-device-width", base.fig_device_width)?,
        fig_device_height: opt_f64_option(value, "fig-device-height", base.fig_device_height)?,
        fig_device_aspect: f64_option(value, "fig-device-aspect", base.fig_device_aspect)?,
        fig_width: raw_option(value, "fig-width").or_else(|| base.fig_width.clone()),
        fig_align: raw_option(value, "fig-align").or_else(|| base.fig_align.clone()),
        fig_responsive: opt_bool_option(value, "fig-responsive")?.or(base.fig_responsive),
        fenced_chunks: fenced_chunks_option(value, &base.fenced_chunks)?,
    })
}

/// Parse the unified `fenced-chunks` setting: `false`/absent, `true` (all engines),
/// an engine name, or a list of engine names.
fn fenced_chunks_option(value: &Value, base: &FencedChunks) -> Result<FencedChunks> {
    match value.get("fenced-chunks") {
        None | Some(Value::Null) => Ok(base.clone()),
        Some(Value::Bool(false)) => Ok(FencedChunks::Off),
        Some(Value::Bool(true)) => Ok(FencedChunks::All),
        Some(Value::String(lang)) => Ok(FencedChunks::Only(vec![lang.clone()])),
        Some(Value::Array(items)) => Ok(FencedChunks::Only(
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect(),
        )),
        Some(other) => Err(anyhow!("invalid `fenced-chunks` value: {other}")),
    }
}

fn parse_metadata_values(query_json: &str) -> Result<Vec<Value>> {
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

fn parse_label(value: &Value) -> Result<String> {
    let Some(label) = value.get("label").and_then(Value::as_str) else {
        return Err(anyhow!("missing label"));
    };
    if label.trim().is_empty() {
        return Err(anyhow!("missing label"));
    }
    Ok(label.to_string())
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

    parse_label_names(&names).map(|labels| labels.into_iter().map(|label| label.to_doc()).collect())
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

fn value_for<'a>(object: &'a Value, key: &str) -> Option<&'a Value> {
    let value = object.get(key)?;
    if is_auto(value) || value.is_null() {
        None
    } else {
        Some(value)
    }
}

fn is_auto(value: &Value) -> bool {
    value.as_str() == Some("auto")
}

fn bool_option(object: &Value, key: &str, default: bool) -> Result<bool> {
    match value_for(object, key) {
        None => Ok(default),
        Some(value) => value
            .as_bool()
            .ok_or_else(|| anyhow!("`{}` must be a boolean", key)),
    }
}

fn string_option(object: &Value, key: &str, default: &str) -> Result<String> {
    match value_for(object, key) {
        None => Ok(default.to_string()),
        Some(value) => value
            .as_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow!("`{}` must be a string", key)),
    }
}

fn opt_string_option(object: &Value, key: &str) -> Result<Option<String>> {
    match value_for(object, key) {
        None => Ok(None),
        Some(value) => value
            .as_str()
            .map(|s| Some(s.to_string()))
            .ok_or_else(|| anyhow!("`{}` must be a string", key)),
    }
}

fn raw_option(object: &Value, key: &str) -> Option<Value> {
    value_for(object, key).cloned()
}

fn opt_bool_option(object: &Value, key: &str) -> Result<Option<bool>> {
    match value_for(object, key) {
        None => Ok(None),
        Some(value) => value
            .as_bool()
            .map(Some)
            .ok_or_else(|| anyhow!("`{}` must be a boolean", key)),
    }
}

fn u32_option(object: &Value, key: &str, default: u32) -> Result<u32> {
    match value_for(object, key) {
        None => Ok(default),
        Some(value) => value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| anyhow!("`{}` must be a positive integer", key)),
    }
}

fn f64_option(object: &Value, key: &str, default: f64) -> Result<f64> {
    match value_for(object, key) {
        None => Ok(default),
        Some(value) => value
            .as_f64()
            .ok_or_else(|| anyhow!("`{}` must be a number", key)),
    }
}

fn fig_device_width_option(
    object: &Value,
    key: &str,
    default: f64,
    fig_width: &Option<Value>,
    default_fig_width: &Option<Value>,
) -> Result<f64> {
    match value_for(object, key) {
        Some(value) => value
            .as_f64()
            .ok_or_else(|| anyhow!("`{}` must be a number", key)),
        None => Ok(derived_fig_device_width(
            default,
            fig_width,
            default_fig_width,
        )),
    }
}

fn derived_fig_device_width(
    default: f64,
    fig_width: &Option<Value>,
    default_fig_width: &Option<Value>,
) -> f64 {
    let Some(display_ratio) = display_width_ratio(fig_width) else {
        return default;
    };
    let Some(default_ratio) = display_width_ratio(default_fig_width) else {
        return default;
    };
    if default_ratio <= 0.0 {
        return default;
    }
    if (display_ratio - default_ratio).abs() < f64::EPSILON {
        return default;
    }
    default * display_ratio / default_ratio
}

fn display_width_ratio(value: &Option<Value>) -> Option<f64> {
    match value.as_ref()? {
        Value::Number(number) => number.as_f64(),
        Value::String(value) => {
            let trimmed = value.trim();
            if let Some(percent) = trimmed.strip_suffix('%') {
                percent
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .map(|value| value / 100.0)
            } else {
                trimmed.parse::<f64>().ok()
            }
        }
        _ => None,
    }
}

fn opt_f64_option(object: &Value, key: &str, default: Option<f64>) -> Result<Option<f64>> {
    match value_for(object, key) {
        None => Ok(default),
        Some(value) => value
            .as_f64()
            .map(Some)
            .ok_or_else(|| anyhow!("`{}` must be a number", key)),
    }
}

fn results_option(object: &Value, key: &str, default: &str) -> Result<ResultsMode> {
    let value = string_option(object, key, default)?;
    ResultsMode::parse(&value)
}
fn caption_option(object: &Value, key: &str) -> Result<Option<String>> {
    let Some(value) = value_for(object, key) else {
        return Ok(None);
    };
    extract_text(value)
        .map(Some)
        .ok_or_else(|| anyhow!("`{}` must be text content or a string", key))
}

fn caption_list_option(object: &Value, key: &str) -> Result<Option<Vec<String>>> {
    let Some(value) = value_for(object, key) else {
        return Ok(None);
    };
    if let Some(array) = value.as_array() {
        let mut captions = Vec::with_capacity(array.len());
        for item in array {
            captions.push(
                extract_text(item)
                    .ok_or_else(|| anyhow!("`{}` array values must be text content", key))?,
            );
        }
        return Ok(Some(captions));
    }
    extract_text(value)
        .map(|caption| Some(vec![caption]))
        .ok_or_else(|| anyhow!("`{}` must be text content or an array", key))
}

fn extract_text(value: &Value) -> Option<String> {
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

/// Typst's raw-block fence parser stops the `lang` identifier at the first
/// period, so ` ```julia-1.2 ` produces `lang="julia-1"` and a code text
/// that begins with `".2\n"`.  This function detects that pattern and
/// re-attaches the split suffix to the engine name while stripping it from
/// the code.
///
/// The heuristic is conservative: the leading line must consist solely of a
/// dot followed by one or more groups of `.<digits>` (a pure version suffix
/// like `.2`, `.11`, `.2.1`).  Anything else is left untouched.
fn reattach_version_suffix(engine: EngineName, lang: &str, raw_text: &str) -> (EngineName, String) {
    // Typst may include the fence line's trailing newline as the first byte
    // of the text field.  For the period-split case (```julia-1.12 →
    // lang="julia-1") the remainder ".12" may appear either right at the
    // start or after that leading newline.  Strip it before inspecting.
    let text = raw_text.strip_prefix('\n').unwrap_or(raw_text);

    // Only relevant for Jupyter kernels (unknown engine names).
    if !matches!(engine, EngineName::Jupyter(_)) {
        return (engine, text.to_string());
    }

    // The text must start with a version-like suffix (".N[.N...]") on its
    // own line.
    let (first_line, rest) = match text.split_once('\n') {
        Some((f, r)) => (f, r),
        // No newline: the entire (stripped) text is the first line.
        None => (text, ""),
    };

    // The first line must look like ".<digits>[.<digits>]*" – pure version.
    let is_version_suffix = first_line.starts_with('.')
        && first_line.len() > 1
        && first_line[1..]
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()));

    if !is_version_suffix {
        return (engine, text.to_string());
    }

    // Reconstruct the full kernel name (lang + the suffix that Typst split off).
    let full_name = format!("{}{}", lang, first_line);
    let new_engine = EngineName::Jupyter(full_name);
    (new_engine, rest.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::model::{EngineName, ResultsMode, SetupDefaults};

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
    fn rejects_unprefixed_crossref_labels_from_metadata_chunks() {
        let json = metadata(
            &serde_json::json!({
                "body":{"func":"raw","text":"plot(1)","block":false},
                "engine":"r",
                "label":"plot",
                "crossref-labels":["plot"]
            })
            .to_string(),
        );
        let err = parse_chunks(&json, None).unwrap_err().to_string();
        assert!(
            err.contains("invalid cross-reference labels for chunk `plot`"),
            "{err}"
        );
        assert!(err.contains("plot"), "{err}");
        assert!(err.contains("fig-"), "{err}");
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
    fn ignores_unknown_fences_when_all_fenced_chunks_enabled() {
        let json = serde_json::json!([
          {"func":"raw","text":"not executable","block":true,"lang":"text"}
        ])
        .to_string();
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::All,
            ..SetupDefaults::default()
        };
        let parsed = parse_chunks_with_warnings(&json, Some(setup_config_with(defaults))).unwrap();
        assert!(parsed.chunks.is_empty());
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
            Some(Value::String("https://example.com/main".to_string()))
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
            echo: false,
            eval: true,
            output: true,
            results: "typst".to_string(),
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
            fig_align: Some(Value::String("center".to_string())),
            fig_responsive: Some(true),
            fenced_chunks: FencedChunks::Off,
        };
        let chunks = parse_chunks(&json, Some(setup_config_with(defaults))).unwrap();
        let chunk = &chunks[0];
        assert_eq!(chunk.engine, EngineName::Python);
        assert!(!chunk.exec_options.eval);
        assert_eq!(chunk.display_options.results, ResultsMode::Typst);
        assert_eq!(chunk.exec_options.fig_device_format, "png");
        assert_eq!(chunk.exec_options.fig_device_dpi, 300);
        assert_eq!(chunk.exec_options.fig_device_width, 8.0);
        assert_eq!(chunk.exec_options.fig_device_height, Some(4.0));
        assert_eq!(chunk.exec_options.fig_device_aspect, 0.5);
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

        assert_eq!(config.defaults.echo, true);
        assert_eq!(config.defaults.eval, false);
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
        let chunks = parse_chunks(&json, Some(setup_config_with(defaults))).unwrap();
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
        let chunks = parse_chunks(&json, Some(setup_config_with(defaults))).unwrap();
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
    fn rejects_plain_language_raw_blocks_by_default() {
        let json = r#"[
          {"func":"raw","text":"x","block":true,"lang":"r"}
        ]"#;
        let chunks = parse_chunks(&json, None).unwrap();
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
        let chunks = parse_chunks(&json, Some(setup_config_with(defaults))).unwrap();
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
    fn advances_auto_labels_around_explicit_chunk_labels() {
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::All,
            ..SetupDefaults::default()
        };
        let json = r#"[
          {"func":"raw","text":"print(1)","block":true,"lang":"python"},
          {"func":"metadata","value":{"body":{"func":"raw","text":"print(2)","block":true},"engine":"r","label":"chunk-1"},"label":"<calepin-chunk>"},
          {"func":"raw","text":"print(3)","block":true,"lang":"python"}
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

    // reattach_version_suffix ---------------------------------------------------

    #[test]
    fn reattaches_minor_version_suffix_no_leading_newline() {
        // Typst puts ".2" at the very start of the text (no leading newline).
        let (engine, code) = reattach_version_suffix(
            EngineName::Jupyter("julia-1".into()),
            "julia-1",
            ".2\nx = 41\nprint(x + 1)",
        );
        assert_eq!(engine, EngineName::Jupyter("julia-1.2".into()));
        assert_eq!(code, "x = 41\nprint(x + 1)");
    }

    #[test]
    fn reattaches_minor_version_suffix_with_leading_newline() {
        // Typst may include the fence line's trailing newline before ".2".
        let (engine, code) = reattach_version_suffix(
            EngineName::Jupyter("julia-1".into()),
            "julia-1",
            "\n.2\nx = 41\nprint(x + 1)",
        );
        assert_eq!(engine, EngineName::Jupyter("julia-1.2".into()));
        assert_eq!(code, "x = 41\nprint(x + 1)");
    }

    #[test]
    fn reattaches_two_part_version_suffix() {
        // ```some-kernel-1.2.3 → lang="some-kernel-1", text=".2.3\ncode"
        let (engine, code) = reattach_version_suffix(
            EngineName::Jupyter("some-kernel-1".into()),
            "some-kernel-1",
            ".2.3\ncode",
        );
        assert_eq!(engine, EngineName::Jupyter("some-kernel-1.2.3".into()));
        assert_eq!(code, "code");
    }

    #[test]
    fn no_reattach_when_first_line_is_not_version() {
        // A leading newline with normal code (no version suffix to strip).
        let (engine, code) =
            reattach_version_suffix(EngineName::Jupyter("julia-1".into()), "julia-1", "\nx = 41");
        assert_eq!(engine, EngineName::Jupyter("julia-1".into()));
        assert_eq!(code, "x = 41");
    }

    #[test]
    fn no_reattach_for_builtin_engine() {
        // Built-in engines like Python are not Jupyter and must not be touched.
        let (engine, code) = reattach_version_suffix(EngineName::Python, "python", ".2\nprint(1)");
        assert_eq!(engine, EngineName::Python);
        assert_eq!(code, ".2\nprint(1)");
    }

    #[test]
    fn raw_block_with_period_in_lang_parsed_correctly() {
        // Simulate what Typst produces for ```julia-1.2\nx = 41\n```
        // (lang truncated to "julia-1", ".2\n" prepended to text).
        let defaults = SetupDefaults {
            fenced_chunks: FencedChunks::All,
            ..SetupDefaults::default()
        };
        let json = r#"[
          {"func":"raw","text":".2\nx = 41\nprint(x + 1)","block":true,"lang":"julia-1"}
        ]"#;
        let chunks = parse_chunks(json, Some(setup_config_with(defaults))).unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].engine, EngineName::Jupyter("julia-1.2".into()));
        assert_eq!(chunks[0].code, "x = 41\nprint(x + 1)");
    }
}
