//! Calepin's internal value type.
//!
//! Replaces saphyr's `YamlOwned` as the common representation for
//! front matter, plugin manifests, and configuration values.
//! Both TOML and minimal YAML parse into this type.

use std::collections::HashMap;
use indexmap::IndexMap;

/// An ordered map of string keys to values.
pub type Table = IndexMap<String, Value>;

/// A generic configuration/metadata value.
#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<Value>),
    Table(Table),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Value::Integer(n) => Some(*n),
            Value::Float(f) => Some(*f as i64),
            _ => None,
        }
    }

    pub fn as_floating_point(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Integer(n) => Some(*n as f64),
            _ => None,
        }
    }

    pub fn as_table(&self) -> Option<&Table> {
        match self {
            Value::Table(t) => Some(t),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Look up a key in a table value.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Table(t) => table_get(t, key),
            _ => None,
        }
    }
}

/// Look up a key in a table.
pub fn table_get<'a>(table: &'a Table, key: &str) -> Option<&'a Value> {
    table.get(key)
}

/// Get an optional string from a table.
pub fn table_str(table: &Table, key: &str) -> Option<String> {
    table_get(table, key).and_then(|v| v.as_str()).map(String::from)
}

/// Get a bool from a table (default false).
pub fn table_bool(table: &Table, key: &str) -> bool {
    table_get(table, key)
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Parse a value that is either a string or an array of strings.
pub fn value_string_list(v: &Value) -> Vec<String> {
    if let Some(s) = v.as_str() {
        return vec![s.to_string()];
    }
    if let Some(arr) = v.as_array() {
        return arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
    }
    vec![]
}

// ---------------------------------------------------------------------------
// Conversion from toml::Value
// ---------------------------------------------------------------------------

/// Convert a `toml::Value` into our `Value`.
pub fn from_toml(tv: toml::Value) -> Value {
    match tv {
        toml::Value::String(s) => Value::String(s),
        toml::Value::Integer(n) => Value::Integer(n),
        toml::Value::Float(f) => Value::Float(f),
        toml::Value::Boolean(b) => Value::Bool(b),
        toml::Value::Datetime(dt) => Value::String(dt.to_string()),
        toml::Value::Array(arr) => Value::Array(arr.into_iter().map(from_toml).collect()),
        toml::Value::Table(map) => {
            Value::Table(map.into_iter().map(|(k, v)| (k, from_toml(v))).collect())
        }
    }
}

/// Convert a `toml::Table` into our `Table`.
pub fn table_from_toml(map: toml::map::Map<String, toml::Value>) -> Table {
    map.into_iter().map(|(k, v)| (k, from_toml(v))).collect()
}

// ---------------------------------------------------------------------------
// Conversion to serde_json::Value
// ---------------------------------------------------------------------------

/// Convert a `Value` to `serde_json::Value` (for Jinja context).
pub fn to_json(val: &Value) -> serde_json::Value {
    match val {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Integer(n) => serde_json::json!(n),
        Value::Float(f) => serde_json::json!(f),
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Array(arr) => serde_json::Value::Array(arr.iter().map(to_json).collect()),
        Value::Table(table) => {
            let mut map = serde_json::Map::new();
            for (k, v) in table {
                map.insert(k.clone(), to_json(v));
            }
            serde_json::Value::Object(map)
        }
    }
}

// ---------------------------------------------------------------------------
// Coerce / build / merge helpers (for CLI overrides)
// ---------------------------------------------------------------------------

/// Coerce a string value into the appropriate typed Value.
/// "true"/"false" -> Bool, integer -> Integer, float -> Float, otherwise -> String.
pub fn coerce_value(s: &str) -> Value {
    match s {
        "true" | "TRUE" | "True" => Value::Bool(true),
        "false" | "FALSE" | "False" => Value::Bool(false),
        "null" | "NULL" | "~" => Value::Null,
        _ => {
            if let Ok(n) = s.parse::<i64>() {
                Value::Integer(n)
            } else if let Ok(f) = s.parse::<f64>() {
                if f.is_finite() { Value::Float(f) } else { Value::String(s.to_string()) }
            } else {
                Value::String(s.to_string())
            }
        }
    }
}

/// Build a nested Value from dot-separated key parts.
/// `["a", "b", "c"]` with leaf `"val"` -> `{"a": {"b": {"c": "val"}}}`.
/// Returns the value rooted at parts[1] (caller handles parts[0]).
pub fn build_nested_value(parts: &[&str], leaf: Value) -> Value {
    let mut val = leaf;
    for &part in parts[1..].iter().rev() {
        let mut t = Table::new();
        t.insert(part.to_string(), val);
        val = Value::Table(t);
    }
    val
}

/// Merge a nested Value into the extra map at the given top-level key.
/// If the key already exists and both values are tables, merge recursively.
pub fn merge_value(
    extra: &mut HashMap<String, Value>,
    key: &str,
    new_val: Value,
) {
    match extra.get_mut(key) {
        Some(Value::Table(existing)) => {
            if let Value::Table(new_table) = new_val {
                merge_tables(existing, new_table);
            } else {
                extra.insert(key.to_string(), new_val);
            }
        }
        _ => {
            extra.insert(key.to_string(), new_val);
        }
    }
}

/// Recursively merge two tables. Values in `source` override `target`.
fn merge_tables(target: &mut Table, source: Table) {
    for (k, v) in source {
        if let Some(existing) = target.get_mut(&k) {
            match (existing, &v) {
                (Value::Table(t), Value::Table(s)) => {
                    merge_tables(t, s.clone());
                }
                (existing, _) => {
                    *existing = v;
                }
            }
        } else {
            target.insert(k, v);
        }
    }
}

// ---------------------------------------------------------------------------
// Minimal YAML parser
// ---------------------------------------------------------------------------

/// Parse minimal YAML: flat `key: value` scalars, simple lists (`- item`),
/// and one level of nesting (for `format:`, `calepin:`, `author:`).
/// This is NOT a full YAML parser -- use TOML for complex front matter.
pub fn parse_minimal_yaml(text: &str) -> Table {
    let mut result = Table::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Skip blank lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            i += 1;
            continue;
        }

        // Must be a top-level key: value or key:
        if let Some((key, rest)) = trimmed.split_once(':') {
            let key = key.trim().to_string();
            let rest = rest.trim();

            if rest.is_empty() {
                // Could be a list, a nested mapping, or a block scalar
                i += 1;
                let indent = next_indent(&lines, i);
                if indent == 0 {
                    // Empty value
                    result.insert(key, Value::Null);
                    continue;
                }

                // Check if first indented line starts with "- "
                if i < lines.len() && lines[i].trim().starts_with("- ") {
                    let list = parse_yaml_list(&lines, &mut i, indent);
                    result.insert(key, list);
                } else if i < lines.len() && lines[i].trim().starts_with('|') {
                    // Block scalar
                    i += 1; // skip the | line... wait, | should be on the key line
                    // Actually in YAML: `abstract: |` then indented lines
                    // But we already consumed the line. Let's collect indented lines.
                    let block = collect_indented_block(&lines, &mut i, indent);
                    result.insert(key, Value::String(block));
                } else {
                    // Nested mapping
                    let sub = parse_yaml_submapping(&lines, &mut i, indent);
                    result.insert(key, Value::Table(sub));
                }
            } else if rest == "|" || rest == "|-" || rest == "|+" || rest == ">" || rest == ">-" {
                // Block scalar
                i += 1;
                let indent = next_indent(&lines, i);
                let block = collect_indented_block(&lines, &mut i, indent);
                let block = if rest.starts_with('>') {
                    // Folded: join lines with spaces
                    block.lines().collect::<Vec<_>>().join(" ")
                } else {
                    block
                };
                result.insert(key, Value::String(block));
            } else {
                // Inline value
                result.insert(key, parse_yaml_scalar(rest));
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    result
}

/// Parse a YAML list (lines starting with `- `) at the given indent level.
fn parse_yaml_list(lines: &[&str], i: &mut usize, min_indent: usize) -> Value {
    let mut items = Vec::new();

    while *i < lines.len() {
        let line = lines[*i];
        if line.trim().is_empty() {
            *i += 1;
            continue;
        }

        let cur_indent = line.len() - line.trim_start().len();
        if cur_indent < min_indent {
            break;
        }

        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("- ") {
            let rest = rest.trim();
            // Check if this is a mapping item (has key: value)
            if rest.contains(": ") || rest.ends_with(':') {
                // Mapping item in list
                let mut sub = Table::new();
                // Parse first key: value on same line as -
                if let Some((k, v)) = rest.split_once(':') {
                    let k = k.trim().to_string();
                    let v = v.trim();
                    if v.is_empty() {
                        *i += 1;
                        let sub_indent = next_indent(lines, *i);
                        if sub_indent > cur_indent {
                            if lines.get(*i).map_or(false, |l| l.trim().starts_with("- ")) {
                                let list = parse_yaml_list(lines, i, sub_indent);
                                sub.insert(k, list);
                            } else {
                                let submap = parse_yaml_submapping(lines, i, sub_indent);
                                sub.insert(k, Value::Table(submap));
                            }
                        } else {
                            sub.insert(k, Value::Null);
                        }
                    } else {
                        sub.insert(k, parse_yaml_scalar(v));
                        *i += 1;
                    }
                }
                // Continue collecting key: value pairs at deeper indent
                while *i < lines.len() {
                    let next_line = lines[*i];
                    if next_line.trim().is_empty() {
                        *i += 1;
                        continue;
                    }
                    let line_indent = next_line.len() - next_line.trim_start().len();
                    // Must be more indented than the "- " and not start with "-"
                    if line_indent <= cur_indent || next_line.trim().starts_with("- ") {
                        break;
                    }
                    let nt = next_line.trim();
                    if let Some((k, v)) = nt.split_once(':') {
                        let k = k.trim().to_string();
                        let v = v.trim();
                        if v.is_empty() {
                            *i += 1;
                            let sub_indent = next_indent(lines, *i);
                            if sub_indent > line_indent {
                                if lines.get(*i).map_or(false, |l| l.trim().starts_with("- ")) {
                                    let list = parse_yaml_list(lines, i, sub_indent);
                                    sub.insert(k, list);
                                } else {
                                    let submap = parse_yaml_submapping(lines, i, sub_indent);
                                    sub.insert(k, Value::Table(submap));
                                }
                            } else {
                                sub.insert(k, Value::Null);
                            }
                        } else {
                            sub.insert(k, parse_yaml_scalar(v));
                            *i += 1;
                        }
                    } else {
                        *i += 1;
                    }
                }
                items.push(Value::Table(sub));
            } else {
                // Simple string item
                items.push(parse_yaml_scalar(rest));
                *i += 1;
            }
        } else {
            break;
        }
    }

    Value::Array(items)
}

/// Parse a nested submapping at the given indent level.
fn parse_yaml_submapping(lines: &[&str], i: &mut usize, min_indent: usize) -> Table {
    let mut result = Table::new();

    while *i < lines.len() {
        let line = lines[*i];
        if line.trim().is_empty() {
            *i += 1;
            continue;
        }

        let cur_indent = line.len() - line.trim_start().len();
        if cur_indent < min_indent {
            break;
        }

        let trimmed = line.trim();
        if let Some((key, rest)) = trimmed.split_once(':') {
            let key = key.trim().to_string();
            let rest = rest.trim();

            if rest.is_empty() {
                *i += 1;
                let sub_indent = next_indent(lines, *i);
                if sub_indent > cur_indent {
                    if lines.get(*i).map_or(false, |l| l.trim().starts_with("- ")) {
                        let list = parse_yaml_list(lines, i, sub_indent);
                        result.insert(key, list);
                    } else {
                        let sub = parse_yaml_submapping(lines, i, sub_indent);
                        result.insert(key, Value::Table(sub));
                    }
                } else {
                    result.insert(key, Value::Null);
                }
            } else if rest == "|" || rest == "|-" || rest == "|+" || rest == ">" || rest == ">-" {
                *i += 1;
                let block_indent = next_indent(lines, *i);
                let block = collect_indented_block(lines, i, block_indent);
                let block = if rest.starts_with('>') {
                    block.lines().collect::<Vec<_>>().join(" ")
                } else {
                    block
                };
                result.insert(key, Value::String(block));
            } else {
                result.insert(key, parse_yaml_scalar(rest));
                *i += 1;
            }
        } else {
            *i += 1;
        }
    }

    result
}

/// Get the indent level of the next non-blank line.
fn next_indent(lines: &[&str], from: usize) -> usize {
    for line in &lines[from..] {
        if !line.trim().is_empty() {
            return line.len() - line.trim_start().len();
        }
    }
    0
}

/// Collect indented lines into a block string.
fn collect_indented_block(lines: &[&str], i: &mut usize, min_indent: usize) -> String {
    let mut block_lines = Vec::new();
    while *i < lines.len() {
        let line = lines[*i];
        if line.trim().is_empty() {
            block_lines.push("");
            *i += 1;
            continue;
        }
        let cur_indent = line.len() - line.trim_start().len();
        if cur_indent < min_indent {
            break;
        }
        // Strip the minimum indent
        if line.len() >= min_indent {
            block_lines.push(&line[min_indent..]);
        } else {
            block_lines.push(line.trim());
        }
        *i += 1;
    }
    // Trim trailing empty lines
    while block_lines.last() == Some(&"") {
        block_lines.pop();
    }
    block_lines.join("\n")
}

/// Parse a scalar YAML value (inline, after the `: `).
fn parse_yaml_scalar(s: &str) -> Value {
    let s = s.trim();

    // Quoted strings
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        return Value::String(s[1..s.len()-1].to_string());
    }

    // Flow sequence: [a, b, c]
    if s.starts_with('[') && s.ends_with(']') {
        let inner = &s[1..s.len()-1];
        let items: Vec<Value> = inner.split(',')
            .map(|item| parse_yaml_scalar(item.trim()))
            .collect();
        return Value::Array(items);
    }

    // Flow mapping: {key: value, ...}
    if s.starts_with('{') && s.ends_with('}') {
        let inner = &s[1..s.len()-1];
        let mut table = Table::new();
        for pair in inner.split(',') {
            if let Some((k, v)) = pair.split_once(':') {
                table.insert(k.trim().to_string(), parse_yaml_scalar(v.trim()));
            }
        }
        return Value::Table(table);
    }

    // Booleans
    match s {
        "true" | "True" | "TRUE" | "yes" | "Yes" | "YES" => return Value::Bool(true),
        "false" | "False" | "FALSE" | "no" | "No" | "NO" => return Value::Bool(false),
        "null" | "Null" | "NULL" | "~" => return Value::Null,
        _ => {}
    }

    // Numbers
    if let Ok(n) = s.parse::<i64>() {
        return Value::Integer(n);
    }
    if let Ok(f) = s.parse::<f64>() {
        if f.is_finite() {
            return Value::Float(f);
        }
    }

    Value::String(s.to_string())
}

// ---------------------------------------------------------------------------
// Front matter format detection
// ---------------------------------------------------------------------------

/// Detect whether front matter text is TOML or minimal YAML.
pub fn detect_frontmatter_format(text: &str) -> FrontMatterFormat {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // TOML indicators: `key = value`, `[section]`, `[[array]]`
        if trimmed.starts_with('[') {
            return FrontMatterFormat::Toml;
        }
        if trimmed.contains(" = ") || trimmed.contains("= ") {
            // But not inside a YAML flow mapping like `{key: val}`
            if !trimmed.starts_with('{') {
                return FrontMatterFormat::Toml;
            }
        }
        // If we see `key: value` first, it's YAML
        if trimmed.contains(": ") || trimmed.ends_with(':') {
            return FrontMatterFormat::Yaml;
        }
    }
    // Default to YAML for empty/ambiguous content
    FrontMatterFormat::Yaml
}

