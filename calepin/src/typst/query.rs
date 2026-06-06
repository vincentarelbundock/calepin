use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

use crate::typst::model::{
    ChunkSpec, DisplayOptions, EngineName, ExecOptions, ItemSelector, RawChunks, ResultsMode,
    SetupDefaults,
};

pub fn parse_chunks(query_json: &str, defaults: Option<SetupConfig>) -> Result<Vec<ChunkSpec>> {
    let config = defaults.unwrap_or_default();
    let values = parse_query_values(query_json)
        .context("failed to parse calepin chunk metadata from typst query output")?;
    let mut seen = HashSet::new();
    let mut chunks = Vec::with_capacity(values.len());
    let mut auto_label_index = 1usize;

    for (ordinal, value) in values.iter().enumerate() {
        if is_raw_code_block(value) {
            if let Some(chunk) =
                parse_chunk_raw_block(value, &config, &mut seen, &mut auto_label_index, ordinal)?
            {
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
                return Err(anyhow!("duplicate label `{}`", label));
            }
            parse_chunk_metadata(value, &config, &label, &mut chunks, ordinal)?;
            bump_auto_label(&mut auto_label_index, &label);
        }
    }

    Ok(chunks)
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetupConfig {
    pub defaults: SetupDefaults,
    pub lang_defaults: BTreeMap<String, SetupDefaults>,
}

impl Default for SetupConfig {
    fn default() -> Self {
        Self {
            defaults: SetupDefaults::default(),
            lang_defaults: BTreeMap::new(),
        }
    }
}

impl SetupConfig {
    fn defaults_for_engine<'a>(&'a self, engine: &EngineName) -> &'a SetupDefaults {
        self.lang_defaults
            .get(engine.as_str())
            .unwrap_or(&self.defaults)
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
) -> Result<()> {
    let engine = parse_engine(value)?;
    let defaults = config.defaults_for_engine(&engine);
    let code = extract_code(value.get("body").unwrap_or(&Value::Null), label)?;
    let fig_display_width =
        raw_option(value, "fig-display-width").or_else(|| defaults.fig_display_width.clone());
    let fig_display_align =
        raw_option(value, "fig-display-align").or_else(|| defaults.fig_display_align.clone());
    let fig_display_responsive =
        opt_bool_option(value, "fig-display-responsive")?.or(defaults.fig_display_responsive);
    let exec_options = parse_exec_options(value, defaults, &fig_display_width)?;
    let display_options = parse_display_options(
        value,
        defaults,
        fig_display_width,
        fig_display_align,
        fig_display_responsive,
    )?;
    chunks.push(ChunkSpec {
        label: label.to_string(),
        engine,
        code,
        exec_options,
        display_options,
        ordinal,
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
) -> Result<Option<ChunkSpec>> {
    let Some(lang) = value.get("lang").and_then(Value::as_str) else {
        return Ok(None);
    };
    let Some(engine) = EngineName::parse(lang).ok() else {
        return Ok(None);
    };
    let raw_text = value.get("text").and_then(Value::as_str).unwrap_or("");
    // Reattach any version suffix Typst split from the lang identifier
    // (e.g. ```julia-1.2 → lang="julia-1", text=".2\n...").
    let (engine, code) = reattach_version_suffix(engine, lang, raw_text);
    // Allow if the original lang OR the reattached engine name is permitted.
    let defaults = config.defaults_for_engine(&engine);
    if !defaults.raw_chunks.allows(lang) && !defaults.raw_chunks.allows(engine.as_str()) {
        return Ok(None);
    }
    let label = next_available_label(seen, auto_label_index);
    seen.insert(label.clone());
    let fig_display_width = defaults.fig_display_width.clone();
    let fig_display_align = defaults.fig_display_align.clone();
    let fig_display_responsive = defaults.fig_display_responsive;
    let exec_options = parse_exec_options(value, defaults, &fig_display_width)?;
    let display_options = parse_display_options(
        value,
        defaults,
        fig_display_width,
        fig_display_align,
        fig_display_responsive,
    )?;
    Ok(Some(ChunkSpec {
        label,
        engine,
        code,
        exec_options,
        display_options,
        ordinal,
    }))
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
        if let Some(lang) = opt_string_option(&value, "lang")? {
            let lang = EngineName::parse(&lang)?.as_str().to_string();
            let base = config
                .lang_defaults
                .get(&lang)
                .unwrap_or(&config.defaults)
                .clone();
            config
                .lang_defaults
                .insert(lang, parse_setup_defaults(&value, &base)?);
        } else {
            config.defaults = parse_setup_defaults(&value, &config.defaults)?;
        }
    }

    Ok(Some(config))
}

fn parse_exec_options(
    value: &Value,
    defaults: &SetupDefaults,
    fig_display_width: &Option<Value>,
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
            fig_display_width,
            &defaults.fig_display_width,
        )?,
        fig_device_height: opt_f64_option(value, "fig-device-height", defaults.fig_device_height)?,
        fig_device_aspect: f64_option(value, "fig-device-aspect", defaults.fig_device_aspect)?,
    })
}

