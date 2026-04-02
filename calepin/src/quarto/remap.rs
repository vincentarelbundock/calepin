//! Remap Quarto YAML keys to calepin-compatible Table keys.
//!
//! Operates on a calepin `Table` (already converted from YAML) and rewrites
//! keys/values so the result looks like what TOML front matter would produce.

use crate::value::{Table, Value};

/// Known Quarto keys that have no calepin equivalent. Warned and dropped.
const UNSUPPORTED: &[&str] = &[
    "freeze",
    "code_fold",
    "code_summary",
    "code_tools",
    "code_overflow",
    "code_line_numbers",
    "code_annotations",
    "embed_resources",
    "self_contained",
    "theme",
    "mainfont",
    "monofont",
    "fontsize",
    "include_in_header",
    "include_before_body",
    "include_after_body",
    "filters",
    "metadata_files",
    "params",
    "lightbox",
];

/// Remap a Table of Quarto-style keys into calepin-compatible keys.
/// Modifies the table in place: renames keys, restructures values, warns on
/// unsupported keys, and normalizes all key names (dashes to underscores).
pub fn remap_quarto_keys(table: &mut Table) {
    // First pass: normalize all keys (dashes/dots to underscores)
    let entries: Vec<(String, Value)> = table
        .drain(..)
        .map(|(k, v)| (crate::util::normalize_key(&k), v))
        .collect();
    for (k, v) in entries {
        table.insert(k, v);
    }

    // format -> target
    if let Some(format_val) = table.swap_remove("format") {
        if table.get("target").is_none() {
            match &format_val {
                Value::String(s) => {
                    table.insert("target".to_string(), Value::String(s.clone()));
                }
                Value::Table(fmt_table) => {
                    // Multi-format: `format: { html: {...}, pdf: {...} }`
                    // Use the first key as target, warn about the rest.
                    if let Some((first_key, _)) = fmt_table.iter().next() {
                        table.insert("target".to_string(), Value::String(first_key.clone()));
                    }
                    if fmt_table.len() > 1 {
                        cwarn!("multiple output formats in `format:` -- using first, ignoring rest");
                    }
                }
                _ => {}
            }
        }
    }

    // toc: true -> toc: { enabled: true }
    if let Some(toc_val) = table.get("toc") {
        if let Some(b) = toc_val.as_bool() {
            let mut toc_table = Table::new();
            toc_table.insert("enabled".to_string(), Value::Bool(b));
            table.insert("toc".to_string(), Value::Table(toc_table));
        }
    }

    // institutes -> affiliations (Quarto/Pandoc alias)
    if let Some(inst_val) = table.swap_remove("institutes") {
        if table.get("affiliations").is_none() {
            table.insert("affiliations".to_string(), inst_val);
        }
    }

    // affiliation (singular) -> affiliations inside author entries
    remap_author_affiliation_singular(table);

    // Warn on unsupported keys and remove them
    let to_remove: Vec<String> = table
        .keys()
        .filter(|k| UNSUPPORTED.contains(&k.as_str()))
        .cloned()
        .collect();
    for key in to_remove {
        cwarn!("ignoring unsupported Quarto field: {}", key.replace('_', "-"));
        table.swap_remove(&key);
    }
}

