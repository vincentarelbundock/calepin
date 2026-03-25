//! Calepin's internal value type.
//!
//! Common representation for front matter, plugin manifests, and
//! configuration values. Both TOML and YAML parse into this type
//! via `from_toml` / `from_yaml` converters.

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
// Conversion from serde_yaml::Value
// ---------------------------------------------------------------------------

/// Convert a `serde_yaml::Value` into our `Value`.
pub fn from_yaml(yv: serde_yaml::Value) -> Value {
    match yv {
        serde_yaml::Value::Null => Value::Null,
        serde_yaml::Value::Bool(b) => Value::Bool(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::String(n.to_string())
            }
        }
        serde_yaml::Value::String(s) => Value::String(s),
        serde_yaml::Value::Sequence(seq) => {
            Value::Array(seq.into_iter().map(from_yaml).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let table: Table = map.into_iter()
                .filter_map(|(k, v)| {
                    let key = match k {
                        serde_yaml::Value::String(s) => s,
                        other => other.as_str().map(|s| s.to_string())
                            .unwrap_or_else(|| format!("{:?}", other)),
                    };
                    Some((key, from_yaml(v)))
                })
                .collect();
            Value::Table(table)
        }
        serde_yaml::Value::Tagged(tagged) => from_yaml(tagged.value),
    }
}

/// Convert a `serde_yaml::Mapping` into our `Table`.
pub fn table_from_yaml(map: serde_yaml::Mapping) -> Table {
    map.into_iter()
        .filter_map(|(k, v)| {
            let key = match k {
                serde_yaml::Value::String(s) => s,
                other => other.as_str().map(|s| s.to_string())?,
            };
            Some((key, from_yaml(v)))
        })
        .collect()
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
            let yv: serde_yaml::Value = serde_yaml::from_str(text)
                .map_err(|e| anyhow::anyhow!("YAML parse error: {}", e))?;
            match yv {
                serde_yaml::Value::Mapping(map) => Ok(table_from_yaml(map)),
                _ => Ok(Table::new()),
            }
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
    fn test_yaml_scalars() {
        let table = parse_frontmatter("title: Hello\nauthor: World\nformat: html").unwrap();
        assert_eq!(table_str(&table, "title").as_deref(), Some("Hello"));
        assert_eq!(table_str(&table, "author").as_deref(), Some("World"));
        assert_eq!(table_str(&table, "format").as_deref(), Some("html"));
    }

    #[test]
    fn test_yaml_list() {
        let table = parse_frontmatter("bibliography:\n  - refs.bib\n  - extra.bib").unwrap();
        let bib = table_get(&table, "bibliography").unwrap();
        let arr = bib.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].as_str(), Some("refs.bib"));
        assert_eq!(arr[1].as_str(), Some("extra.bib"));
    }

    #[test]
    fn test_yaml_nested() {
        let table = parse_frontmatter("calepin:\n  plugins:\n    - txtfmt\n  files-dir: custom").unwrap();
        let cal = table_get(&table, "calepin").unwrap().as_table().unwrap();
        let plugins = table_get(cal, "plugins").unwrap().as_array().unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].as_str(), Some("txtfmt"));
        assert_eq!(table_str(cal, "files-dir").as_deref(), Some("custom"));
    }

    #[test]
    fn test_yaml_booleans() {
        let table = parse_frontmatter("number-sections: true\ntoc: false").unwrap();
        assert_eq!(table_get(&table, "number-sections").unwrap().as_bool(), Some(true));
        assert_eq!(table_get(&table, "toc").unwrap().as_bool(), Some(false));
    }

    #[test]
    fn test_yaml_quoted_strings() {
        let table = parse_frontmatter("title: \"Hello World\"\nauthor: 'Jane Doe'").unwrap();
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
        let table = parse_frontmatter("format:\n  html: default").unwrap();
        let fmt = table_get(&table, "format").unwrap();
        let fmt_table = fmt.as_table().unwrap();
        assert_eq!(fmt_table.keys().next().unwrap(), "html");
    }

    #[test]
    fn test_yaml_block_scalar() {
        let table = parse_frontmatter("abstract: |\n  some content\n  more content").unwrap();
        let abs = table_str(&table, "abstract").unwrap();
        assert!(abs.contains("some content"));
        assert!(abs.contains("more content"));
    }

    #[test]
    fn test_yaml_flow_sequence() {
        let table = parse_frontmatter("keywords: [one, two, three]").unwrap();
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
        let table = parse_frontmatter(yaml).unwrap();
        let authors = table_get(&table, "author").unwrap().as_array().unwrap();
        assert_eq!(authors.len(), 2);
        let a0 = authors[0].as_table().unwrap();
        assert_eq!(table_str(a0, "name").as_deref(), Some("Alice"));
        assert_eq!(table_str(a0, "email").as_deref(), Some("alice@example.com"));
    }
}