fn parse_display_options(
    value: &Value,
    defaults: &SetupDefaults,
    fig_display_width: Option<Value>,
    fig_display_align: Option<Value>,
    fig_display_responsive: Option<bool>,
) -> Result<DisplayOptions> {
    Ok(DisplayOptions {
        echo: bool_option(value, "echo", defaults.echo)?,
        output: bool_option(value, "output", defaults.output)?,
        results: results_option(value, "results", &defaults.results)?,
        warning: bool_option(value, "warning", defaults.warning)?,
        message: bool_option(value, "message", defaults.message)?,
        format: format_option(value, "format", &defaults.format)?,
        item: item_option(value, "item", &defaults.item)?,
        placeholder: bool_option(value, "placeholder", defaults.placeholder)?,
        fig_display_width,
        fig_display_height: raw_option(value, "fig-display-height"),
        fig_display_align,
        fig_display_responsive,
        fig_display_link: raw_option(value, "fig-display-link"),
        fig_caption: caption_option(value, "fig-caption")?,
        fig_caption_position: raw_option(value, "fig-caption-position"),
        fig_alt_text: caption_option(value, "fig-alt-text")?,
        fig_subcaptions: caption_list_option(value, "fig-subcaptions")?,
        fig_layout_columns: raw_option(value, "fig-layout-columns"),
        fig_layout_rows: raw_option(value, "fig-layout-rows"),
        fig_layout_design: raw_option(value, "fig-layout-design"),
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
        format: format_option(value, "format", &base.format)?,
        item: item_option(value, "item", &base.item)?,
        placeholder: bool_option(value, "placeholder", base.placeholder)?,
        fig_device_format: string_option(value, "fig-device-format", &base.fig_device_format)?,
        fig_device_dpi: u32_option(value, "fig-device-dpi", base.fig_device_dpi)?,
        fig_device_width: f64_option(value, "fig-device-width", base.fig_device_width)?,
        fig_device_height: opt_f64_option(value, "fig-device-height", base.fig_device_height)?,
        fig_device_aspect: f64_option(value, "fig-device-aspect", base.fig_device_aspect)?,
        fig_display_width: raw_option(value, "fig-display-width")
            .or_else(|| base.fig_display_width.clone()),
        fig_display_align: raw_option(value, "fig-display-align")
            .or_else(|| base.fig_display_align.clone()),
        fig_display_responsive: opt_bool_option(value, "fig-display-responsive")?
            .or(base.fig_display_responsive),
        raw_chunks: raw_chunks_option(value, &base.raw_chunks)?,
    })
}

