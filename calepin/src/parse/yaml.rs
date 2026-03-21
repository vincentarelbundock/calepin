use anyhow::Result;
use std::collections::HashMap;

use saphyr::{LoadableYamlNode, MappingOwned, ScalarOwned, YamlOwned};

use crate::types::{Affiliation, Author, AuthorName, CitationMeta, Copyright, Funding, License, Metadata};

/// Construct a YAML key node from a string.
fn yaml_key(key: &str) -> YamlOwned {
    YamlOwned::Value(ScalarOwned::String(key.to_string()))
}

/// Split YAML front matter from the document body.
/// Returns (metadata, body_text).
pub fn split_yaml(text: &str) -> Result<(Metadata, String)> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() || lines[0].trim() != "---" {
        return Ok((Metadata::default(), text.to_string()));
    }

    // Find closing --- or ... (must start at column 0, not indented)
    let mut end = None;
    for (i, line) in lines.iter().enumerate().skip(1) {
        let trimmed = line.trim_end();
        if trimmed == "---" || trimmed == "..." {
            end = Some(i);
            break;
        }
    }

    let end = match end {
        Some(e) => e,
        None => return Ok((Metadata::default(), text.to_string())),
    };

    let yaml_str: String = lines[1..end].join("\n");
    let body: String = lines[end + 1..].join("\n");

    let metadata = parse_yaml(&yaml_str)?;
    Ok((metadata, body))
}

fn parse_yaml(yaml_str: &str) -> Result<Metadata> {
    let docs = YamlOwned::load_from_str(yaml_str)?;
    let value = match docs.into_iter().next() {
        Some(v) => v,
        None => return Ok(Metadata::default()),
    };
    let map = match value.as_mapping() {
        Some(m) => m,
        None => return Ok(Metadata::default()),
    };

    let mut meta = Metadata::default();
    let mut extra = HashMap::new();

    // First pass: collect top-level affiliations (needed for ref: lookups)
    let top_level_affiliations = map
        .get(&yaml_key("affiliations"))
        .and_then(|v| v.as_sequence())
        .cloned()
        .unwrap_or_default();

    for (k, v) in map {
        let key = match k.as_str() {
            Some(s) => s,
            None => continue,
        };
        match key {
            "title" => meta.title = v.as_str().map(String::from),
            "subtitle" => meta.subtitle = v.as_str().map(String::from),
            "author" | "authors" => {
                parse_authors(v, &mut meta, &top_level_affiliations);
            }
            "affiliations" => {} // handled above
            "date" => meta.date = v.as_str().map(String::from),
            "abstract" => meta.abstract_text = v.as_str().map(String::from),
            "keywords" => {
                meta.keywords = yaml_string_list(v);
            }
            "copyright" => meta.copyright = Some(parse_copyright(v)),
            "license" => meta.license = Some(parse_license(v)),
            "citation" => meta.citation = parse_citation(v),
            "funding" => meta.funding = parse_funding(v),
            "appendix-style" => meta.appendix_style = v.as_str().map(String::from),
            "css" => {
                meta.css = yaml_string_list(v);
            }
            "header-includes" => meta.header_includes = v.as_str().map(String::from),
            "include-before" => meta.include_before = v.as_str().map(String::from),
            "include-after" => meta.include_after = v.as_str().map(String::from),
            "format" => {
                meta.format = v.as_str().map(String::from).or_else(|| {
                    // Support `format: { html: default }` — extract first key
                    v.as_mapping()
                        .and_then(|m| m.keys().next())
                        .and_then(|k| k.as_str())
                        .map(String::from)
                });
            }
            "number-sections" => meta.number_sections = v.as_bool().unwrap_or(false),
            "toc" => meta.toc = Some(v.as_bool().unwrap_or(false)),
            "toc-depth" => meta.toc_depth = v.as_integer().unwrap_or(3) as u8,
            "toc-title" => meta.toc_title = v.as_str().map(String::from),
            "date-format" => meta.date_format = v.as_str().map(String::from),
            "bibliography" => {
                meta.bibliography = yaml_string_list(v);
            }
            "csl" => meta.csl = v.as_str().map(String::from),
            "calepin" => {
                if let Some(cmap) = v.as_mapping() {
                    if let Some(pv) = cmap.get(&yaml_key("plugins")) {
                        meta.plugins = yaml_string_list(pv);
                    }
                }
            }
            _ => {
                extra.insert(key.to_string(), v.clone());
            }
        }
    }
    meta.extra = extra;

    Ok(meta)
}

