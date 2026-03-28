use anyhow::Result;
use std::collections::HashMap;

use serde::de::DeserializeOwned;

use crate::value::{Value, Table, table_get, table_str, table_bool, value_string_list};
use super::{Affiliation, Author, AuthorName, CitationConfig, Copyright, Funding, License, Metadata, TocConfig};

/// Deserialize a Value into a typed struct via serde_json roundtrip.
/// Normalizes all keys (dashes/dots to underscores) before deserializing.
/// Returns Some(T) on success, None on failure (silently drops parse errors).
fn deserialize_section<T: DeserializeOwned>(v: &Value) -> Option<T> {
    let normalized = normalize_keys(v);
    let json = crate::value::to_json(&normalized);
    serde_json::from_value(json).ok()
}

/// Recursively normalize all keys in a Value tree (dashes/dots to underscores).
fn normalize_keys(v: &Value) -> Value {
    match v {
        Value::Table(table) => {
            let normalized: crate::value::Table = table.iter()
                .map(|(k, v)| (crate::util::normalize_key(k), normalize_keys(v)))
                .collect();
            Value::Table(normalized)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(normalize_keys).collect()),
        other => other.clone(),
    }
}

/// Parse TOML front matter from the document and return (metadata, body).
/// Front matter is delimited by `---` (opening) and `---` or `...` (closing).
/// If the front matter block is empty or absent, returns default metadata.
#[inline(never)]
pub fn split_frontmatter(text: &str) -> Result<(Metadata, String)> {
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

    let raw = lines[1..end].join("\n");
    let body: String = lines[end + 1..].join("\n");

    if raw.trim().is_empty() {
        return Ok((Metadata::default(), body));
    }

    // Parse as TOML; fall back to simple YAML key: value parsing for basic fields
    let meta = match crate::value::parse_frontmatter(&raw) {
        Ok(table) => parse_metadata(&table).unwrap_or_default(),
        Err(_) => parse_yaml_simple(&raw),
    };
    Ok((meta, body))
}