/// Rewrite `affiliation` (singular) to `affiliations` inside author entries.
/// Quarto accepts both; calepin only checks `affiliations`.
fn remap_author_affiliation_singular(table: &mut Table) {
    let key = if table.get("author").is_some() {
        "author"
    } else if table.get("authors").is_some() {
        "authors"
    } else {
        return;
    };

    let Some(author_val) = table.get_mut(key) else { return };

    match author_val {
        Value::Array(authors) => {
            for entry in authors.iter_mut() {
                if let Value::Table(m) = entry {
                    if m.get("affiliations").is_none() {
                        if let Some(aff) = m.swap_remove("affiliation") {
                            m.insert("affiliations".to_string(), aff);
                        }
                    }
                }
            }
        }
        Value::Table(m) => {
            // Single author as mapping
            if m.get("affiliations").is_none() {
                if let Some(aff) = m.swap_remove("affiliation") {
                    m.insert("affiliations".to_string(), aff);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_table(yaml: &str) -> Table {
        let yv: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        match crate::quarto::convert::from_yaml(yv) {
            Value::Table(t) => t,
            _ => panic!("expected table"),
        }
    }

    #[test]
    fn test_format_string_to_target() {
        let mut t = make_table("format: html");
        remap_quarto_keys(&mut t);
        assert_eq!(t.get("target").unwrap().as_str(), Some("html"));
        assert!(t.get("format").is_none());
    }

    #[test]
    fn test_format_mapping_to_target() {
        let mut t = make_table("format:\n  html:\n    toc: true");
        remap_quarto_keys(&mut t);
        assert_eq!(t.get("target").unwrap().as_str(), Some("html"));
    }

    #[test]
    fn test_toc_bool_to_table() {
        let mut t = make_table("toc: true");
        remap_quarto_keys(&mut t);
        let toc = t.get("toc").unwrap().as_table().unwrap();
        assert_eq!(toc.get("enabled").unwrap().as_bool(), Some(true));
    }

    #[test]
    fn test_toc_mapping_preserved() {
        let mut t = make_table("toc: true\ntoc-depth: 4");
        remap_quarto_keys(&mut t);
        // toc should be a table with enabled=true
        let toc = t.get("toc").unwrap().as_table().unwrap();
        assert_eq!(toc.get("enabled").unwrap().as_bool(), Some(true));
        // toc_depth should survive as a separate key (handled by parse_metadata)
        assert!(t.get("toc_depth").is_some());
    }

    #[test]
    fn test_dash_normalization() {
        let mut t = make_table("number-sections: true\ndate-format: \"%Y\"");
        remap_quarto_keys(&mut t);
        assert!(t.get("number_sections").is_some());
        assert!(t.get("date_format").is_some());
    }

    #[test]
    fn test_unsupported_keys_removed() {
        let mut t = make_table("title: Hello\nfreeze: true\ncode-fold: true");
        remap_quarto_keys(&mut t);
        assert!(t.get("title").is_some());
        assert!(t.get("freeze").is_none());
        assert!(t.get("code_fold").is_none());
    }

    #[test]
    fn test_target_takes_precedence_over_format() {
        let mut t = make_table("target: latex\nformat: html");
        remap_quarto_keys(&mut t);
        assert_eq!(t.get("target").unwrap().as_str(), Some("latex"));
    }

    #[test]
    fn test_format_multi_key_uses_first() {
        let mut t = make_table("format:\n  html:\n    toc: true\n  pdf:\n    toc: false");
        remap_quarto_keys(&mut t);
        assert_eq!(t.get("target").unwrap().as_str(), Some("html"));
    }

    #[test]
    fn test_toc_false_to_table() {
        let mut t = make_table("toc: false");
        remap_quarto_keys(&mut t);
        let toc = t.get("toc").unwrap().as_table().unwrap();
        assert_eq!(toc.get("enabled").unwrap().as_bool(), Some(false));
    }

    #[test]
    fn test_toc_mapping_not_rewritten() {
        let mut t = make_table("toc:\n  depth: 4\n  title: TOC");
        remap_quarto_keys(&mut t);
        // toc is already a mapping, should not be touched by the bool rewrite
        let toc = t.get("toc").unwrap().as_table().unwrap();
        assert!(toc.get("depth").is_some());
        assert!(toc.get("title").is_some());
        assert!(toc.get("enabled").is_none());
    }

    #[test]
    fn test_unknown_keys_survive() {
        let mut t = make_table("title: Hi\ncustom-thing: value");
        remap_quarto_keys(&mut t);
        assert!(t.get("custom_thing").is_some());
        assert_eq!(t.get("custom_thing").unwrap().as_str(), Some("value"));
    }

    #[test]
    fn test_dot_normalization() {
        let mut t = make_table("\"some.dotted.key\": value");
        remap_quarto_keys(&mut t);
        assert!(t.get("some_dotted_key").is_some());
    }

    #[test]
    fn test_all_unsupported_keys_listed() {
        // Every key in UNSUPPORTED should be removed
        let yaml_keys: Vec<String> = UNSUPPORTED.iter()
            .map(|k| format!("{}: true", k.replace('_', "-")))
            .collect();
        let yaml = yaml_keys.join("\n");
        let mut t = make_table(&yaml);
        remap_quarto_keys(&mut t);
        assert!(t.is_empty(), "all unsupported keys should be removed, remaining: {:?}", t.keys().collect::<Vec<_>>());
    }

    #[test]
    fn test_format_non_string_non_table_ignored() {
        let mut t = make_table("format: 42");
        remap_quarto_keys(&mut t);
        // format was an integer, not string or table; target should not be set
        assert!(t.get("target").is_none());
    }

    #[test]
    fn test_empty_table() {
        let mut t = Table::new();
        remap_quarto_keys(&mut t);
        assert!(t.is_empty());
    }

    #[test]
    fn test_institutes_to_affiliations() {
        let mut t = make_table("institutes:\n  - id: mit\n    name: MIT");
        remap_quarto_keys(&mut t);
        assert!(t.get("institutes").is_none());
        let affs = t.get("affiliations").unwrap().as_array().unwrap();
        assert_eq!(affs.len(), 1);
    }

    #[test]
    fn test_institutes_does_not_overwrite_affiliations() {
        let mut t = make_table("affiliations:\n  - name: Stanford\ninstitutes:\n  - name: MIT");
        remap_quarto_keys(&mut t);
        let affs = t.get("affiliations").unwrap().as_array().unwrap();
        // affiliations should win; institutes should be dropped
        assert_eq!(affs.len(), 1);
        let name = affs[0].as_table().unwrap().get("name").unwrap().as_str().unwrap();
        assert_eq!(name, "Stanford");
    }

    #[test]
    fn test_affiliation_singular_to_plural() {
        let mut t = make_table("author:\n  - name: Jane Doe\n    affiliation: MIT");
        remap_quarto_keys(&mut t);
        let authors = t.get("author").unwrap().as_array().unwrap();
        let a = authors[0].as_table().unwrap();
        assert!(a.get("affiliation").is_none());
        assert!(a.get("affiliations").is_some());
        assert_eq!(a.get("affiliations").unwrap().as_str(), Some("MIT"));
    }

    #[test]
    fn test_affiliation_singular_list() {
        let mut t = make_table("author:\n  - name: Jane\n    affiliation:\n      - MIT\n      - Stanford");
        remap_quarto_keys(&mut t);
        let authors = t.get("author").unwrap().as_array().unwrap();
        let a = authors[0].as_table().unwrap();
        let affs = a.get("affiliations").unwrap().as_array().unwrap();
        assert_eq!(affs.len(), 2);
    }

    #[test]
    fn test_affiliation_singular_does_not_overwrite_plural() {
        let mut t = make_table("author:\n  - name: Jane\n    affiliations:\n      - Stanford\n    affiliation: MIT");
        remap_quarto_keys(&mut t);
        let authors = t.get("author").unwrap().as_array().unwrap();
        let a = authors[0].as_table().unwrap();
        // affiliations (plural) should win
        let affs = a.get("affiliations").unwrap().as_array().unwrap();
        assert_eq!(affs[0].as_str(), Some("Stanford"));
    }

    #[test]
    fn test_single_author_mapping_affiliation_singular() {
        let mut t = make_table("author:\n  name: Jane Doe\n  affiliation: MIT");
        remap_quarto_keys(&mut t);
        let a = t.get("author").unwrap().as_table().unwrap();
        assert!(a.get("affiliation").is_none());
        assert_eq!(a.get("affiliations").unwrap().as_str(), Some("MIT"));
    }
}