// ---------------------------------------------------------------------------
// Rich author / affiliation parsing
// ---------------------------------------------------------------------------

/// Parse the `author:` or `authors:` YAML value into rich `Author` structs
/// and a flat, deduplicated affiliation list. Also populates `meta.author`
/// (simple string list) for backward compatibility.
fn parse_authors(
    v: &YamlOwned,
    meta: &mut Metadata,
    top_level_affiliations: &[YamlOwned],
) {
    let entries: Vec<&YamlOwned> = match v {
        YamlOwned::Value(ScalarOwned::String(_)) => vec![v],
        YamlOwned::Mapping(_) => vec![v],
        YamlOwned::Sequence(seq) => seq.iter().collect(),
        _ => return,
    };

    let mut authors: Vec<Author> = Vec::new();
    let mut affiliations: Vec<Affiliation> = Vec::new();
    let mut simple_names: Vec<String> = Vec::new();

    for entry in entries {
        if let Some(s) = entry.as_str() {
            let name = parse_author_name_str(s);
            simple_names.push(name.literal.clone());
            authors.push(Author { name, ..Default::default() });
        } else if let YamlOwned::Mapping(m) = entry {
            let author = parse_author_mapping(m, &mut affiliations, top_level_affiliations);
            simple_names.push(author.name.literal.clone());
            authors.push(author);
        }
    }

    // Number affiliations
    for (i, aff) in affiliations.iter_mut().enumerate() {
        aff.number = i + 1;
    }

    meta.author = if simple_names.is_empty() { None } else { Some(simple_names) };
    meta.authors = authors;
    meta.affiliations = affiliations;
}

/// Parse a single author name string into given/family/literal components.
fn parse_author_name_str(s: &str) -> AuthorName {
    let s = s.trim();
    if s.contains(',') {
        // "Family, Given" (BibTeX convention)
        let mut parts = s.splitn(2, ',');
        let family = parts.next().unwrap_or("").trim().to_string();
        let given = parts.next().unwrap_or("").trim().to_string();
        let literal = if given.is_empty() {
            family.clone()
        } else {
            format!("{} {}", given, family)
        };
        AuthorName {
            given: if given.is_empty() { None } else { Some(given) },
            family: Some(family),
            literal,
        }
    } else {
        // "Given Family" — last word is family
        let words: Vec<&str> = s.split_whitespace().collect();
        if words.len() <= 1 {
            AuthorName { literal: s.to_string(), ..Default::default() }
        } else {
            let family = words.last().unwrap().to_string();
            let given = words[..words.len() - 1].join(" ");
            AuthorName {
                given: Some(given),
                family: Some(family),
                literal: s.to_string(),
            }
        }
    }
}

/// Parse a mapping-form author entry into an `Author`.
fn parse_author_mapping(
    m: &MappingOwned,
    affiliations: &mut Vec<Affiliation>,
    top_level_affiliations: &[YamlOwned],
) -> Author {
    let mut author = Author::default();

    // Name
    if let Some(name_val) = m.get(&yaml_key("name")) {
        if let Some(s) = name_val.as_str() {
            author.name = parse_author_name_str(s);
        } else if let Some(nm) = name_val.as_mapping() {
            let given = yaml_str(nm, "given");
            let family = yaml_str(nm, "family");
            let literal = yaml_str(nm, "literal").unwrap_or_else(|| {
                match (&given, &family) {
                    (Some(g), Some(f)) => format!("{} {}", g, f),
                    (Some(g), None) => g.clone(),
                    (None, Some(f)) => f.clone(),
                    (None, None) => String::new(),
                }
            });
            author.name = AuthorName { given, family, literal };
        }
    }

    // Scalar fields
    author.email = yaml_str(m, "email");
    author.url = yaml_str(m, "url");
    author.orcid = yaml_str(m, "orcid");
    author.note = yaml_str(m, "note");

    // Attributes (can appear at top level or under "attributes")
    author.corresponding = yaml_bool(m, "corresponding");
    author.equal_contributor = yaml_bool(m, "equal-contributor");
    author.deceased = yaml_bool(m, "deceased");
    if let Some(attrs) = m.get(&yaml_key("attributes")) {
        if let Some(am) = attrs.as_mapping() {
            if yaml_bool(am, "corresponding") { author.corresponding = true; }
            if yaml_bool(am, "equal-contributor") { author.equal_contributor = true; }
            if yaml_bool(am, "deceased") { author.deceased = true; }
        }
    }

    // Roles
    let role_key = m.get(&yaml_key("roles"))
        .or_else(|| m.get(&yaml_key("role")));
    if let Some(rv) = role_key {
        if let Some(s) = rv.as_str() {
            author.roles.push(s.to_string());
        } else if let Some(seq) = rv.as_sequence() {
            for item in seq {
                if let Some(s) = item.as_str() {
                    author.roles.push(s.to_string());
                }
            }
        }
    }

    // Affiliations
    let aff_key = m.get(&yaml_key("affiliations"))
        .or_else(|| m.get(&yaml_key("affiliation")));
    if let Some(aff_val) = aff_key {
        let aff_entries: Vec<&YamlOwned> = if aff_val.as_str().is_some() || aff_val.as_mapping().is_some() {
            vec![aff_val]
        } else if let Some(seq) = aff_val.as_sequence() {
            seq.iter().collect()
        } else {
            vec![]
        };
        for entry in aff_entries {
            let idx = resolve_affiliation(entry, affiliations, top_level_affiliations);
            if let Some(i) = idx {
                author.affiliation_ids.push(i);
            }
        }
    }

    author
}

