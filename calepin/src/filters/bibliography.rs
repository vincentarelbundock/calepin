// Bibliography processing using hayagriva.
//!
//! Citation syntax:
//!   @key        → narrative: "Author et al. (Year)"
//!   [@key]      → parenthetical: "(Author et al. Year)"
//!   [-@key]     → year only: "Year"
//!
//! Uses a single BibliographyDriver call. Narrative and year-only forms
//! are derived from the parenthetical citation by string manipulation.

use anyhow::{Context, Result};
use hayagriva::citationberg::{self, IndependentStyle};
use hayagriva::{BibliographyDriver, BibliographyRequest, CitationItem, CitationRequest};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use crate::types::Element;
use crate::types::Metadata;

fn format_plain(elem: &impl std::fmt::Display) -> String {
    format!("{:#}", elem)
}

#[inline(never)]
pub fn process_citations(elements: &mut Vec<Element>, metadata: &Metadata) -> Result<()> {
    if metadata.bibliography.is_empty() {
        return Ok(());
    }

    let mut library = hayagriva::Library::new();
    for bib_path in &metadata.bibliography {
        if !Path::new(bib_path).exists() {
            cwarn!("bibliography '{}' not found, skipping", bib_path);
            continue;
        }
        let bib_src = fs::read_to_string(bib_path)
            .with_context(|| format!("Failed to read bibliography: {}", bib_path))?;
        let lib = hayagriva::io::from_biblatex_str(&bib_src)
            .map_err(|e| anyhow::anyhow!("Failed to parse bibliography '{}': {:?}", bib_path, e))?;
        for entry in lib.iter() {
            library.push(entry);
        }
    }
    let style = load_csl_style(metadata.csl.as_deref())?;
    let locales: Vec<citationberg::Locale> = Vec::new();

    // Collect cited keys
    static RE_ANY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?:\[-)?@([a-zA-Z0-9_][-a-zA-Z0-9_:]*)\]?").unwrap()
    });
    // Cross-reference prefixes that should not be looked up as citation keys
    static CROSSREF_PREFIXES: &[&str] = &[
        "fig-", "sec-", "tbl-", "eq-", "thm-", "lem-", "cor-", "prp-",
        "cnj-", "def-", "exm-", "exr-", "sol-", "rem-", "alg-",
    ];

    let re_any = &*RE_ANY;
    let mut all_keys: Vec<String> = Vec::new();
    let mut seen_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
    for el in elements.iter() {
        if let Element::Text { content } = el {
            for caps in re_any.captures_iter(content) {
                let key = caps[1].to_string();
                if !key.contains(':')
                    && !CROSSREF_PREFIXES.iter().any(|p| key.starts_with(p))
                    && seen_keys.insert(key.clone())
                    && library.get(&key).is_some()
                {
                    all_keys.push(key);
                }
            }
        }
    }
    if all_keys.is_empty() {
        return Ok(());
    }

    // Single driver call with all citations
    let mut driver = BibliographyDriver::new();
    for key in &all_keys {
        if let Some(entry) = library.get(key) {
            driver.citation(CitationRequest::from_items(
                vec![CitationItem::with_entry(entry)],
                &style,
                &locales,
            ));
        }
    }
    let rendered = driver.finish(BibliographyRequest {
        style: &style,
        locale: None,
        locale_files: &locales,
    });

    // Build maps: parenthetical form, then derive narrative and year-only
    let mut paren_map: HashMap<String, String> = HashMap::new();
    let mut prose_map: HashMap<String, String> = HashMap::new();
    let mut year_map: HashMap<String, String> = HashMap::new();

    for (i, key) in all_keys.iter().enumerate() {
        if let Some(c) = rendered.citations.get(i) {
            let mut paren = format_plain(&c.citation); // e.g. "Arel-Bundock et al. 2026"

            // hayagriva 0.9 may not apply et-al truncation correctly.
            // If the entry has 4+ authors and the citation doesn't contain
            // "et al.", manually truncate to first author + "et al."
            if let Some(entry) = library.get(key) {
                if let Some(authors) = entry.authors() {
                    if authors.len() >= 4 && !paren.contains("et al.") {
                        if let Some(first) = authors.first() {
                            let surname = &first.name;
                            let year = extract_year(&paren);
                            paren = format!("{} et al. {}", surname, year);
                        }
                    }
                }
            }

            paren_map.insert(key.clone(), paren.clone());
            prose_map.insert(key.clone(), format_narrative(&paren));
            year_map.insert(key.clone(), extract_year(&paren));
        }
    }

    // Replace citations in Text elements (order: [-@key], [@key], @key)
    static RE_SUPPRESS: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\[-@([a-zA-Z0-9_][-a-zA-Z0-9_:.]*)\]").unwrap()
    });
    static RE_BRACKET: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\[@([a-zA-Z0-9_][-a-zA-Z0-9_:.]*)\]").unwrap()
    });
    static RE_BARE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"@([a-zA-Z0-9_][-a-zA-Z0-9_:]*)").unwrap()
    });
    let re_suppress = &*RE_SUPPRESS;
    let re_bracket = &*RE_BRACKET;
    let re_bare = &*RE_BARE;

    for el in elements.iter_mut() {
        if let Element::Text { content } = el {
            if !content.contains('@') {
                continue;
            }

            *content = re_suppress
                .replace_all(content, |caps: &regex::Captures| {
                    year_map.get(&caps[1]).cloned().unwrap_or_else(|| caps[0].to_string())
                })
                .to_string();

            *content = re_bracket
                .replace_all(content, |caps: &regex::Captures| {
                    match paren_map.get(&caps[1]) {
                        Some(cite) => format!("({})", cite),
                        None => caps[0].to_string(),
                    }
                })
                .to_string();

            *content = re_bare
                .replace_all(content, |caps: &regex::Captures| {
                    prose_map.get(&caps[1]).cloned().unwrap_or_else(|| caps[0].to_string())
                })
                .to_string();
        }
    }

    // Append bibliography
    if let Some(bib) = &rendered.bibliography {
        let mut bib_md = String::from("\n# References\n\n");
        for entry in &bib.items {
            let text = format_plain(&entry.content);
            if !text.is_empty() {
                bib_md.push_str(&text);
                bib_md.push_str("\n\n");
            }
        }
        elements.push(Element::Text { content: bib_md });
    }

    Ok(())
}