#[derive(Debug, PartialEq)]
pub enum FrontMatterFormat {
    Toml,
    Yaml,
}

/// Parse front matter text (between `---` delimiters) into a Table.
/// Auto-detects TOML vs minimal YAML.
pub fn parse_frontmatter(text: &str) -> anyhow::Result<Table> {
    match detect_frontmatter_format(text) {
        FrontMatterFormat::Toml => {
            let tv: toml::Value = toml::from_str(text)
                .map_err(|e| anyhow::anyhow!("TOML parse error: {}", e))?;
            match tv {
                toml::Value::Table(map) => Ok(table_from_toml(map)),
                _ => Ok(Table::new()),
            }
        }
        FrontMatterFormat::Yaml => {
            Ok(parse_minimal_yaml(text))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_toml() {
        assert_eq!(detect_frontmatter_format("title = \"Hello\""), FrontMatterFormat::Toml);
        assert_eq!(detect_frontmatter_format("[section]\nkey = 1"), FrontMatterFormat::Toml);
    }

    #[test]
    fn test_detect_yaml() {
        assert_eq!(detect_frontmatter_format("title: Hello"), FrontMatterFormat::Yaml);
        assert_eq!(detect_frontmatter_format("format: html"), FrontMatterFormat::Yaml);
    }

    #[test]
    fn test_minimal_yaml_scalars() {
        let table = parse_minimal_yaml("title: Hello\nauthor: World\nformat: html");
        assert_eq!(table_str(&table, "title").as_deref(), Some("Hello"));
        assert_eq!(table_str(&table, "author").as_deref(), Some("World"));
        assert_eq!(table_str(&table, "format").as_deref(), Some("html"));
    }

    #[test]
    fn test_minimal_yaml_list() {
        let table = parse_minimal_yaml("bibliography:\n  - refs.bib\n  - extra.bib");
        let bib = table_get(&table, "bibliography").unwrap();
        let arr = bib.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].as_str(), Some("refs.bib"));
        assert_eq!(arr[1].as_str(), Some("extra.bib"));
    }

    #[test]
    fn test_minimal_yaml_nested() {
        let table = parse_minimal_yaml("calepin:\n  plugins:\n    - txtfmt\n  files-dir: custom");
        let cal = table_get(&table, "calepin").unwrap().as_table().unwrap();
        let plugins = table_get(cal, "plugins").unwrap().as_array().unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].as_str(), Some("txtfmt"));
        assert_eq!(table_str(cal, "files-dir").as_deref(), Some("custom"));
    }

    #[test]
    fn test_minimal_yaml_booleans() {
        let table = parse_minimal_yaml("number-sections: true\ntoc: false");
        assert_eq!(table_get(&table, "number-sections").unwrap().as_bool(), Some(true));
        assert_eq!(table_get(&table, "toc").unwrap().as_bool(), Some(false));
    }

    #[test]
    fn test_minimal_yaml_quoted_strings() {
        let table = parse_minimal_yaml("title: \"Hello World\"\nauthor: 'Jane Doe'");
        assert_eq!(table_str(&table, "title").as_deref(), Some("Hello World"));
        assert_eq!(table_str(&table, "author").as_deref(), Some("Jane Doe"));
    }

    #[test]
    fn test_toml_frontmatter() {
        let table = parse_frontmatter("title = \"Hello\"\nauthor = \"World\"").unwrap();
        assert_eq!(table_str(&table, "title").as_deref(), Some("Hello"));
        assert_eq!(table_str(&table, "author").as_deref(), Some("World"));
    }

    #[test]
    fn test_toml_nested() {
        let table = parse_frontmatter("[calepin]\nplugins = [\"txtfmt\"]").unwrap();
        let cal = table_get(&table, "calepin").unwrap().as_table().unwrap();
        let plugins = table_get(cal, "plugins").unwrap().as_array().unwrap();
        assert_eq!(plugins[0].as_str(), Some("txtfmt"));
    }

    #[test]
    fn test_yaml_format_mapping() {
        let table = parse_minimal_yaml("format:\n  html: default");
        let fmt = table_get(&table, "format").unwrap();
        let fmt_table = fmt.as_table().unwrap();
        assert_eq!(fmt_table.keys().next().unwrap(), "html");
    }

    #[test]
    fn test_yaml_block_scalar() {
        let table = parse_minimal_yaml("abstract: |\n  some content\n  more content");
        let abs = table_str(&table, "abstract").unwrap();
        assert!(abs.contains("some content"));
        assert!(abs.contains("more content"));
    }

    #[test]
    fn test_yaml_flow_sequence() {
        let table = parse_minimal_yaml("keywords: [one, two, three]");
        let kw = table_get(&table, "keywords").unwrap().as_array().unwrap();
        assert_eq!(kw.len(), 3);
        assert_eq!(kw[0].as_str(), Some("one"));
    }

    #[test]
    fn test_coerce_value() {
        assert!(matches!(coerce_value("true"), Value::Bool(true)));
        assert!(matches!(coerce_value("false"), Value::Bool(false)));
        assert!(matches!(coerce_value("42"), Value::Integer(42)));
        assert!(matches!(coerce_value("3.14"), Value::Float(_)));
        assert!(matches!(coerce_value("hello"), Value::String(_)));
    }

    #[test]
    fn test_yaml_author_list_of_mappings() {
        let yaml = "author:\n  - name: Alice\n    email: alice@example.com\n  - name: Bob\n    email: bob@example.com";
        let table = parse_minimal_yaml(yaml);
        let authors = table_get(&table, "author").unwrap().as_array().unwrap();
        assert_eq!(authors.len(), 2);
        let a0 = authors[0].as_table().unwrap();
        assert_eq!(table_str(a0, "name").as_deref(), Some("Alice"));
        assert_eq!(table_str(a0, "email").as_deref(), Some("alice@example.com"));
    }
}