/// Resolve an affiliation entry to an index in the affiliations vec.
/// Handles: plain string, `{ref: id}`, or inline affiliation mapping.
fn resolve_affiliation(
    entry: &YamlOwned,
    affiliations: &mut Vec<Affiliation>,
    top_level: &[YamlOwned],
) -> Option<usize> {
    if let Some(s) = entry.as_str() {
        // Deduplicate by name
        if let Some(idx) = affiliations.iter().position(|a| a.name.as_deref() == Some(s)) {
            return Some(idx);
        }
        let aff = Affiliation { name: Some(s.to_string()), ..Default::default() };
        affiliations.push(aff);
        return Some(affiliations.len() - 1);
    }
    if let Some(m) = entry.as_mapping() {
        // Check for ref:
        if let Some(ref_val) = yaml_str(m, "ref") {
            // Look up in top-level affiliations
            for tl in top_level {
                if let Some(tlm) = tl.as_mapping() {
                    if yaml_str(tlm, "id").as_deref() == Some(ref_val.as_str()) {
                        return resolve_affiliation(tl, affiliations, &[]);
                    }
                }
            }
            // Also check already-parsed affiliations by id
            if let Some(idx) = affiliations.iter().position(|a| a.id.as_deref() == Some(ref_val.as_str())) {
                return Some(idx);
            }
            return None;
        }
        // Inline affiliation — deduplicate by id or name
        let id = yaml_str(m, "id");
        let name = yaml_str(m, "name");
        if let Some(ref id_str) = id {
            if let Some(idx) = affiliations.iter().position(|a| a.id.as_deref() == Some(id_str.as_str())) {
                return Some(idx);
            }
        }
        if id.is_none() {
            if let Some(ref name_str) = name {
                if let Some(idx) = affiliations.iter().position(|a| a.name.as_deref() == Some(name_str.as_str())) {
                    return Some(idx);
                }
            }
        }
        let aff = Affiliation {
            id,
            name,
            department: yaml_str(m, "department"),
            city: yaml_str(m, "city"),
            region: yaml_str(m, "region").or_else(|| yaml_str(m, "state")),
            country: yaml_str(m, "country"),
            postal_code: yaml_str(m, "postal-code"),
            url: yaml_str(m, "url").or_else(|| yaml_str(m, "affiliation-url")),
            ..Default::default()
        };
        affiliations.push(aff);
        return Some(affiliations.len() - 1);
    }
    None
}

