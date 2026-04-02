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

    // Parse as TOML; fall back to YAML via the quarto compatibility layer
    let meta = match crate::value::parse_frontmatter(&raw) {
        Ok(table) => parse_metadata(&table).unwrap_or_default(),
        Err(_) => match crate::quarto::parse_yaml(&raw) {
            Ok(table) => parse_metadata(&table).unwrap_or_default(),
            Err(_) => Metadata::default(),
        },
    };
    Ok((meta, body))
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
            "target" => {
                meta.target = v.as_str().map(String::from);
            }
            "format" => {
                cwarn!("`format` is deprecated, use `target` instead");
                if meta.target.is_none() {
                    meta.target = v.as_str().map(String::from);
                }
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
            "html_math_method" => {} // deprecated: override math.html template instead
            // Project-level fields (also valid in front matter)
            "output" => meta.output = v.as_str().map(String::from),
            "lang" => {
                meta.lang = v.as_str().map(String::from);
            }
            "translations" => {} // deprecated: use [[languages]] path instead
            "url" => meta.url = v.as_str().map(String::from),
            "favicon" => meta.favicon = v.as_str().map(String::from),
            "navbar" => meta.navbar = deserialize_section(v),
            "orchestrator" => meta.orchestrator = v.as_str().map(String::from),
            "global_crossref" => meta.global_crossref = v.as_bool().unwrap_or(false),
            "standalone" => {
                meta.standalone = Some(v.as_bool().unwrap_or(false));
            }
            "number_offset" => {} // accepted but handled elsewhere
            "calepin" => {
                if let Some(cmap) = v.as_table() {
                    if let Some(pv) = table_get(cmap, "plugins") {
                        let plugins = value_string_list(pv);
                        meta.plugins = plugins.clone();
                        // plugins is an alias for extensions
                        for p in &plugins {
                            if !meta.extensions.contains(p) {
                                meta.extensions.push(p.clone());
                            }
                        }
                    }
                    if let Some(ev) = table_get(cmap, "extensions") {
                        let exts = value_string_list(ev);
                        for e in exts {
                            if !meta.extensions.contains(&e) {
                                meta.extensions.push(e);
                            }
                        }
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

            "toc" => {
                // "toc" can be a bool (in front matter) or a table (in config).
                // When a bare bool, also propagate to var.toc so templates
                // (e.g. website base.html) can use {{ var.toc }} consistently.
                if let Some(b) = v.as_bool() {
                    let toc = meta.toc.get_or_insert_with(TocConfig::default);
                    toc.enabled = Some(b);
                    meta.cfg.entry("toc".to_string()).or_insert(v.clone());
                } else {
                    meta.toc = deserialize_section(v);
                }
            }
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
            "tpl" => {
                if let Some(t) = v.as_table() {
                    for (k, val) in t {
                        if let Some(s) = val.as_str() {
                            meta.tpl.insert(k.clone(), s.to_string());
                        }
                    }
                }
            }

            _ => {
                extra.insert(key.to_string(), v.clone());
            }
        }
    }
    // Extra top-level fields are accessible as {{ cfg.key }}.
    for (k, v) in extra {
        meta.cfg.entry(k).or_insert(v);
    }

    // Also expose all top-level keys as cfg.* in templates.
    // Known fields get their structured parsing above AND are available
    // for template access here. Explicit [var] entries take precedence.
    for (key, v) in table {
        let key = crate::util::normalize_key(key);
        meta.cfg.entry(key).or_insert(v.clone());
    }

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

    // -----------------------------------------------------------------------
    // End-to-end YAML front matter tests (through split_frontmatter)
    // -----------------------------------------------------------------------

    #[test]
    fn test_yaml_format_becomes_target() {
        let text = "---\ntitle: Hello\nformat: html\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.target.as_deref(), Some("html"));
    }

    #[test]
    fn test_yaml_toc_bool() {
        let text = "---\ntoc: true\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert!(meta.toc.is_some());
        assert_eq!(meta.toc.as_ref().unwrap().enabled, Some(true));
    }

    #[test]
    fn test_yaml_toc_false() {
        let text = "---\ntoc: false\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert!(meta.toc.is_some());
        assert_eq!(meta.toc.as_ref().unwrap().enabled, Some(false));
    }

    #[test]
    fn test_yaml_nested_execute() {
        let text = "---\nexecute:\n  echo: false\n  eval: true\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        let exec = meta.execute.unwrap();
        assert_eq!(exec.echo, Some(false));
        assert_eq!(exec.eval, Some(true));
    }

    #[test]
    fn test_yaml_bibliography_list() {
        let text = "---\nbibliography:\n  - refs.bib\n  - extra.bib\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.bibliography, vec!["refs.bib", "extra.bib"]);
    }

    #[test]
    fn test_yaml_number_sections_with_dashes() {
        let text = "---\nnumber-sections: true\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert!(meta.number_sections);
    }

    #[test]
    fn test_yaml_unknown_keys_pass_through_to_cfg() {
        let text = "---\ntitle: Hi\ncustom_field: my_value\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Hi"));
        // Unknown keys land in cfg for template access
        let val = meta.cfg.get("custom_field");
        assert!(val.is_some());
        assert_eq!(val.unwrap().as_str(), Some("my_value"));
    }

    #[test]
    fn test_yaml_target_takes_precedence_over_format() {
        let text = "---\ntarget: latex\nformat: html\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        // target should win; format should not overwrite it
        assert_eq!(meta.target.as_deref(), Some("latex"));
    }

    #[test]
    fn test_yaml_structured_authors() {
        let text = "---\nauthor:\n  - name: Jane Doe\n    email: jane@example.com\n  - name: John Smith\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.authors.len(), 2);
        assert_eq!(meta.author_names(), vec!["Jane Doe", "John Smith"]);
        assert_eq!(meta.authors[0].email.as_deref(), Some("jane@example.com"));
    }

    #[test]
    fn test_yaml_date_not_mangled() {
        // YAML parses 2024-01-01 as a string (serde_yaml behavior)
        let text = "---\ndate: 2024-01-01\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.date.as_deref(), Some("2024-01-01"));
    }

    #[test]
    fn test_yaml_keywords() {
        let text = "---\nkeywords:\n  - rust\n  - cli\n  - rendering\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.keywords, vec!["rust", "cli", "rendering"]);
    }

    #[test]
    fn test_yaml_lang() {
        let text = "---\nlang: fr\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.lang.as_deref(), Some("fr"));
    }

    #[test]
    fn test_yaml_csl() {
        let text = "---\ncsl: apa.csl\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.csl.as_deref(), Some("apa.csl"));
    }

    #[test]
    fn test_yaml_subtitle_and_abstract() {
        let text = "---\ntitle: Main\nsubtitle: Sub\nabstract: Some abstract text\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.subtitle.as_deref(), Some("Sub"));
        assert_eq!(meta.abstract_text.as_deref(), Some("Some abstract text"));
    }

    // -----------------------------------------------------------------------
    // YAML author/affiliation integration tests (Quarto compatibility)
    // -----------------------------------------------------------------------

    #[test]
    fn test_yaml_author_single_string() {
        let text = "---\nauthor: Jane Doe\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.authors.len(), 1);
        assert_eq!(meta.author_names(), vec!["Jane Doe"]);
    }

    #[test]
    fn test_yaml_author_list_of_strings() {
        let text = "---\nauthor:\n  - Jane Doe\n  - John Smith\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.authors.len(), 2);
        assert_eq!(meta.author_names(), vec!["Jane Doe", "John Smith"]);
    }

    #[test]
    fn test_yaml_author_structured_name() {
        let text = "---\nauthor:\n  - name:\n      given: Jane\n      family: Doe\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.authors.len(), 1);
        assert_eq!(meta.authors[0].name.given.as_deref(), Some("Jane"));
        assert_eq!(meta.authors[0].name.family.as_deref(), Some("Doe"));
        assert_eq!(meta.authors[0].name.literal, "Jane Doe");
    }

    #[test]
    fn test_yaml_author_with_orcid_and_roles() {
        let text = "---\nauthor:\n  - name: Jane Doe\n    orcid: 0000-0000-0000-0001\n    roles:\n      - writing\n      - methodology\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.authors[0].orcid.as_deref(), Some("0000-0000-0000-0001"));
        assert_eq!(meta.authors[0].roles, vec!["writing", "methodology"]);
    }

    #[test]
    fn test_yaml_author_corresponding_and_flags() {
        let text = "---\nauthor:\n  - name: Jane Doe\n    corresponding: true\n    equal-contributor: true\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert!(meta.authors[0].corresponding);
        assert!(meta.authors[0].equal_contributor);
    }

    #[test]
    fn test_yaml_author_affiliation_singular_string() {
        // Quarto accepts `affiliation` (singular) on author entries
        let text = "---\nauthor:\n  - name: Jane Doe\n    affiliation: MIT\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.affiliations.len(), 1);
        assert_eq!(meta.affiliations[0].name.as_deref(), Some("MIT"));
        assert_eq!(meta.authors[0].affiliation_ids, vec![0]);
    }

    #[test]
    fn test_yaml_author_affiliation_singular_list() {
        let text = "---\nauthor:\n  - name: Jane Doe\n    affiliation:\n      - MIT\n      - Stanford\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.affiliations.len(), 2);
        assert_eq!(meta.authors[0].affiliation_ids, vec![0, 1]);
    }

    #[test]
    fn test_yaml_inline_affiliation_with_department() {
        let text = "---\nauthor:\n  - name: Jane Doe\n    affiliations:\n      - name: MIT\n        department: CSAIL\n        city: Cambridge\n        country: US\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.affiliations.len(), 1);
        assert_eq!(meta.affiliations[0].name.as_deref(), Some("MIT"));
        assert_eq!(meta.affiliations[0].department.as_deref(), Some("CSAIL"));
        assert_eq!(meta.affiliations[0].city.as_deref(), Some("Cambridge"));
        assert_eq!(meta.affiliations[0].country.as_deref(), Some("US"));
    }

    #[test]
    fn test_yaml_top_level_affiliations_with_ref() {
        let text = "---\nauthor:\n  - name: Jane Doe\n    affiliations:\n      - ref: stanford\n  - name: John Smith\n    affiliations:\n      - ref: stanford\naffiliations:\n  - id: stanford\n    name: Stanford University\n    department: Statistics\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.affiliations.len(), 1);
        assert_eq!(meta.affiliations[0].name.as_deref(), Some("Stanford University"));
        assert_eq!(meta.affiliations[0].department.as_deref(), Some("Statistics"));
        // Both authors should reference the same affiliation
        assert_eq!(meta.authors[0].affiliation_ids, vec![0]);
        assert_eq!(meta.authors[1].affiliation_ids, vec![0]);
    }

    #[test]
    fn test_yaml_multiple_affiliations_with_ref() {
        let text = "---\nauthor:\n  - name: Jane Doe\n    affiliations:\n      - ref: mit\n      - ref: stanford\naffiliations:\n  - id: mit\n    name: MIT\n  - id: stanford\n    name: Stanford\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.affiliations.len(), 2);
        assert_eq!(meta.authors[0].affiliation_ids, vec![0, 1]);
    }

    #[test]
    fn test_yaml_institutes_alias() {
        let text = "---\nauthor:\n  - name: Jane Doe\n    affiliations:\n      - ref: mit\ninstitutes:\n  - id: mit\n    name: MIT\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.affiliations.len(), 1);
        assert_eq!(meta.affiliations[0].name.as_deref(), Some("MIT"));
        assert_eq!(meta.authors[0].affiliation_ids, vec![0]);
    }

    #[test]
    fn test_yaml_shared_affiliation_deduplication() {
        // Two authors referencing the same affiliation string should share one entry
        let text = "---\nauthor:\n  - name: Jane Doe\n    affiliations:\n      - MIT\n  - name: John Smith\n    affiliations:\n      - MIT\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.affiliations.len(), 1);
        assert_eq!(meta.authors[0].affiliation_ids, vec![0]);
        assert_eq!(meta.authors[1].affiliation_ids, vec![0]);
    }

    #[test]
    fn test_yaml_full_quarto_scholarly() {
        let text = r#"---
title: "A Study of Things"
author:
  - name: Jane Doe
    email: jane@example.com
    orcid: 0000-0000-0000-0001
    corresponding: true
    affiliations:
      - ref: stanford
  - name: John Smith
    affiliations:
      - ref: stanford
      - ref: mit
affiliations:
  - id: stanford
    name: Stanford University
    department: Department of Statistics
    city: Stanford
    region: CA
    country: US
  - id: mit
    name: MIT
    department: Department of Mathematics
    city: Cambridge
    region: MA
    country: US
---
Body"#;
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.title.as_deref(), Some("A Study of Things"));
        assert_eq!(meta.authors.len(), 2);
        assert!(meta.authors[0].corresponding);
        assert_eq!(meta.authors[0].email.as_deref(), Some("jane@example.com"));
        assert_eq!(meta.affiliations.len(), 2);
        assert_eq!(meta.affiliations[0].name.as_deref(), Some("Stanford University"));
        assert_eq!(meta.affiliations[0].region.as_deref(), Some("CA"));
        assert_eq!(meta.affiliations[1].name.as_deref(), Some("MIT"));
        // Jane -> Stanford only
        assert_eq!(meta.authors[0].affiliation_ids, vec![0]);
        // John -> Stanford + MIT
        assert_eq!(meta.authors[1].affiliation_ids, vec![0, 1]);
    }
}