/// Parse simple YAML-style `key: value` lines as a fallback when TOML parsing fails.
/// Only recognizes: title, author, date, bibliography. Warns on unknown fields.
fn parse_yaml_simple(raw: &str) -> Metadata {
    const ALLOWED: &[&str] = &["title", "author", "date", "bibliography"];
    let mut meta = Metadata::default();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Match `key: value` pattern
        let Some((key, val)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let val = val.trim();
        // Strip surrounding quotes (single or double)
        let val = val.strip_prefix('"').and_then(|v| v.strip_suffix('"'))
            .or_else(|| val.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(val);

        if !ALLOWED.contains(&key) {
            cwarn!("ignoring unsupported front matter field: {}", key);
            continue;
        }

        match key {
            "title" => meta.title = Some(val.to_string()),
            "author" => {
                let name = parse_author_name_str(val);
                meta.authors = vec![Author { name, ..Default::default() }];
            }
            "date" => meta.date = Some(val.to_string()),
            "bibliography" => {
                meta.bibliography = vec![val.to_string()];
            }
            _ => {}
        }
    }

    meta
}

pub fn parse_metadata(table: &Table) -> Result<Metadata> {
    let mut meta = Metadata::default();
    let mut extra = HashMap::new();

    // First pass: collect top-level affiliations (needed for ref: lookups)
    let top_level_affiliations = table_get(table, "affiliations")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    for (key, v) in table {
        let key = crate::util::normalize_key(key);
        match key.as_str() {
            "title" => meta.title = v.as_str().map(String::from),
            "subtitle" => meta.subtitle = v.as_str().map(String::from),
            "author" | "authors" => {
                parse_authors(v, &mut meta, &top_level_affiliations);
            }
            "affiliations" => {} // handled above
            "date" => meta.date = v.as_str().map(String::from),
            "abstract" => meta.abstract_text = v.as_str().map(String::from),
            "keywords" => {
                meta.keywords = value_string_list(v);
            }
            "copyright" => meta.copyright = Some(parse_copyright(v)),
            "license" => meta.license = Some(parse_license(v)),
            "citation" => meta.citation = parse_citation(v),
            "funding" => meta.funding = parse_funding(v),
            "appendix_style" => meta.appendix_style = v.as_str().map(String::from),
            "target" | "format" => {
                let val = v.as_str().map(String::from).or_else(|| {
                    // Support `target: { html: default }` or [target]\n html = "default"
                    v.as_table()
                        .and_then(|t| t.first())
                        .map(|(k, _)| k.clone())
                });
                meta.target = val;
            }
            "number_sections" => meta.number_sections = v.as_bool().unwrap_or(false),
            "toc_depth" => {
                let depth = v.as_integer().unwrap_or(3) as u32;
                let toc = meta.toc.get_or_insert_with(TocConfig::default);
                toc.depth = Some(depth);
            }
            "toc_title" => {
                let title = v.as_str().map(String::from);
                if let Some(t) = title {
                    let toc = meta.toc.get_or_insert_with(TocConfig::default);
                    toc.title = Some(t);
                }
            }
            "date_format" => meta.date_format = v.as_str().map(String::from),
            "bibliography" => {
                meta.bibliography = value_string_list(v);
            }
            "csl" => {
                meta.csl = v.as_str().map(String::from);
            }
            "html_math_method" => meta.html_math_method = v.as_str().map(String::from),
            // Project-level fields (also valid in front matter)
            "output" => meta.output = v.as_str().map(String::from),
            "lang" => {
                meta.lang = v.as_str().map(String::from);
            }
            "translations" => {
                if let Some(table) = v.as_table() {
                    let mut map = std::collections::HashMap::new();
                    for (k, val) in table {
                        if let Some(s) = val.as_str() {
                            map.insert(k.clone(), s.to_string());
                        }
                    }
                    if !map.is_empty() {
                        meta.translations = Some(map);
                    }
                }
            }
            "url" => meta.url = v.as_str().map(String::from),
            "favicon" => meta.favicon = v.as_str().map(String::from),
            "navbar" => meta.navbar = deserialize_section(v),
            "orchestrator" => meta.orchestrator = v.as_str().map(String::from),
            "global_crossref" => meta.global_crossref = v.as_bool().unwrap_or(false),
            "embed_resources" => {
                meta.embed_resources = Some(v.as_bool().unwrap_or(false));
            }
            "number_offset" => {} // accepted but handled elsewhere
            "calepin" => {
                if let Some(cmap) = v.as_table() {
                    if let Some(pv) = table_get(cmap, "plugins") {
                        meta.plugins = value_string_list(pv);
                    }
                    if let Some(cm) = table_get(cmap, "convert_math")
                        .or_else(|| table_get(cmap, "convert-math"))
                    {
                        meta.convert_math = cm.as_bool().unwrap_or(false);
                    }
                }
            }

            // -- Defaults sections --
            "dpi" => meta.dpi = v.as_floating_point(),
            "math" => meta.math = v.as_str().map(String::from),
            "preview_port" => meta.preview_port = v.as_integer().map(|n| n as u16),
            "highlight" => meta.highlight = deserialize_section(v),
            "toc" => {
                // "toc" can be a bool (in front matter) or a table (in config)
                if let Some(b) = v.as_bool() {
                    let toc = meta.toc.get_or_insert_with(TocConfig::default);
                    toc.enabled = Some(b);
                } else {
                    meta.toc = deserialize_section(v);
                }
            }
            "labels" => meta.labels = deserialize_section(v),
            "execute" => meta.execute = deserialize_section(v),
            "figure" => meta.figure = deserialize_section(v),
            "layout" => meta.layout = deserialize_section(v),
            "video" => meta.video = deserialize_section(v),
            "placeholder" => meta.placeholder = deserialize_section(v),
            "lipsum" => meta.lipsum = deserialize_section(v),
            // -- Collection structure (deserialized via serde_json) --
            "targets" => {
                let json = crate::value::to_json(&normalize_keys(v));
                if let Ok(t) = serde_json::from_value(json) {
                    meta.targets = t;
                }
            }
            "contents" => {
                let json = crate::value::to_json(&normalize_keys(v));
                if let Ok(c) = serde_json::from_value(json) {
                    meta.contents = c;
                }
            }
            "languages" => {
                let json = crate::value::to_json(&normalize_keys(v));
                if let Ok(l) = serde_json::from_value(json) {
                    meta.languages = l;
                }
            }
            "post" => {
                let json = crate::value::to_json(&normalize_keys(v));
                if let Ok(p) = serde_json::from_value(json) {
                    meta.post = p;
                }
            }
            "static" => {
                meta.static_dirs = value_string_list(v);
            }
            "exclude" => {
                meta.exclude = value_string_list(v);
            }
            "var" => {
                if let Some(t) = v.as_table() {
                    for (k, val) in t {
                        meta.var.insert(k.clone(), val.clone());
                    }
                }
            }

            _ => {
                extra.insert(key.to_string(), v.clone());
            }
        }
    }
    meta.var = extra;

    Ok(meta)
}

// ---------------------------------------------------------------------------
// Rich author / affiliation parsing
// ---------------------------------------------------------------------------

/// Parse the `author:` or `authors:` value into rich `Author` structs
/// and a flat, deduplicated affiliation list.
fn parse_authors(
    v: &Value,
    meta: &mut Metadata,
    top_level_affiliations: &[Value],
) {
    let entries: Vec<&Value> = match v {
        Value::String(_) => vec![v],
        Value::Table(_) => vec![v],
        Value::Array(seq) => seq.iter().collect(),
        _ => return,
    };

    let mut authors: Vec<Author> = Vec::new();
    let mut affiliations: Vec<Affiliation> = Vec::new();
    let mut id_map: HashMap<String, usize> = HashMap::new();
    for entry in entries {
        if let Some(s) = entry.as_str() {
            let name = parse_author_name_str(s);
            authors.push(Author { name, ..Default::default() });
        } else if let Some(t) = entry.as_table() {
            let author = parse_author_mapping(t, &mut affiliations, &mut id_map, top_level_affiliations);
            authors.push(author);
        }
    }

    // Number affiliations
    for (i, aff) in affiliations.iter_mut().enumerate() {
        aff.number = i + 1;
    }

    meta.authors = authors;
    meta.affiliations = affiliations;
}

/// Parse a single author name string into given/family/literal components.
/// "Last, First" -> given=First, family=Last, literal="First Last"
/// "First Last"  -> given=First, family=Last, literal="First Last"
fn parse_author_name_str(s: &str) -> AuthorName {
    let s = s.trim();
    if s.contains(',') {
        let mut parts = s.splitn(2, ',');
        let family = parts.next().unwrap_or("").trim().to_string();
        let given = parts.next().unwrap_or("").trim().to_string();
        let literal = if given.is_empty() {
            family.clone()
        } else {
            format!("{} {}", given, family)
        };
        AuthorName {
            literal,
            given: if given.is_empty() { None } else { Some(given) },
            family: Some(family),
        }
    } else {
        // "First Middle Last" -> given = everything before last word, family = last word
        let words: Vec<&str> = s.split_whitespace().collect();
        let (given, family) = if words.len() >= 2 {
            let family = words.last().unwrap().to_string();
            let given = words[..words.len() - 1].join(" ");
            (Some(given), Some(family))
        } else {
            (None, None)
        };
        AuthorName { literal: s.to_string(), given, family }
    }
}

/// Parse a mapping-form author entry into an `Author`.
fn parse_author_mapping(
    m: &Table,
    affiliations: &mut Vec<Affiliation>,
    id_map: &mut HashMap<String, usize>,
    top_level_affiliations: &[Value],
) -> Author {
    let mut author = Author::default();

    // Name
    if let Some(name_val) = table_get(m, "name") {
        if let Some(s) = name_val.as_str() {
            author.name = parse_author_name_str(s);
        } else if let Some(nm) = name_val.as_table() {
            let given = table_str(nm, "given");
            let family = table_str(nm, "family");
            let literal = table_str(nm, "literal").unwrap_or_else(|| {
                match (&given, &family) {
                    (Some(g), Some(f)) => format!("{} {}", g, f),
                    (Some(g), None) => g.clone(),
                    (None, Some(f)) => f.clone(),
                    (None, None) => String::new(),
                }
            });
            author.name = AuthorName { literal, given, family };
        }
    }

    // Scalar fields
    author.email = table_str(m, "email");
    author.url = table_str(m, "url");
    author.orcid = table_str(m, "orcid");
    author.note = table_str(m, "note");

    // Attributes (can appear at top level or under "attributes")
    author.corresponding = table_bool(m, "corresponding");
    author.equal_contributor = table_bool(m, "equal-contributor");
    author.deceased = table_bool(m, "deceased");
    if let Some(attrs) = table_get(m, "attributes") {
        if let Some(am) = attrs.as_table() {
            if table_bool(am, "corresponding") { author.corresponding = true; }
            if table_bool(am, "equal-contributor") { author.equal_contributor = true; }
            if table_bool(am, "deceased") { author.deceased = true; }
        }
    }

    // Roles
    let role_key = table_get(m, "roles");
    if let Some(rv) = role_key {
        if let Some(s) = rv.as_str() {
            author.roles.push(s.to_string());
        } else if let Some(seq) = rv.as_array() {
            for item in seq {
                if let Some(s) = item.as_str() {
                    author.roles.push(s.to_string());
                }
            }
        }
    }

    // Affiliations
    let aff_key = table_get(m, "affiliations");
    if let Some(aff_val) = aff_key {
        let aff_entries: Vec<&Value> = if aff_val.as_str().is_some() || aff_val.as_table().is_some() {
            vec![aff_val]
        } else if let Some(seq) = aff_val.as_array() {
            seq.iter().collect()
        } else {
            vec![]
        };
        for entry in aff_entries {
            let idx = resolve_affiliation(entry, affiliations, id_map, top_level_affiliations);
            if let Some(i) = idx {
                author.affiliation_ids.push(i);
            }
        }
    }

    author
}

/// Resolve an affiliation entry to an index in the affiliations vec.
/// `id_map` tracks id -> index for deduplication during parsing.
fn resolve_affiliation(
    entry: &Value,
    affiliations: &mut Vec<Affiliation>,
    id_map: &mut HashMap<String, usize>,
    top_level: &[Value],
) -> Option<usize> {
    if let Some(s) = entry.as_str() {
        if let Some(idx) = affiliations.iter().position(|a| a.name.as_deref() == Some(s)) {
            return Some(idx);
        }
        let aff = Affiliation { name: Some(s.to_string()), ..Default::default() };
        affiliations.push(aff);
        return Some(affiliations.len() - 1);
    }
    if let Some(m) = entry.as_table() {
        // Check for ref:
        if let Some(ref_val) = table_str(m, "ref") {
            for tl in top_level {
                if let Some(tlm) = tl.as_table() {
                    if table_str(tlm, "id").as_deref() == Some(ref_val.as_str()) {
                        return resolve_affiliation(tl, affiliations, id_map, &[]);
                    }
                }
            }
            if let Some(&idx) = id_map.get(&ref_val) {
                return Some(idx);
            }
            return None;
        }
        // Inline affiliation
        let id = table_str(m, "id");
        let name = table_str(m, "name");
        if let Some(ref id_str) = id {
            if let Some(&idx) = id_map.get(id_str) {
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
            name,
            department: table_str(m, "department"),
            city: table_str(m, "city"),
            region: table_str(m, "region"),
            country: table_str(m, "country"),
            ..Default::default()
        };
        let idx = affiliations.len();
        affiliations.push(aff);
        if let Some(id_str) = id {
            id_map.insert(id_str, idx);
        }
        return Some(idx);
    }
    None
}

// ---------------------------------------------------------------------------
// Copyright, license, citation, funding parsing
// ---------------------------------------------------------------------------

fn resolve_cc_license(s: &str) -> Option<(&'static str, &'static str)> {
    let normalized = s.to_uppercase().replace('-', " ");
    match normalized.trim() {
        "CC0" => Some(("CC0 1.0 Universal", "https://creativecommons.org/publicdomain/zero/1.0/")),
        "CC BY" => Some(("Creative Commons Attribution 4.0", "https://creativecommons.org/licenses/by/4.0/")),
        "CC BY SA" => Some(("Creative Commons Attribution ShareAlike 4.0", "https://creativecommons.org/licenses/by-sa/4.0/")),
        "CC BY ND" => Some(("Creative Commons Attribution NoDerivatives 4.0", "https://creativecommons.org/licenses/by-nd/4.0/")),
        "CC BY NC" => Some(("Creative Commons Attribution NonCommercial 4.0", "https://creativecommons.org/licenses/by-nc/4.0/")),
        "CC BY NC SA" => Some(("Creative Commons Attribution NonCommercial ShareAlike 4.0", "https://creativecommons.org/licenses/by-nc-sa/4.0/")),
        "CC BY NC ND" => Some(("Creative Commons Attribution NonCommercial NoDerivatives 4.0", "https://creativecommons.org/licenses/by-nc-nd/4.0/")),
        _ => None,
    }
}

fn parse_copyright(v: &Value) -> Copyright {
    if let Some(s) = v.as_str() {
        return Copyright { statement: Some(s.to_string()), ..Default::default() };
    }
    if let Some(m) = v.as_table() {
        return Copyright {
            holder: table_str(m, "holder"),
            year: table_str(m, "year")
                .or_else(|| table_get(m, "year")
                    .and_then(|v| v.as_integer()).map(|n| n.to_string())),
            statement: table_str(m, "statement"),
        };
    }
    Copyright::default()
}

fn parse_license(v: &Value) -> License {
    if let Some(s) = v.as_str() {
        return if let Some((text, url)) = resolve_cc_license(s) {
            License { text: Some(text.to_string()), url: Some(url.to_string()) }
        } else {
            License { text: Some(s.to_string()), ..Default::default() }
        };
    }
    if let Some(m) = v.as_table() {
        let mut lic = License {
            text: table_str(m, "text"),
            url: table_str(m, "url"),
            ..Default::default()
        };
        if let Some(t) = table_str(m, "type") {
            if let Some((text, url)) = resolve_cc_license(&t) {
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

fn parse_citation(v: &Value) -> Option<CitationConfig> {
    let m = v.as_table()?;
    Some(CitationConfig {
        container_title: table_str(m, "container-title"),
        volume: table_str(m, "volume")
            .or_else(|| table_get(m, "volume")
                .and_then(|v| v.as_integer()).map(|n| n.to_string())),
        issue: table_str(m, "issue")
            .or_else(|| table_get(m, "issue")
                .and_then(|v| v.as_integer()).map(|n| n.to_string())),
        issued: table_str(m, "issued"),
        doi: table_str(m, "doi"),
        url: table_str(m, "url"),
        page: table_str(m, "page"),
    })
}

fn parse_funding(v: &Value) -> Vec<Funding> {
    let entries: Vec<&Value> = if v.as_str().is_some() || v.as_table().is_some() {
        vec![v]
    } else if let Some(seq) = v.as_array() {
        seq.iter().collect()
    } else {
        return vec![];
    };
    entries.iter().map(|e| {
        if let Some(s) = e.as_str() {
            Funding { statement: Some(s.to_string()), ..Default::default() }
        } else if let Some(m) = e.as_table() {
            Funding {
                source: table_str(m, "source"),
                award: table_str(m, "award"),
                recipient: table_str(m, "recipient"),
                statement: table_str(m, "statement"),
            }
        } else {
            Funding::default()
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_frontmatter_parses_toml() {
        let text = "---\ntitle = \"Hello\"\nauthor = \"World\"\n---\n\n# Body\n\nSome text.";
        let (meta, body) = split_frontmatter(text).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Hello"));
        assert_eq!(meta.author_names(), vec!["World"]);
        assert!(body.starts_with("\n# Body"));
    }

    #[test]
    fn test_split_frontmatter_empty_block() {
        let text = "---\n---\n\n# Body";
        let (meta, body) = split_frontmatter(text).unwrap();
        assert!(meta.title.is_none());
        assert!(body.starts_with("\n# Body"));
    }

    #[test]
    fn test_split_frontmatter_yaml_fallback() {
        let text = "---\ntitle: Hello\nauthor: World\n---\n\n# Body";
        let (meta, body) = split_frontmatter(text).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Hello"));
        assert_eq!(meta.author_names(), vec!["World"]);
        assert!(body.starts_with("\n# Body"));
    }

    #[test]
    fn test_split_frontmatter_yaml_quoted() {
        let text = "---\ntitle: \"Hello World\"\nauthor: 'Jane Doe'\n---\nBody";
        let (meta, _body) = split_frontmatter(text).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Hello World"));
        assert_eq!(meta.author_names(), vec!["Jane Doe"]);
    }

    #[test]
    fn test_split_frontmatter_yaml_bibliography() {
        let text = "---\ntitle: Test\nbibliography: refs.bib\ndate: 2025-01-01\n---\nBody";
        let (meta, _body) = split_frontmatter(text).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Test"));
        assert_eq!(meta.bibliography, vec!["refs.bib"]);
        assert_eq!(meta.date.as_deref(), Some("2025-01-01"));
    }

    #[test]
    fn test_no_frontmatter() {
        let text = "# Just markdown\n\nNo front matter.";
        let (meta, body) = split_frontmatter(text).unwrap();
        assert!(meta.title.is_none());
        assert_eq!(body, text);
    }

    #[test]
    fn test_parse_metadata_from_toml_table() {
        let table = crate::value::parse_frontmatter("title = \"Hello\"\nauthor = \"World\"\nformat = \"html\"").unwrap();
        let meta = parse_metadata(&table).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Hello"));
        assert_eq!(meta.author_names(), vec!["World"]);
        assert_eq!(meta.target.as_deref(), Some("html"));
    }

    #[test]
    fn test_parse_metadata_nested_toml() {
        let table = crate::value::parse_frontmatter("[calepin]\nplugins = [\"txtfmt\"]").unwrap();
        let meta = parse_metadata(&table).unwrap();
        assert_eq!(meta.plugins, vec!["txtfmt"]);
    }
}