/// Helper: get an optional string from a YAML mapping.
fn yaml_str(m: &MappingOwned, key: &str) -> Option<String> {
    m.get(&yaml_key(key))
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Helper: get a bool from a YAML mapping (default false).
fn yaml_bool(m: &MappingOwned, key: &str) -> bool {
    m.get(&yaml_key(key))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Helper: parse a YAML value that is either a string or a sequence of strings.
fn yaml_string_list(v: &YamlOwned) -> Vec<String> {
    if let Some(s) = v.as_str() {
        return vec![s.to_string()];
    }
    if let Some(seq) = v.as_sequence() {
        return seq.iter().filter_map(|v| v.as_str().map(String::from)).collect();
    }
    vec![]
}

// ---------------------------------------------------------------------------
// Copyright, license, citation, funding parsing
// ---------------------------------------------------------------------------

/// Known Creative Commons abbreviation expansions.
fn expand_cc_license(s: &str) -> Option<(&'static str, &'static str)> {
    match s.to_uppercase().replace('-', " ").trim() {
        s if s == "CC0" => Some(("CC0 1.0 Universal", "https://creativecommons.org/publicdomain/zero/1.0/")),
        s if s == "CC BY" => Some(("Creative Commons Attribution 4.0", "https://creativecommons.org/licenses/by/4.0/")),
        s if s == "CC BY SA" => Some(("Creative Commons Attribution ShareAlike 4.0", "https://creativecommons.org/licenses/by-sa/4.0/")),
        s if s == "CC BY ND" => Some(("Creative Commons Attribution NoDerivatives 4.0", "https://creativecommons.org/licenses/by-nd/4.0/")),
        s if s == "CC BY NC" => Some(("Creative Commons Attribution NonCommercial 4.0", "https://creativecommons.org/licenses/by-nc/4.0/")),
        s if s == "CC BY NC SA" => Some(("Creative Commons Attribution NonCommercial ShareAlike 4.0", "https://creativecommons.org/licenses/by-nc-sa/4.0/")),
        s if s == "CC BY NC ND" => Some(("Creative Commons Attribution NonCommercial NoDerivatives 4.0", "https://creativecommons.org/licenses/by-nc-nd/4.0/")),
        _ => None,
    }
}

fn parse_copyright(v: &YamlOwned) -> Copyright {
    if let Some(s) = v.as_str() {
        return Copyright { statement: Some(s.to_string()), ..Default::default() };
    }
    if let Some(m) = v.as_mapping() {
        return Copyright {
            holder: yaml_str(m, "holder"),
            year: yaml_str(m, "year")
                .or_else(|| m.get(&yaml_key("year"))
                    .and_then(|v| v.as_integer()).map(|n| n.to_string())),
            statement: yaml_str(m, "statement"),
        };
    }
    Copyright::default()
}

fn parse_license(v: &YamlOwned) -> License {
    if let Some(s) = v.as_str() {
        return if let Some((text, url)) = expand_cc_license(s) {
            License { text: Some(text.to_string()), url: Some(url.to_string()), cc_type: Some(s.to_string()) }
        } else {
            License { text: Some(s.to_string()), ..Default::default() }
        };
    }
    if let Some(m) = v.as_mapping() {
        let mut lic = License {
            text: yaml_str(m, "text"),
            url: yaml_str(m, "url"),
            ..Default::default()
        };
        // If type: is a CC abbreviation, expand it
        if let Some(t) = yaml_str(m, "type") {
            if let Some((text, url)) = expand_cc_license(&t) {
                lic.cc_type = Some(t);
                if lic.text.is_none() { lic.text = Some(text.to_string()); }
                if lic.url.is_none() { lic.url = Some(url.to_string()); }
            } else if lic.text.is_none() {
                lic.text = Some(t);
            }
        }
        return lic;
    }
    License::default()
}

fn parse_citation(v: &YamlOwned) -> Option<CitationMeta> {
    let m = v.as_mapping()?;
    Some(CitationMeta {
        type_: yaml_str(m, "type"),
        container_title: yaml_str(m, "container-title"),
        volume: yaml_str(m, "volume")
            .or_else(|| m.get(&yaml_key("volume"))
                .and_then(|v| v.as_integer()).map(|n| n.to_string())),
        issue: yaml_str(m, "issue")
            .or_else(|| m.get(&yaml_key("issue"))
                .and_then(|v| v.as_integer()).map(|n| n.to_string())),
        issued: yaml_str(m, "issued"),
        doi: yaml_str(m, "doi"),
        url: yaml_str(m, "url"),
        issn: yaml_str(m, "issn"),
        isbn: yaml_str(m, "isbn"),
        publisher: yaml_str(m, "publisher"),
        page: yaml_str(m, "page"),
    })
}

fn parse_funding(v: &YamlOwned) -> Vec<Funding> {
    let entries: Vec<&YamlOwned> = if v.as_str().is_some() || v.as_mapping().is_some() {
        vec![v]
    } else if let Some(seq) = v.as_sequence() {
        seq.iter().collect()
    } else {
        return vec![];
    };
    entries.iter().map(|e| {
        if let Some(s) = e.as_str() {
            Funding { statement: Some(s.to_string()), ..Default::default() }
        } else if let Some(m) = e.as_mapping() {
            Funding {
                source: yaml_str(m, "source"),
                award: yaml_str(m, "award"),
                recipient: yaml_str(m, "recipient"),
                statement: yaml_str(m, "statement"),
            }
        } else {
            Funding::default()
        }
    }).collect()
}

// ---------------------------------------------------------------------------
// YAML value helpers (used by Metadata::apply_overrides)
// ---------------------------------------------------------------------------

/// Coerce a string value into the appropriate YAML scalar type.
/// "true"/"false" → Bool, integer → Number, float → Number, otherwise → String.
pub(crate) fn coerce_yaml_value(s: &str) -> YamlOwned {
    match s {
        "true" | "TRUE" | "True" => YamlOwned::Value(ScalarOwned::Boolean(true)),
        "false" | "FALSE" | "False" => YamlOwned::Value(ScalarOwned::Boolean(false)),
        "null" | "NULL" | "~" => YamlOwned::Value(ScalarOwned::Null),
        _ => {
            if let Ok(n) = s.parse::<i64>() {
                YamlOwned::Value(ScalarOwned::Integer(n))
            } else if let Ok(f) = s.parse::<f64>() {
                YamlOwned::Value(ScalarOwned::FloatingPoint(f.into()))
            } else {
                YamlOwned::Value(ScalarOwned::String(s.to_string()))
            }
        }
    }
}

/// Build a nested YamlOwned from dot-separated key parts.
/// `["a", "b", "c"]` with leaf `"val"` → `{"a": {"b": {"c": "val"}}}`.
/// Returns the value rooted at parts[1] (caller handles parts[0]).
pub(crate) fn build_nested_yaml(parts: &[&str], leaf: YamlOwned) -> YamlOwned {
    let mut val = leaf;
    for &part in parts[1..].iter().rev() {
        let mut map = MappingOwned::new();
        map.insert(yaml_key(part), val);
        val = YamlOwned::Mapping(map);
    }
    val
}

/// Merge a nested YAML value into the extra map at the given top-level key.
/// If the key already exists and both values are mappings, merge recursively.
pub(crate) fn merge_yaml_value(
    extra: &mut HashMap<String, YamlOwned>,
    key: &str,
    new_val: YamlOwned,
) {
    match extra.get_mut(key) {
        Some(YamlOwned::Mapping(existing)) => {
            if let YamlOwned::Mapping(new_map) = new_val {
                merge_yaml_mappings(existing, new_map);
            } else {
                extra.insert(key.to_string(), new_val);
            }
        }
        _ => {
            extra.insert(key.to_string(), new_val);
        }
    }
}

/// Recursively merge two YAML mappings. Values in `source` override `target`.
fn merge_yaml_mappings(target: &mut MappingOwned, source: MappingOwned) {
    for (k, v) in source {
        match (target.get_mut(&k), &v) {
            (Some(YamlOwned::Mapping(t)), YamlOwned::Mapping(s)) => {
                merge_yaml_mappings(t, s.clone());
            }
            _ => {
                target.insert(k, v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_yaml() {
        let text = "---\ntitle: Hello\nauthor: World\n---\n\n# Body\n\nSome text.";
        let (meta, body) = split_yaml(text).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Hello"));
        assert_eq!(meta.author, Some(vec!["World".to_string()]));
        assert!(body.starts_with("\n# Body"));
    }

    #[test]
    fn test_no_yaml() {
        let text = "# Just markdown\n\nNo front matter.";
        let (meta, body) = split_yaml(text).unwrap();
        assert!(meta.title.is_none());
        assert_eq!(body, text);
    }

    #[test]
    fn test_simple_author_string() {
        let text = "---\nauthor: Norah Jones\n---\nBody";
        let (meta, _) = split_yaml(text).unwrap();
        assert_eq!(meta.author, Some(vec!["Norah Jones".to_string()]));
        assert_eq!(meta.authors.len(), 1);
        assert_eq!(meta.authors[0].name.literal, "Norah Jones");
        assert_eq!(meta.authors[0].name.given.as_deref(), Some("Norah"));
        assert_eq!(meta.authors[0].name.family.as_deref(), Some("Jones"));
    }

    #[test]
    fn test_simple_author_list() {
        let text = "---\nauthor:\n  - Alice Smith\n  - Bob Lee\n---\nBody";
        let (meta, _) = split_yaml(text).unwrap();
        assert_eq!(meta.author, Some(vec!["Alice Smith".to_string(), "Bob Lee".to_string()]));
        assert_eq!(meta.authors.len(), 2);
    }

    #[test]
    fn test_rich_author_with_affiliations() {
        let text = "---\nauthor:\n  - name: Norah Jones\n    email: norah@example.com\n    orcid: 0000-0001-2345-6789\n    corresponding: true\n    affiliations:\n      - name: Carnegie Mellon University\n        city: Pittsburgh\n        state: PA\n---\nBody";
        let (meta, _) = split_yaml(text).unwrap();
        assert_eq!(meta.authors.len(), 1);
        assert_eq!(meta.authors[0].email.as_deref(), Some("norah@example.com"));
        assert_eq!(meta.authors[0].orcid.as_deref(), Some("0000-0001-2345-6789"));
        assert!(meta.authors[0].corresponding);
        assert_eq!(meta.authors[0].affiliation_ids, vec![0]);
        assert_eq!(meta.affiliations.len(), 1);
        assert_eq!(meta.affiliations[0].name.as_deref(), Some("Carnegie Mellon University"));
        assert_eq!(meta.affiliations[0].city.as_deref(), Some("Pittsburgh"));
        assert_eq!(meta.affiliations[0].region.as_deref(), Some("PA"));
        assert_eq!(meta.affiliations[0].number, 1);
    }

    #[test]
    fn test_shared_affiliations_via_ref() {
        let text = "---\nauthor:\n  - name: Alice\n    affiliations:\n      - ref: mit\n  - name: Bob\n    affiliations:\n      - ref: mit\naffiliations:\n  - id: mit\n    name: MIT\n    city: Cambridge\n---\nBody";
        let (meta, _) = split_yaml(text).unwrap();
        assert_eq!(meta.affiliations.len(), 1);
        assert_eq!(meta.authors[0].affiliation_ids, vec![0]);
        assert_eq!(meta.authors[1].affiliation_ids, vec![0]);
        assert_eq!(meta.affiliations[0].name.as_deref(), Some("MIT"));
    }

    #[test]
    fn test_multiple_affiliations() {
        let text = "---\nauthor:\n  - name: Alice\n    affiliations:\n      - MIT\n      - Stanford\n  - name: Bob\n    affiliations:\n      - Stanford\n---\nBody";
        let (meta, _) = split_yaml(text).unwrap();
        assert_eq!(meta.affiliations.len(), 2);
        assert_eq!(meta.authors[0].affiliation_ids, vec![0, 1]);
        assert_eq!(meta.authors[1].affiliation_ids, vec![1]); // deduplicated
    }

    #[test]
    fn test_yaml_block_scalar_with_dashes() {
        // Indented --- inside a block scalar should NOT terminate the front matter
        let text = "---\ntitle: Test\nheader-includes: |\n  some content\n  ---\n  more content\n---\nBody";
        let (meta, body) = split_yaml(text).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Test"));
        assert!(meta.header_includes.as_ref().unwrap().contains("more content"));
        assert_eq!(body, "Body");
    }

    #[test]
    fn test_format_mapping() {
        // format: { html: default } should extract "html"
        let text = "---\ntitle: Test\nformat:\n  html: default\n---\nBody";
        let (meta, _) = split_yaml(text).unwrap();
        assert_eq!(meta.format.as_deref(), Some("html"));
    }

    #[test]
    fn test_format_string() {
        let text = "---\nformat: latex\n---\nBody";
        let (meta, _) = split_yaml(text).unwrap();
        assert_eq!(meta.format.as_deref(), Some("latex"));
    }

    #[test]
    fn test_bibliography_list() {
        let text = "---\nbibliography:\n  - refs.bib\n  - extra.bib\n---\nBody";
        let (meta, _) = split_yaml(text).unwrap();
        assert_eq!(meta.bibliography, vec!["refs.bib", "extra.bib"]);
    }

    #[test]
    fn test_bibliography_string() {
        let text = "---\nbibliography: refs.bib\n---\nBody";
        let (meta, _) = split_yaml(text).unwrap();
        assert_eq!(meta.bibliography, vec!["refs.bib"]);
    }
}