/// Parse the unified `raw-chunks` setting: `false`/absent, `true` (all engines),
/// an engine name, or a list of engine names.
fn raw_chunks_option(value: &Value, base: &RawChunks) -> Result<RawChunks> {
    match value.get("raw-chunks") {
        None | Some(Value::Null) => Ok(base.clone()),
        Some(Value::Bool(false)) => Ok(RawChunks::Off),
        Some(Value::Bool(true)) => Ok(RawChunks::All),
        Some(Value::String(lang)) => Ok(RawChunks::Only(vec![lang.clone()])),
        Some(Value::Array(items)) => Ok(RawChunks::Only(
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect(),
        )),
        Some(other) => Err(anyhow!("invalid `raw-chunks` value: {other}")),
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

fn extract_code(body: &Value, label: &str) -> Result<String> {
    let raw = extract_raw_node(body, label)?;
    raw.get("text")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow!("chunk `{}` raw element is missing text", label))
}

fn extract_raw_node<'a>(node: &'a Value, label: &str) -> Result<&'a Value> {
    if node.get("func").and_then(Value::as_str) == Some("raw") {
        return Ok(node);
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

    for child in children {
        if child.get("func").and_then(Value::as_str) == Some("raw") {
            continue;
        }
        if !is_whitespace_node(child) {
            return Err(anyhow!(
                "chunk `{}` body contains extra non-whitespace markup",
                label
            ));
        }
    }

    Ok(raw_children[0])
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
    fig_display_width: &Option<Value>,
    default_fig_display_width: &Option<Value>,
) -> Result<f64> {
    match value_for(object, key) {
        Some(value) => value
            .as_f64()
            .ok_or_else(|| anyhow!("`{}` must be a number", key)),
        None => Ok(derived_fig_device_width(
            default,
            fig_display_width,
            default_fig_display_width,
        )),
    }
}

fn derived_fig_device_width(
    default: f64,
    fig_display_width: &Option<Value>,
    default_fig_display_width: &Option<Value>,
) -> f64 {
    let Some(display_ratio) = display_width_ratio(fig_display_width) else {
        return default;
    };
    let Some(default_ratio) = display_width_ratio(default_fig_display_width) else {
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

fn format_option(object: &Value, key: &str, default: &[String]) -> Result<Vec<String>> {
    match value_for(object, key) {
        None => Ok(default.to_vec()),
        Some(value) => {
            if let Some(s) = value.as_str() {
                return Ok(vec![s.to_string()]);
            }
            let Some(array) = value.as_array() else {
                return Err(anyhow!("`{}` must be a string, array, or auto", key));
            };
            array
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(ToOwned::to_owned)
                        .ok_or_else(|| anyhow!("`{}` array values must be strings", key))
                })
                .collect()
        }
    }
}