/// Regex matching a 4-digit year with optional letter suffix.
static RE_YEAR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(\d{4}[a-z]?)\b").unwrap()
});

/// Convert parenthetical "Author et al. 2026" to narrative "Author et al. (2026)".
fn format_narrative(paren: &str) -> String {
    if let Some(m) = RE_YEAR.find(paren) {
        let before = paren[..m.start()].trim_end();
        let year = m.as_str();
        let after = &paren[m.end()..];
        format!("{} ({}){}", before, year, after)
    } else {
        paren.to_string()
    }
}

/// Extract just the year from a citation string.
fn extract_year(cite: &str) -> String {
    RE_YEAR.find(cite)
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| cite.to_string())
}

fn load_csl_style(csl_path: Option<&str>) -> Result<IndependentStyle> {
    // 1. Explicit CSL path from front matter
    if let Some(path) = csl_path {
        if Path::new(path).exists() {
            let xml = fs::read_to_string(path)
                .with_context(|| format!("Failed to read CSL file: {}", path))?;
            match IndependentStyle::from_xml(&xml) {
                Ok(style) => return Ok(style),
                Err(e) => {
                    cwarn!("CSL '{}' not usable ({:?}), using default", path, e);
                }
            }
        } else {
            cwarn!("CSL file '{}' not found, using default", path);
        }
    }

    // 2. Project/user: first .csl file (alphabetically) in _calepin/templates/
    if let Some(path) = crate::util::resolve_first_match("templates", "csl") {
        if let Ok(xml) = fs::read_to_string(&path) {
            if let Ok(style) = IndependentStyle::from_xml(&xml) {
                return Ok(style);
            }
        }
    }

    // 3. Built-in
    let default_csl = include_str!("../templates/misc/default.csl");
    IndependentStyle::from_xml(default_csl)
        .map_err(|e| anyhow::anyhow!("Failed to parse default CSL: {:?}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_narrative_modern_year() {
        assert_eq!(format_narrative("Knuth 1997"), "Knuth (1997)");
    }

    #[test]
    fn test_format_narrative_old_year() {
        assert_eq!(format_narrative("Euler 1748"), "Euler (1748)");
        assert_eq!(format_narrative("Gauss 1801"), "Gauss (1801)");
    }

    #[test]
    fn test_format_narrative_year_suffix() {
        assert_eq!(format_narrative("Smith 2024a"), "Smith (2024a)");
    }

    #[test]
    fn test_format_narrative_no_year() {
        assert_eq!(format_narrative("No year here"), "No year here");
    }

    #[test]
    fn test_extract_year_modern() {
        assert_eq!(extract_year("Knuth 1997"), "1997");
    }

    #[test]
    fn test_extract_year_old() {
        assert_eq!(extract_year("Euler 1748"), "1748");
    }

    #[test]
    fn test_extract_year_suffix() {
        assert_eq!(extract_year("Smith 2024a"), "2024a");
    }

    #[test]
    fn test_extract_year_missing() {
        assert_eq!(extract_year("no year"), "no year");
    }

    #[test]
    fn test_bare_citation_regex_no_trailing_period() {
        // @key at end of sentence: period must NOT be part of the key
        static RE_BARE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"@([a-zA-Z0-9_][-a-zA-Z0-9_:]*)").unwrap()
        });
        let text = "see @gauss1801disquisitiones.";
        let caps = RE_BARE.captures(text).unwrap();
        assert_eq!(&caps[1], "gauss1801disquisitiones");
    }

    #[test]
    fn test_bare_citation_regex_comma() {
        static RE_BARE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"@([a-zA-Z0-9_][-a-zA-Z0-9_:]*)").unwrap()
        });
        let text = "@knuth1997art, @euler1748introductio";
        let keys: Vec<&str> = RE_BARE.captures_iter(text)
            .map(|c| c.get(1).unwrap().as_str())
            .collect();
        assert_eq!(keys, vec!["knuth1997art", "euler1748introductio"]);
    }
}
