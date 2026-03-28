use crate::types::{ChunkOptions, OptionValue};

/// Parse the chunk header to extract a label and any inline options.
/// Accepts `{r}`, `{r, label}`, and `{r, label, key=value, ...}`.
/// Key=value options in the header are converted to TOML-equivalent options
/// (keys are normalized to underscored lowercase, values are normalized).
///
/// Returns `(label_or_none, converted_options)`.
pub fn parse_header_label(s: &str) -> (Option<String>, ChunkOptions) {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return (None, ChunkOptions::default());
    }
    // Strip leading comma
    let trimmed = trimmed.strip_prefix(',').unwrap_or(trimmed).trim();
    if trimmed.is_empty() {
        return (None, ChunkOptions::default());
    }

    let parts: Vec<String> = split_csv(trimmed);
    let mut label: Option<String> = None;
    let mut header_opts: Vec<String> = Vec::new();

    for part in &parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part.contains('=') {
            // Convert key=value to TOML-style "key = value"
            if let Some((key, value)) = part.split_once('=') {
                let key = crate::util::normalize_key(key.trim());
                let value = value.trim();
                let value = value.trim_matches('"').trim_matches('\'');
                header_opts.push(format!("{} = {}", key, toml_quote(value)));
            }
        } else if label.is_none() {
            label = Some(part.to_string());
        }
    }

    let opts = if header_opts.is_empty() {
        ChunkOptions::default()
    } else {
        let lines: Vec<&str> = header_opts.iter().map(|s| s.as_str()).collect();
        parse_pipe_options(&lines)
    };

    (label, opts)
}

/// Parse pipe comment options (`#|` lines) as TOML key-value pairs.
/// For Quarto compatibility, lines using `key: value` syntax are converted
/// to `key = value` before parsing.
/// Keys are normalized to underscored lowercase internally (e.g., `fig-width` -> `fig_width`).
/// The `label` key is rejected here -- labels belong in the chunk header.
pub fn parse_pipe_options(lines: &[&str]) -> ChunkOptions {
    let mut opts = ChunkOptions::default();

    // Convert all lines to TOML format and join into a single TOML document
    let toml_lines: Vec<String> = lines
        .iter()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            Some(pipe_line_to_toml(line))
        })
        .collect();

    let toml_str = toml_lines.join("\n");
    if toml_str.is_empty() {
        return opts;
    }

    match toml_str.parse::<toml::Table>() {
        Ok(table) => {
            for (raw_key, value) in table {
                if raw_key.contains('.') {
                    let underscored = raw_key.replace('.', "-");
                    cwarn!("use underscores or dashes in option names: #| {} = ...", underscored);
                }
                let key = crate::util::normalize_key(&raw_key);
                if key == "label" {
                    cwarn!(
                        "Warning: 'label' cannot be set with #| pipe syntax. \
                         Use the chunk header instead: ```{{r, {}}}`",
                        value
                    );
                    continue;
                }
                opts.inner.insert(key, toml_to_option_value(value));
            }
        }
        Err(e) => {
            cwarn!("failed to parse chunk options as TOML: {}", e);
            // Fall back to line-by-line parsing
            for line in lines {
                let line = line.trim();
                if let Some((key, value)) = line.split_once(':') {
                    let raw_key = key.trim();
                    let key = crate::util::normalize_key(raw_key);
                    opts.inner.insert(key, parse_value_fallback(value.trim()));
                } else if let Some((key, value)) = line.split_once('=') {
                    let raw_key = key.trim();
                    let key = crate::util::normalize_key(raw_key);
                    opts.inner.insert(key, parse_value_fallback(value.trim()));
                }
            }
        }
    }

    opts
}

/// Convert a pipe comment line to TOML syntax.
/// `key: value` becomes `key = value` (with quoting if needed).
/// Lines already using `=` are passed through.
fn pipe_line_to_toml(line: &str) -> String {
    // If line already contains `=`, treat as TOML
    if line.contains('=') {
        let (key, value) = line.split_once('=').unwrap();
        let key = crate::util::normalize_key(key.trim());
        let value = value.trim();
        return format!("{} = {}", key, toml_ensure_valid(value));
    }
    // Otherwise, convert `key: value` to `key = value`
    if let Some((key, value)) = line.split_once(':') {
        let key = crate::util::normalize_key(key.trim());
        let value = value.trim();
        format!("{} = {}", key, toml_ensure_valid(value))
    } else {
        line.to_string()
    }
}

