use crate::types::{ChunkOptions, OptionValue};

/// Parse the chunk header to extract a label and any inline options.
/// Accepts `{r}`, `{r, label}`, and `{r, label, key=value, ...}`.
/// Key=value options in the header are converted to pipe-equivalent options
/// (dots in keys become dashes, values are normalized).
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
            // Convert key=value to pipe-style "key: value"
            if let Some((key, value)) = part.split_once('=') {
                let key = key.trim().replace('.', "-");
                let value = value.trim();
                let value = value.trim_matches('"').trim_matches('\'');
                header_opts.push(format!("{}: {}", key, value));
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

/// Parse pipe comment options (`#|` lines) in YAML format: `#| key: value`.
/// Dashes in keys are normalized to dots internally (e.g., `fig-width` → `fig.width`).
/// Dots in option names are not accepted — use dashes instead.
/// The `label` key is rejected here — labels belong in the chunk header.
pub fn parse_pipe_options(lines: &[&str]) -> ChunkOptions {
    let mut opts = ChunkOptions::default();
    for line in lines {
        let line = line.trim();
        if let Some((key, value)) = line.split_once(':') {
            let raw_key = key.trim();
            let value = value.trim();
            if raw_key.contains('.') {
                let dashed = raw_key.replace('.', "-");
                cwarn!("use dashes in option names: #| {}: {}", dashed, value);
            }
            let key = raw_key.replace('-', ".");
            if key == "label" {
                cwarn!(
                    "Warning: 'label' cannot be set with #| pipe syntax. \
                     Use the chunk header instead: ```{{r, {}}}`",
                    value
                );
                continue;
            }
            opts.inner.insert(key, parse_value(value));
        }
    }
    opts
}

/// Parse a single value string into OptionValue.
fn parse_value(s: &str) -> OptionValue {
    let s = s.trim();
    match s {
        "TRUE" | "true" | "yes" => OptionValue::Bool(true),
        "FALSE" | "false" | "no" => OptionValue::Bool(false),
        "NULL" | "null" | "~" => OptionValue::Null,
        _ => {
            // Try number (reject non-finite values like NaN, inf)
            if let Ok(n) = s.parse::<f64>() {
                if n.is_finite() {
                    return OptionValue::Number(n);
                }
            }
            // Strip quotes if present
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
    fn test_yaml_pipe_options() {
        let lines = vec!["eval: true", "echo: false", "fig-width: 10"];
        let opts = parse_pipe_options(&lines);
        assert!(opts.eval());
        assert!(!opts.echo());
        assert!((opts.fig_width() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pipe_label_rejected() {
        let lines = vec!["label: setup", "echo: false"];
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
        assert!(opts.inner.contains_key("fig.width"));
    }

    #[test]
    fn test_header_label_with_kv_converted() {
        let (label, opts) = parse_header_label(", setup, echo=FALSE");
        assert_eq!(label, Some("setup".to_string()));
        assert!(!opts.echo());
    }
}
