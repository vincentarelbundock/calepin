//! Quarto YAML compatibility layer.
//!
//! Parses YAML front matter and produces a calepin `Table` that looks like
//! what TOML front matter would have produced. All Quarto-specific logic is
//! isolated in this module.

mod convert;
mod remap;

use anyhow::Result;
use crate::value::Table;

/// Parse a YAML front matter string into a calepin `Table`.
///
/// The returned table has been remapped from Quarto conventions to calepin
/// conventions (e.g. `format:` -> `target`, `toc: true` -> `toc.enabled`).
/// It can be passed directly to `parse_metadata()`.
pub fn parse_yaml(raw: &str) -> Result<Table> {
    let yaml_value: serde_yaml::Value = serde_yaml::from_str(raw)
        .map_err(|e| anyhow::anyhow!("YAML parse error: {}", e))?;

    let value = convert::from_yaml(yaml_value);
    let mut table = match value {
        crate::value::Value::Table(t) => t,
        _ => return Ok(Table::new()),
    };

    remap::remap_quarto_keys(&mut table);
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{table_str, table_get};

    #[test]
    fn test_basic_yaml() {
        let yaml = "title: My Document\nauthor: Jane Doe\ndate: 2024-01-01";
        let table = parse_yaml(yaml).unwrap();
        assert_eq!(table_str(&table, "title").as_deref(), Some("My Document"));
        assert_eq!(table_str(&table, "author").as_deref(), Some("Jane Doe"));
        assert_eq!(table_str(&table, "date").as_deref(), Some("2024-01-01"));
    }

    #[test]
    fn test_nested_execute() {
        let yaml = "execute:\n  echo: false\n  eval: true";
        let table = parse_yaml(yaml).unwrap();
        let exec = table_get(&table, "execute").unwrap().as_table().unwrap();
        assert_eq!(exec.get("echo").unwrap().as_bool(), Some(false));
        assert_eq!(exec.get("eval").unwrap().as_bool(), Some(true));
    }

    #[test]
    fn test_bibliography_list() {
        let yaml = "bibliography:\n  - refs.bib\n  - extra.bib";
        let table = parse_yaml(yaml).unwrap();
        let bib = table_get(&table, "bibliography").unwrap().as_array().unwrap();
        assert_eq!(bib.len(), 2);
        assert_eq!(bib[0].as_str(), Some("refs.bib"));
    }

    #[test]
    fn test_format_to_target() {
        let yaml = "format: html";
        let table = parse_yaml(yaml).unwrap();
        assert_eq!(table_str(&table, "target").as_deref(), Some("html"));
        assert!(table.get("format").is_none());
    }

    #[test]
    fn test_author_list() {
        let yaml = "author:\n  - name: Jane Doe\n    email: jane@example.com\n  - name: John Smith";
        let table = parse_yaml(yaml).unwrap();
        let authors = table_get(&table, "author").unwrap().as_array().unwrap();
        assert_eq!(authors.len(), 2);
    }

    #[test]
    fn test_invalid_yaml() {
        let result = parse_yaml("{{invalid yaml");
        assert!(result.is_err());
    }

    #[test]
    fn test_non_mapping_yaml_returns_empty() {
        // A bare scalar is valid YAML but not a mapping
        let table = parse_yaml("just a string").unwrap();
        assert!(table.is_empty());
    }

    #[test]
    fn test_empty_yaml() {
        let table = parse_yaml("").unwrap();
        assert!(table.is_empty());
    }

    #[test]
    fn test_yaml_12_booleans() {
        // YAML 1.2 (serde_yaml): only true/false are booleans; yes/no are strings
        let yaml = "echo: false\neval: true";
        let table = parse_yaml(yaml).unwrap();
        assert_eq!(table.get("echo").unwrap().as_bool(), Some(false));
        assert_eq!(table.get("eval").unwrap().as_bool(), Some(true));
    }

    #[test]
    fn test_full_quarto_frontmatter() {
        let yaml = r#"
title: My Paper
subtitle: A study
author:
  - name: Jane Doe
    email: jane@example.com
date: 2024-06-15
format: html
bibliography:
  - refs.bib
  - extra.bib
toc: true
number-sections: true
execute:
  echo: false
  cache: true
lang: en
csl: apa.csl
keywords:
  - statistics
  - modeling
"#;
        let table = parse_yaml(yaml).unwrap();
        assert_eq!(table_str(&table, "title").as_deref(), Some("My Paper"));
        assert_eq!(table_str(&table, "subtitle").as_deref(), Some("A study"));
        assert_eq!(table_str(&table, "target").as_deref(), Some("html"));
        assert_eq!(table_str(&table, "lang").as_deref(), Some("en"));
        assert_eq!(table_str(&table, "csl").as_deref(), Some("apa.csl"));
        assert!(table.get("format").is_none());

        let bib = table_get(&table, "bibliography").unwrap().as_array().unwrap();
        assert_eq!(bib.len(), 2);

        let toc = table_get(&table, "toc").unwrap().as_table().unwrap();
        assert_eq!(toc.get("enabled").unwrap().as_bool(), Some(true));

        let exec = table_get(&table, "execute").unwrap().as_table().unwrap();
        assert_eq!(exec.get("echo").unwrap().as_bool(), Some(false));
        assert_eq!(exec.get("cache").unwrap().as_bool(), Some(true));

        let kw = table_get(&table, "keywords").unwrap().as_array().unwrap();
        assert_eq!(kw.len(), 2);
    }
}
