use anyhow::Result;
use std::collections::HashMap;

use crate::value::{self, Value, Table, table_get, table_str, table_bool, value_string_list};
use crate::types::{Affiliation, Author, AuthorName, CitationMeta, Copyright, Funding, License, Metadata};

/// Split front matter from the document body.
/// Returns (metadata, body_text).
/// Front matter is delimited by `---` and auto-detected as TOML or minimal YAML.
#[inline(never)]
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

    let fm_str: String = lines[1..end].join("\n");
    let body: String = lines[end + 1..].join("\n");

    let table = value::parse_frontmatter(&fm_str)?;
    let metadata = parse_metadata(&table)?;
    Ok((metadata, body))
}

fn parse_metadata(table: &Table) -> Result<Metadata> {
    let mut meta = Metadata::default();
    let mut extra = HashMap::new();

    // First pass: collect top-level affiliations (needed for ref: lookups)
    let top_level_affiliations = table_get(table, "affiliations")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    for (key, v) in table {
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
            "appendix-style" => meta.appendix_style = v.as_str().map(String::from),
            "target" | "format" => {
                meta.target = v.as_str().map(String::from).or_else(|| {
                    // Support `target: { html: default }` or [target]\n html = "default"
                    v.as_table()
                        .and_then(|t| t.first())
                        .map(|(k, _)| k.clone())
                });
            }
            "number-sections" => meta.number_sections = v.as_bool().unwrap_or(false),
            "toc" => meta.toc = Some(v.as_bool().unwrap_or(false)),
            "toc-depth" => meta.toc_depth = v.as_integer().unwrap_or(3) as u8,
            "toc-title" => meta.toc_title = v.as_str().map(String::from),
            "date-format" => meta.date_format = v.as_str().map(String::from),
            "bibliography" => {
                meta.bibliography = value_string_list(v);
            }
            "csl" => meta.csl = v.as_str().map(String::from),
            "html-math-method" => meta.html_math_method = v.as_str().map(String::from),
            "calepin" => {
                if let Some(cmap) = v.as_table() {
                    if let Some(pv) = table_get(cmap, "plugins") {
                        meta.plugins = value_string_list(pv);
                    }
                    if let Some(fd) = table_get(cmap, "files-dir") {
                        meta.files_dir = fd.as_str().map(String::from);
                    }
                    if let Some(cd) = table_get(cmap, "cache-dir") {
                        meta.cache_dir = cd.as_str().map(String::from);
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
    let mut simple_names: Vec<String> = Vec::new();

    for entry in entries {
        if let Some(s) = entry.as_str() {
            let name = parse_author_name_str(s);
            simple_names.push(name.literal.clone());
            authors.push(Author { name, ..Default::default() });
        } else if let Some(t) = entry.as_table() {
            let author = parse_author_mapping(t, &mut affiliations, top_level_affiliations);
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
        let mut parts = s.splitn(2, ',');
        let family = parts.next().unwrap_or("").trim().to_string();
        let given = parts.next().unwrap_or("").trim().to_string();
        let literal = if given.is_empty() {
            family.clone()
        } else {
            format!("{} {}", given, family)
        };
        AuthorName { literal }
    } else {
        AuthorName { literal: s.to_string() }
    }
}

/// Parse a mapping-form author entry into an `Author`.
fn parse_author_mapping(
    m: &Table,
    affiliations: &mut Vec<Affiliation>,
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
            author.name = AuthorName { literal };
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
    let role_key = table_get(m, "roles")
        .or_else(|| table_get(m, "role"));
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
    let aff_key = table_get(m, "affiliations")
        .or_else(|| table_get(m, "affiliation"));
    if let Some(aff_val) = aff_key {
        let aff_entries: Vec<&Value> = if aff_val.as_str().is_some() || aff_val.as_table().is_some() {
            vec![aff_val]
        } else if let Some(seq) = aff_val.as_array() {
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
fn resolve_affiliation(
    entry: &Value,
    affiliations: &mut Vec<Affiliation>,
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
                        return resolve_affiliation(tl, affiliations, &[]);
                    }
                }
            }
            if let Some(idx) = affiliations.iter().position(|a| a.id.as_deref() == Some(ref_val.as_str())) {
                return Some(idx);
            }
            return None;
        }
        // Inline affiliation
        let id = table_str(m, "id");
        let name = table_str(m, "name");
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
            department: table_str(m, "department"),
            city: table_str(m, "city"),
            region: table_str(m, "region").or_else(|| table_str(m, "state")),
            country: table_str(m, "country"),
            ..Default::default()
        };
        affiliations.push(aff);
        return Some(affiliations.len() - 1);
    }
    None
}

// ---------------------------------------------------------------------------
// Copyright, license, citation, funding parsing
// ---------------------------------------------------------------------------

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
        return if let Some((text, url)) = expand_cc_license(s) {
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
            if let Some((text, url)) = expand_cc_license(&t) {
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

fn parse_citation(v: &Value) -> Option<CitationMeta> {
    let m = v.as_table()?;
    Some(CitationMeta {
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
        let text = "---\ntitle: Test\nabstract: |\n  some content\n  ---\n  more content\n---\nBody";
        let (meta, body) = split_yaml(text).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Test"));
        assert!(meta.abstract_text.as_ref().unwrap().contains("more content"));
        assert_eq!(body, "Body");
    }

    #[test]
    fn test_format_mapping() {
        let text = "---\ntitle: Test\nformat:\n  html: default\n---\nBody";
        let (meta, _) = split_yaml(text).unwrap();
        assert_eq!(meta.target.as_deref(), Some("html"));
    }

    #[test]
    fn test_format_string() {
        let text = "---\nformat: latex\n---\nBody";
        let (meta, _) = split_yaml(text).unwrap();
        assert_eq!(meta.target.as_deref(), Some("latex"));
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

    #[test]
    fn test_toml_frontmatter() {
        let text = "---\ntitle = \"Hello\"\nauthor = \"World\"\nformat = \"html\"\n---\nBody";
        let (meta, body) = split_yaml(text).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Hello"));
        assert_eq!(meta.author, Some(vec!["World".to_string()]));
        assert_eq!(meta.target.as_deref(), Some("html"));
        assert_eq!(body, "Body");
    }

    #[test]
    fn test_toml_frontmatter_nested() {
        let text = "---\ntitle = \"Hello\"\n\n[calepin]\nplugins = [\"txtfmt\"]\n---\nBody";
        let (meta, _) = split_yaml(text).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Hello"));
        assert_eq!(meta.plugins, vec!["txtfmt"]);
    }
}