/// Ensure a value is valid TOML. Bare strings need quoting; booleans and
/// numbers can stay bare.
fn toml_ensure_valid(value: &str) -> String {
    // Already quoted
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        return value.to_string();
    }
    // TOML booleans
    if value == "true" || value == "false" {
        return value.to_string();
    }
    // Quarto-style booleans: normalize to TOML
    match value {
        "TRUE" | "yes" => return "true".to_string(),
        "FALSE" | "no" => return "false".to_string(),
        "NULL" | "null" | "~" => return "false".to_string(), // no null in TOML; treat as false
        _ => {}
    }
    // Number
    if value.parse::<f64>().map_or(false, |n| n.is_finite()) {
        return value.to_string();
    }
    // Bare string: quote it for TOML
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Quote a value for TOML (used when converting header options).
fn toml_quote(value: &str) -> String {
    toml_ensure_valid(value)
}

/// Convert a `toml::Value` to our `OptionValue`.
fn toml_to_option_value(v: toml::Value) -> OptionValue {
    match v {
        toml::Value::Boolean(b) => OptionValue::Bool(b),
        toml::Value::Integer(i) => OptionValue::Number(i as f64),
        toml::Value::Float(f) => OptionValue::Number(f),
        toml::Value::String(s) => OptionValue::String(s),
        other => OptionValue::String(other.to_string()),
    }
}

/// Fallback value parser when TOML parsing fails.
fn parse_value_fallback(s: &str) -> OptionValue {
    let s = s.trim();
    match s {
        "TRUE" | "true" | "yes" => OptionValue::Bool(true),
        "FALSE" | "false" | "no" => OptionValue::Bool(false),
        "NULL" | "null" | "~" => OptionValue::Null,
        _ => {
            if let Ok(n) = s.parse::<f64>() {
                if n.is_finite() {
                    return OptionValue::Number(n);
                }
            }
            let unquoted = if (s.starts_with('"') && s.ends_with('"'))
                || (s.starts_with('\'') && s.ends_with('\''))
            {
                &s[1..s.len() - 1]
            } else {
                s
            };
            OptionValue::String(unquoted.to_string())
        }
    }
}

/// Split a CSV string respecting quoted values.
fn split_csv(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut quote_char = '"';

    for ch in s.chars() {
        if in_quote {
            current.push(ch);
            if ch == quote_char {
                in_quote = false;
            }
        } else if ch == '"' || ch == '\'' {
            in_quote = true;
            quote_char = ch;
            current.push(ch);
        } else if ch == ',' {
            parts.push(std::mem::take(&mut current));
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toml_pipe_options() {
        let lines = vec!["eval = true", "echo = false", "fig-width = 10"];
        let opts = parse_pipe_options(&lines);
        assert!(opts.eval());
        assert!(!opts.echo());
        assert!((opts.fig_width() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_yaml_compat_pipe_options() {
        // Quarto-style colon syntax should still work
        let lines = vec!["eval: true", "echo: false", "fig-width: 5"];
        let opts = parse_pipe_options(&lines);
        assert!(opts.eval());
        assert!(!opts.echo());
        assert!((opts.fig_width() - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pipe_label_rejected() {
        let lines = vec!["label = \"setup\"", "echo = false"];
        let opts = parse_pipe_options(&lines);
        // label should be ignored (not stored)
        assert!(opts.get_opt_string("label").is_none());
        // other options should still work
        assert!(!opts.echo());
    }

    #[test]
    fn test_header_label_only() {
        let (label, opts) = parse_header_label(", setup");
        assert_eq!(label, Some("setup".to_string()));
        assert!(opts.inner.is_empty());
    }

    #[test]
    fn test_header_empty() {
        let (label, opts) = parse_header_label("");
        assert!(label.is_none());
        assert!(opts.inner.is_empty());
    }

    #[test]
    fn test_header_kv_options_converted() {
        let (label, opts) = parse_header_label(", echo=FALSE, fig.width=8");
        assert!(label.is_none());
        assert!(!opts.echo());
        assert!(opts.inner.contains_key("fig_width"));
    }

    #[test]
    fn test_header_label_with_kv_converted() {
        let (label, opts) = parse_header_label(", setup, echo=FALSE");
        assert_eq!(label, Some("setup".to_string()));
        assert!(!opts.echo());
    }

    #[test]
    fn test_quarto_booleans_normalized() {
        let lines = vec!["echo: TRUE", "eval: FALSE"];
        let opts = parse_pipe_options(&lines);
        assert!(opts.inner.contains_key("echo"));
        assert!(opts.inner.contains_key("eval"));
    }

    #[test]
    fn test_string_values_preserved() {
        let lines = vec!["fig-cap: \"My figure\""];
        let opts = parse_pipe_options(&lines);
        assert_eq!(opts.get_opt_string("fig_cap"), Some("My figure".to_string()));
    }
}