fn item_option(object: &Value, key: &str, default: &ItemSelector) -> Result<ItemSelector> {
    match value_for(object, key) {
        None => Ok(default.clone()),
        Some(value) => ItemSelector::parse(value),
    }
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
        && first_line[1..].split('.').all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()));

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
    use crate::typst::model::{EngineName, ItemSelector, ResultsMode, SetupDefaults};

    fn metadata(value: &str) -> String {
        format!(r#"[{{"func":"metadata","value":{value},"label":"<calepin-chunk>"}}]"#)
    }

    fn setup_metadata(value: &str) -> String {
        format!(r#"[{{"func":"metadata","value":{value},"label":"<calepin-config>"}}]"#)
    }

    fn setup_config_with(defaults: SetupDefaults) -> SetupConfig {
        SetupConfig {
            defaults,
            lang_defaults: BTreeMap::new(),
        }
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
              "results":"auto",
              "warning":"auto",
              "message":"auto",
              "error":"auto",
              "format":"auto",
              "item":"auto",
              "placeholder":"auto",
              "fig-device-format":"auto",
              "fig-device-dpi":"auto",
              "fig-device-width":"auto",
              "fig-device-height":"auto",
              "fig-device-aspect":"auto",
              "fig-display-width":"70%",
              "fig-display-height":"auto",
              "fig-display-align":"center",
              "fig-display-responsive":true,
              "fig-display-link":"https://example.com",
              "fig-caption":{"func":"text","text":"Caption"},
              "fig-caption-position":"top",
              "fig-alt-text":null,
              "fig-subcaptions":["A","B"],
              "fig-layout-columns":[1,1],
              "fig-layout-rows":"auto",
              "fig-layout-design":"A B",
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
    fn merges_setup_defaults_and_chunk_overrides() {
        let json = metadata(
            r#"{
              "body":{"func":"raw","text":"print(x)","block":false},
              "engine":"python",
              "label":"show",
              "echo":"auto",
              "eval":false,
              "output":"auto",
              "results":"auto",
              "warning":"auto",
              "message":"auto",
              "error":"auto",
              "format":"auto",
              "item":"auto",
              "placeholder":"auto",
              "fig-device-format":"auto",
              "fig-device-dpi":"auto",
              "fig-device-width":"auto",
              "fig-device-height":"auto",
              "fig-device-aspect":"auto",
              "fig-display-width":"auto",
              "fig-display-height":"auto",
              "fig-display-align":"auto",
              "fig-display-responsive":"auto",
              "fig-display-link":"auto",
              "fig-caption":null,
              "fig-caption-position":"auto",
              "fig-alt-text":null,
              "fig-subcaptions":null,
              "fig-layout-columns":"auto",
              "fig-layout-rows":"auto",
              "fig-layout-design":"auto",
              "kind":"auto"
            }"#,
        );
        let defaults = SetupDefaults {
            echo: false,
            eval: true,
            output: true,
            results: "asis".to_string(),
            warning: false,
            message: false,
            error: true,
            format: vec!["text/plain".to_string()],
            item: ItemSelector::LAST,
            placeholder: true,
            fig_device_format: "png".to_string(),
            fig_device_dpi: 300,
            fig_device_width: 8.0,
            fig_device_height: Some(4.0),
            fig_device_aspect: 0.5,
            fig_display_width: Some(Value::String("70%".to_string())),
            fig_display_align: Some(Value::String("center".to_string())),
            fig_display_responsive: Some(true),
            raw_chunks: RawChunks::Off,
        };
        let chunks = parse_chunks(&json, Some(setup_config_with(defaults))).unwrap();
        let chunk = &chunks[0];
        assert_eq!(chunk.engine, EngineName::Python);
        assert!(!chunk.exec_options.eval);
        assert_eq!(chunk.display_options.results, ResultsMode::Asis);
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
              "fig-display-width":"95%"
            }"#,
        );

        let chunks = parse_chunks(&json, None).unwrap();
        let chunk = &chunks[0];

        assert_eq!(
            chunk.display_options.fig_display_width.as_ref().unwrap(),
            "95%"
        );
        assert_eq!(
            chunk.display_options.fig_display_align.as_ref().unwrap(),
            "center"
        );
        assert_eq!(chunk.display_options.fig_display_responsive, Some(true));
        assert!((chunk.exec_options.fig_device_width - 8.142_857_142).abs() < 0.000_001);
    }

    #[test]
    fn parses_setup_language_specific_defaults() {
        let json = r#"[
          {"func":"metadata","value":{"echo":false,"eval":false},"label":"<calepin-config>"},
          {"func":"metadata","value":{"lang":"python","echo":true},"label":"<calepin-config>"}
        ]"#;
        let config = parse_setup_config(json).unwrap().unwrap();

        assert_eq!(config.defaults.echo, false);
        assert_eq!(config.defaults.eval, false);
        assert_eq!(
            config.lang_defaults.get("python").unwrap().echo,
            true
        );
        assert_eq!(config.lang_defaults.get("python").unwrap().eval, false);
    }

    #[test]
    fn parses_setup_raw_chunks_option() {
        let json = setup_metadata(
            r#"{
              "echo":true,
              "eval":true,
              "output":true,
              "results":"verbatim",
              "warning":true,
              "message":true,
              "error":false,
              "format":"auto",
              "item":"all",
              "placeholder":false,
              "fig-device-format":"svg",
              "fig-device-dpi":150,
              "fig-device-width":6,
              "fig-device-height":"auto",
              "fig-device-aspect":0.618,
              "raw-chunks":true
            }"#,
        );

        let config = parse_setup_config(&json).unwrap().unwrap();
        assert_eq!(config.defaults.raw_chunks, RawChunks::All);
    }

    #[test]
    fn parses_setup_raw_chunks_single_engine() {
        let json = setup_metadata(
            r#"{
              "echo":true,
              "eval":true,
              "output":true,
              "results":"verbatim",
              "warning":true,
              "message":true,
              "error":false,
              "format":"auto",
              "item":"all",
              "placeholder":false,
              "fig-device-format":"svg",
              "fig-device-dpi":150,
              "fig-device-width":6,
              "fig-device-height":"auto",
              "fig-device-aspect":0.618,
              "raw-chunks":"python"
            }"#,
        );

        let config = parse_setup_config(&json).unwrap().unwrap();
        assert_eq!(
            config.defaults.raw_chunks,
            RawChunks::Only(vec!["python".to_string()])
        );
    }

    #[test]
    fn parses_setup_raw_chunks_engine_list() {
        let json = setup_metadata(
            r#"{
              "echo":true,
              "eval":true,
              "output":true,
              "results":"verbatim",
              "warning":true,
              "message":true,
              "error":false,
              "format":"auto",
              "item":"all",
              "placeholder":false,
              "fig-device-format":"svg",
              "fig-device-dpi":150,
              "fig-device-width":6,
              "fig-device-height":"auto",
              "fig-device-aspect":0.618,
              "raw-chunks":["python","r"]
            }"#,
        );

        let config = parse_setup_config(&json).unwrap().unwrap();
        assert_eq!(
            config.defaults.raw_chunks,
            RawChunks::Only(vec!["python".to_string(), "r".to_string()])
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
            raw_chunks: RawChunks::All,
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
            raw_chunks: RawChunks::Only(vec!["python".to_string()]),
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
        let mut lang_defaults = BTreeMap::new();
        lang_defaults.insert(
            "python".to_string(),
            SetupDefaults {
                raw_chunks: RawChunks::Only(vec!["python".to_string()]),
                ..SetupDefaults::default()
            },
        );
        let chunks = parse_chunks(
            r#"[
              {"func":"raw","text":"x","block":true,"lang":"r"},
              {"func":"raw","text":"print(1)","block":true,"lang":"python"}
            ]"#,
            Some(SetupConfig {
                defaults: SetupDefaults {
                    raw_chunks: RawChunks::Off,
                    ..SetupDefaults::default()
                },
                lang_defaults,
            }),
        )
        .unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].engine, EngineName::Python);
        assert_eq!(chunks[0].code, "print(1)");
    }

    #[test]
    fn normalizes_implicit_chunk_labels_when_raw_chunks_configured() {
        let defaults = SetupDefaults {
            raw_chunks: RawChunks::Only(vec!["r".to_string()]),
            ..SetupDefaults::default()
        };
        let json = r#"[
          {"func":"raw","text":"x <- 1","block":true,"lang":"r"},
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
    fn parses_chunked_raw_blocks() {
        let defaults = SetupDefaults {
            raw_chunks: RawChunks::All,
            ..SetupDefaults::default()
        };
        let json = r#"[
          {"func":"raw","text":"print(1)","block":true,"lang":"python"},
          {"func":"metadata","value":{"body":{"func":"raw","text":"print(2)","block":true},"engine":"r","label":"answer"},"label":"<calepin-chunk>"}
        ]"#;
        let chunks = parse_chunks(json, Some(setup_config_with(defaults))).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].engine, EngineName::Python);
        assert_eq!(chunks[0].label, "chunk-1");
        assert_eq!(chunks[0].code, "print(1)");
        assert_eq!(chunks[1].engine, EngineName::R);
        assert_eq!(chunks[1].label, "answer");
        assert_eq!(chunks[1].code, "print(2)");
    }

    #[test]
    fn advances_auto_labels_around_explicit_chunk_labels() {
        let defaults = SetupDefaults {
            raw_chunks: RawChunks::All,
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
        let (engine, code) = reattach_version_suffix(
            EngineName::Jupyter("julia-1".into()),
            "julia-1",
            "\nx = 41",
        );
        assert_eq!(engine, EngineName::Jupyter("julia-1".into()));
        assert_eq!(code, "x = 41");
    }

    #[test]
    fn no_reattach_for_builtin_engine() {
        // Built-in engines like Python are not Jupyter and must not be touched.
        let (engine, code) =
            reattach_version_suffix(EngineName::Python, "python", ".2\nprint(1)");
        assert_eq!(engine, EngineName::Python);
        assert_eq!(code, ".2\nprint(1)");
    }

    #[test]
    fn raw_block_with_period_in_lang_parsed_correctly() {
        // Simulate what Typst produces for ```julia-1.2\nx = 41\n```
        // (lang truncated to "julia-1", ".2\n" prepended to text).
        let defaults = SetupDefaults {
            raw_chunks: RawChunks::All,
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
