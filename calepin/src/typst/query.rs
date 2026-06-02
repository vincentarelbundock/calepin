use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashSet;

use crate::typst::model::{
    default_format_order, ChunkSpec, DisplayOptions, EngineName, ExecOptions, ItemSelector,
    ResultsMode, SetupDefaults,
};

pub fn parse_chunks(query_json: &str, defaults: Option<SetupDefaults>) -> Result<Vec<ChunkSpec>> {
    let defaults = defaults.unwrap_or_default();
    let values = parse_metadata_values(query_json)
        .context("failed to parse calepin chunk metadata from typst query output")?;
    let mut seen = HashSet::new();
    let mut chunks = Vec::with_capacity(values.len());

    for (ordinal, value) in values.iter().enumerate() {
        let label = parse_label(value)?;
        if !seen.insert(label.clone()) {
            return Err(anyhow!("duplicate label `{}`", label));
        }
        let engine = parse_engine(value)?;
        let code = extract_code(value.get("body").unwrap_or(&Value::Null), &label)?;
        let exec_options = ExecOptions {
            cache: bool_option(value, "cache", defaults.cache)?,
            eval: bool_option(value, "eval", defaults.eval)?,
            error: bool_option(value, "error", defaults.error)?,
            dev: string_option(value, "dev", &defaults.dev)?,
            dpi: u32_option(value, "dpi", defaults.dpi)?,
            fig_width: f64_option(value, "fig-width", defaults.fig_width)?,
            fig_height: opt_f64_option(value, "fig-height", defaults.fig_height)?,
        };
        let display_options = DisplayOptions {
            echo: bool_option(value, "echo", defaults.echo)?,
            include: bool_option(value, "include", defaults.include)?,
            results: results_option(value, "results", &defaults.results)?,
            warning: bool_option(value, "warning", defaults.warning)?,
            message: bool_option(value, "message", defaults.message)?,
            format: format_option(value, "format", &defaults.format)?,
            item: item_option(value, "item", &defaults.item)?,
            placeholder: bool_option(value, "placeholder", defaults.placeholder)?,
            out_width: opt_string_option(value, "out-width")?,
            out_height: opt_string_option(value, "out-height")?,
            fig_cap: caption_option(value, "fig-cap")?,
            fig_alt: caption_option(value, "fig-alt")?,
            tbl_cap: caption_option(value, "tbl-cap")?,
            kind: opt_string_option(value, "kind")?,
        };
        chunks.push(ChunkSpec {
            label,
            engine,
            code,
            exec_options,
            display_options,
            ordinal,
        });
    }

    Ok(chunks)
}

pub fn parse_setup_defaults(query_json: &str) -> Result<Option<SetupDefaults>> {
    let mut values = parse_metadata_values(query_json)
        .context("failed to parse calepin setup metadata from typst query output")?;
    let Some(value) = values.pop() else {
        return Ok(None);
    };
    let defaults = SetupDefaults {
        cache: bool_option(&value, "cache", SetupDefaults::default().cache)?,
        echo: bool_option(&value, "echo", SetupDefaults::default().echo)?,
        eval: bool_option(&value, "eval", SetupDefaults::default().eval)?,
        include: bool_option(&value, "include", SetupDefaults::default().include)?,
        results: string_option(&value, "results", &SetupDefaults::default().results)?,
        warning: bool_option(&value, "warning", SetupDefaults::default().warning)?,
        message: bool_option(&value, "message", SetupDefaults::default().message)?,
        error: bool_option(&value, "error", SetupDefaults::default().error)?,
        format: format_option(&value, "format", &default_format_order())?,
        item: item_option(&value, "item", &ItemSelector::ALL)?,
        placeholder: bool_option(&value, "placeholder", SetupDefaults::default().placeholder)?,
        dev: string_option(&value, "dev", &SetupDefaults::default().dev)?,
        dpi: u32_option(&value, "dpi", SetupDefaults::default().dpi)?,
        fig_width: f64_option(&value, "fig-width", SetupDefaults::default().fig_width)?,
        fig_height: opt_f64_option(&value, "fig-height", SetupDefaults::default().fig_height)?,
    };
    Ok(Some(defaults))
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
    if raw.get("lang").is_some_and(|lang| !lang.is_null()) {
        return Err(anyhow!("chunk `{}` raw element must not declare a language", label));
    }
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
        return Err(anyhow!("chunk `{}` body must contain exactly one raw element", label));
    };

    let raw_children: Vec<&Value> = children
        .iter()
        .filter(|child| child.get("func").and_then(Value::as_str) == Some("raw"))
        .collect();
    if raw_children.len() != 1 {
        return Err(anyhow!("chunk `{}` body must contain exactly one raw element", label));
    }

    for child in children {
        if child.get("func").and_then(Value::as_str) == Some("raw") {
            continue;
        }
        if !is_whitespace_node(child) {
            return Err(anyhow!("chunk `{}` body contains extra non-whitespace markup", label));
        }
    }

    Ok(raw_children[0])
}

