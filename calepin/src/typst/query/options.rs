use anyhow::{anyhow, Context, Result};
use serde_json::Value;

use super::value::{extract_text, parse_metadata_values, value_for};
use crate::typst::model::{DisplayOptions, ExecOptions, FencedChunks, ResultsMode, SetupDefaults};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SetupConfig {
    pub defaults: SetupDefaults,
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

pub(super) fn parse_chunk_options(
    value: &Value,
    defaults: &SetupDefaults,
) -> Result<(ExecOptions, DisplayOptions)> {
    let fig_width = raw_option(value, "fig-width").or_else(|| defaults.fig_width.clone());
    let fig_align = raw_option(value, "fig-align").or_else(|| defaults.fig_align.clone());
    let fig_responsive = opt_bool_option(value, "fig-responsive")?.or(defaults.fig_responsive);
    let exec_options = parse_exec_options(value, defaults, &fig_width)?;
    let display_options =
        parse_display_options(value, defaults, fig_width, fig_align, fig_responsive)?;
    Ok((exec_options, display_options))
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
        results: results_option(value, "results", defaults.results)?,
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
        results: results_option(value, "results", base.results)?,
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
        params: params_option(value, "params", &base.params)?,
        theme: raw_option(value, "theme").or_else(|| base.theme.clone()),
    })
}

fn params_option(value: &Value, key: &str, base: &Value) -> Result<Value> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(base.clone()),
        Some(Value::Object(map)) if map.is_empty() => Ok(base.clone()),
        Some(Value::Object(map)) => Ok(Value::Object(map.clone())),
        Some(_) => Err(anyhow!("`{}` must be a dictionary", key)),
    }
}

fn fenced_chunks_option(value: &Value, base: &FencedChunks) -> Result<FencedChunks> {
    match value.get("fenced-chunks") {
        None | Some(Value::Null) => Ok(base.clone()),
        Some(Value::Bool(false)) => Ok(FencedChunks::Off),
        Some(Value::Bool(true)) => Ok(FencedChunks::All),
        Some(Value::String(lang)) => Ok(FencedChunks::Only(vec![lang.clone()])),
        Some(Value::Array(items)) => {
            let mut langs = Vec::with_capacity(items.len());
            for item in items {
                let Some(lang) = item.as_str() else {
                    return Err(anyhow!("`fenced-chunks` array entries must be strings"));
                };
                langs.push(lang.to_string());
            }
            Ok(FencedChunks::Only(langs))
        }
        Some(other) => Err(anyhow!("invalid `fenced-chunks` value: {other}")),
    }
}

fn bool_option(object: &Value, key: &str, default: bool) -> Result<bool> {
    option_or(object, key, default, Value::as_bool, "a boolean")
}

fn string_option(object: &Value, key: &str, default: &str) -> Result<String> {
    option_or(
        object,
        key,
        default.to_string(),
        value_to_string,
        "a string",
    )
}

fn opt_string_option(object: &Value, key: &str) -> Result<Option<String>> {
    optional_option(object, key, value_to_string, "a string")
}

fn raw_option(object: &Value, key: &str) -> Option<Value> {
    value_for(object, key).cloned()
}

fn opt_bool_option(object: &Value, key: &str) -> Result<Option<bool>> {
    optional_option(object, key, Value::as_bool, "a boolean")
}

fn u32_option(object: &Value, key: &str, default: u32) -> Result<u32> {
    option_or(
        object,
        key,
        default,
        |value| {
            value
                .as_u64()
                .filter(|n| *n > 0)
                .and_then(|n| u32::try_from(n).ok())
        },
        "a positive integer",
    )
}

fn f64_option(object: &Value, key: &str, default: f64) -> Result<f64> {
    option_or(object, key, default, Value::as_f64, "a number")
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
    optional_option(object, key, Value::as_f64, "a number").map(|value| value.or(default))
}

fn results_option(object: &Value, key: &str, default: ResultsMode) -> Result<ResultsMode> {
    let Some(value) = value_for(object, key) else {
        return Ok(default);
    };
    let value = value
        .as_str()
        .ok_or_else(|| anyhow!("`{}` must be a string", key))?;
    ResultsMode::parse(value)
}

fn option_or<T>(
    object: &Value,
    key: &str,
    default: T,
    parse: impl Fn(&Value) -> Option<T>,
    expected: &str,
) -> Result<T> {
    match value_for(object, key) {
        None => Ok(default),
        Some(value) => parse(value).ok_or_else(|| anyhow!("`{}` must be {}", key, expected)),
    }
}

fn optional_option<T>(
    object: &Value,
    key: &str,
    parse: impl Fn(&Value) -> Option<T>,
    expected: &str,
) -> Result<Option<T>> {
    match value_for(object, key) {
        None => Ok(None),
        Some(value) => parse(value)
            .map(Some)
            .ok_or_else(|| anyhow!("`{}` must be {}", key, expected)),
    }
}

fn value_to_string(value: &Value) -> Option<String> {
    value.as_str().map(ToOwned::to_owned)
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