fn is_whitespace_node(node: &Value) -> bool {
    matches!(
        node.get("func").and_then(Value::as_str),
        Some("space") | Some("linebreak")
    ) || node.get("text").and_then(Value::as_str).is_some_and(|s| s.trim().is_empty())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::model::{EngineName, ItemSelector, ResultsMode, SetupDefaults};

    fn metadata(value: &str) -> String {
        format!(r#"[{{"func":"metadata","value":{value},"label":"<calepin-chunk>"}}]"#)
    }

    #[test]
    fn parse_valid_chunk() {
        let json = metadata(
            r#"{
              "body":{"func":"raw","text":"x <- 1","block":false},
              "code":"x <- 1",
              "engine":"r",
              "label":"setup",
              "cache":"auto",
              "echo":false,
              "eval":"auto",
              "include":"auto",
              "results":"auto",
              "warning":"auto",
              "message":"auto",
              "error":"auto",
              "format":"auto",
              "item":"auto",
              "placeholder":"auto",
              "dev":"auto",
              "dpi":"auto",
              "fig-width":"auto",
              "fig-height":"auto",
              "out-width":"auto",
              "out-height":"auto",
              "fig-cap":{"func":"text","text":"Caption"},
              "fig-alt":null,
              "tbl-cap":null,
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
        assert_eq!(chunk.display_options.fig_cap.as_deref(), Some("Caption"));
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
              "cache":"auto",
              "eval":false,
              "include":"auto",
              "results":"auto",
              "warning":"auto",
              "message":"auto",
              "error":"auto",
              "format":"auto",
              "item":"auto",
              "placeholder":"auto",
              "dev":"auto",
              "dpi":"auto",
              "fig-width":"auto",
              "fig-height":"auto",
              "out-width":"auto",
              "out-height":"auto",
              "fig-cap":null,
              "fig-alt":null,
              "tbl-cap":null,
              "kind":"auto"
            }"#,
        );
        let defaults = SetupDefaults {
            cache: false,
            echo: false,
            eval: true,
            include: true,
            results: "asis".to_string(),
            warning: false,
            message: false,
            error: true,
            format: vec!["text/plain".to_string()],
            item: ItemSelector::LAST,
            placeholder: true,
            dev: "png".to_string(),
            dpi: 300,
            fig_width: 8.0,
            fig_height: Some(4.0),
        };
        let chunks = parse_chunks(&json, Some(defaults)).unwrap();
        let chunk = &chunks[0];
        assert_eq!(chunk.engine, EngineName::Python);
        assert!(!chunk.exec_options.cache);
        assert!(!chunk.exec_options.eval);
        assert_eq!(chunk.display_options.results, ResultsMode::Asis);
        assert_eq!(chunk.exec_options.dev, "png");
        assert_eq!(chunk.exec_options.dpi, 300);
    }

    #[test]
    fn rejects_missing_label() {
        let json = metadata(r#"{"body":{"func":"raw","text":"x","block":false},"engine":"r","label":null}"#);
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
    fn rejects_unsupported_engine() {
        let json = metadata(r#"{"body":{"func":"raw","text":"x","block":false},"engine":"ruby","label":"bad"}"#);
        let err = parse_chunks(&json, None).unwrap_err().to_string();
        assert!(err.contains("unsupported engine `ruby`"));
    }

    #[test]
    fn rejects_language_tagged_raw_block() {
        let json = metadata(r#"{"body":{"func":"raw","text":"x","block":true,"lang":"r"},"engine":"r","label":"bad"}"#);
        let err = parse_chunks(&json, None).unwrap_err().to_string();
        assert!(err.contains("must not declare a language"));
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
}
