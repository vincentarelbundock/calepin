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
use std::sync::{LazyLock, Mutex};

use crate::types::Element;
use crate::config::Metadata;

// ---------------------------------------------------------------------------
// Process-global CSL style cache
// ---------------------------------------------------------------------------
//
// CSL style deserialization (CBOR → citationberg AST) is expensive (~5ms for
// archive styles). In batch mode every file typically uses the same style,
// so we cache the parsed IndependentStyle keyed by the resolved style name.
// The cache persists for the process lifetime; entries are never evicted
// (bounded by the small number of distinct styles a single run can use).

static CSL_CACHE: LazyLock<Mutex<HashMap<String, IndependentStyle>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static LOCALES: LazyLock<Vec<hayagriva::citationberg::Locale>> =
    LazyLock::new(|| hayagriva::archive::locales());

/// Rendered forms of a single citation key.
#[derive(Clone)]
struct CitationForms {
    paren: String,
    prose: String,
    year: String,
}

/// Cached rendered citation data, keyed by (sorted citation keys, style name).
#[derive(Clone)]
struct CitationData {
    citations: HashMap<String, CitationForms>,
    bibliography_md: Option<String>,
}

static CITATION_CACHE: LazyLock<Mutex<HashMap<(Vec<String>, String), CitationData>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Cached parsed bibliography libraries, keyed by sorted bib file paths.
static BIB_CACHE: LazyLock<Mutex<HashMap<Vec<String>, hayagriva::Library>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn format_plain(elem: &impl std::fmt::Display) -> String {
    format!("{:#}", elem)
}

#[inline(never)]
pub fn process_citations(elements: &mut Vec<Element>, metadata: &Metadata, project_root: &Path) -> Result<()> {
    if metadata.bibliography.is_empty() {
        return Ok(());
    }

    // Quick scan: if no text element contains '@', there can be no citations,
    // so skip all bib file parsing and CSL loading.
    let has_at = elements.iter().any(|el| {
        matches!(el, Element::Text { content } if content.contains('@'))
    });
    if !has_at {
        return Ok(());
    }

    let bib_paths: Vec<String> = metadata.bibliography.iter()
        .map(|p| project_root.join(p).to_string_lossy().to_string())
        .collect();

    let library = load_bib_library(&bib_paths)?;

    // Collect cited keys before loading the CSL style. CSL deserialization is
    // expensive (~5ms), so we defer it until we know there are actual citations.
    static RE_ANY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?:\[-)?@([a-zA-Z0-9_][-a-zA-Z0-9_]*)\]?").unwrap()
    });
    // Cross-reference prefixes that should not be looked up as citation keys
    let crossref_prefixes: Vec<String> = crate::registry::all_crossref_prefixes()
        .iter()
        .map(|(p, _)| format!("{}-", p))
        .collect();

    let re_any = &*RE_ANY;
    let mut all_keys: Vec<String> = Vec::new();
    let mut seen_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
    for el in elements.iter() {
        if let Element::Text { content } = el {
            for caps in re_any.captures_iter(content) {
                let key = caps[1].to_string();
                if !crossref_prefixes.iter().any(|p| key.starts_with(p.as_str()))
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

    let style_name = metadata.csl.as_deref().unwrap_or("__default__").to_string();
    let cache_key = (all_keys.clone(), style_name.clone());

    // Check citation cache
    let cite_data = if let Ok(cache) = CITATION_CACHE.lock() {
        if let Some(cached) = cache.get(&cache_key) {
            cached.clone()
        } else {
            drop(cache);
            render_citations(&all_keys, &library, metadata, &style_name)?
        }
    } else {
        render_citations(&all_keys, &library, metadata, &style_name)?
    };

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
                    cite_data.citations.get(&caps[1])
                        .map(|c| c.year.clone())
                        .unwrap_or_else(|| caps[0].to_string())
                })
                .to_string();

            *content = re_bracket
                .replace_all(content, |caps: &regex::Captures| {
                    match cite_data.citations.get(&caps[1]) {
                        Some(c) => format!("({})", c.paren),
                        None => caps[0].to_string(),
                    }
                })
                .to_string();

            *content = re_bare
                .replace_all(content, |caps: &regex::Captures| {
                    cite_data.citations.get(&caps[1])
                        .map(|c| c.prose.clone())
                        .unwrap_or_else(|| caps[0].to_string())
                })
                .to_string();
        }
    }

    // Append bibliography
    if let Some(bib_md) = cite_data.bibliography_md {
        elements.push(Element::Text { content: bib_md });
    }

    Ok(())
}

fn load_bib_library(bib_paths: &[String]) -> Result<hayagriva::Library> {
    // Check cache
    let mut sorted = bib_paths.to_vec();
    sorted.sort();
    if let Ok(cache) = BIB_CACHE.lock() {
        if let Some(lib) = cache.get(&sorted) {
            return Ok(lib.clone());
        }
    }

    let mut library = hayagriva::Library::new();
    for path in bib_paths {
        let resolved = Path::new(path);
        if !resolved.exists() {
            cwarn!("bibliography '{}' not found, skipping", resolved.display());
            continue;
        }
        let bib_src = fs::read_to_string(resolved)
            .with_context(|| format!("Failed to read bibliography: {}", resolved.display()))?;
        let lib = hayagriva::io::from_biblatex_str(&bib_src)
            .map_err(|e| anyhow::anyhow!("Failed to parse bibliography '{}': {:?}", path, e))?;
        for entry in lib.iter() {
            library.push(entry);
        }
    }

    if let Ok(mut cache) = BIB_CACHE.lock() {
        cache.insert(sorted, library.clone());
    }

    Ok(library)
}

fn render_citations(
    all_keys: &[String],
    library: &hayagriva::Library,
    metadata: &Metadata,
    style_name: &str,
) -> Result<CitationData> {
    let style = load_csl_style(metadata.csl.as_deref(), metadata)?;
    let locales = &*LOCALES;

    let mut driver = BibliographyDriver::new();
    for key in all_keys {
        if let Some(entry) = library.get(key) {
            driver.citation(CitationRequest::from_items(
                vec![CitationItem::with_entry(entry)],
                &style,
                locales,
            ));
        }
    }
    let rendered = driver.finish(BibliographyRequest {
        style: &style,
        locale: None,
        locale_files: locales,
    });

    let mut citations: HashMap<String, CitationForms> = HashMap::new();
    for (i, key) in all_keys.iter().enumerate() {
        if let Some(c) = rendered.citations.get(i) {
            let paren = format_plain(&c.citation);
            citations.insert(key.clone(), CitationForms {
                prose: format_narrative(&paren),
                year: extract_year(&paren),
                paren,
            });
        }
    }

    let bibliography_md = rendered.bibliography.as_ref().map(|bib| {
        let mut md = String::from("\n# References\n\n");
        for entry in &bib.items {
            let text = format_plain(&entry.content);
            if !text.is_empty() {
                md.push_str(&text);
                md.push_str("\n\n");
            }
        }
        md
    });

    let result = CitationData { citations, bibliography_md };

    // Store in cache
    let cache_key = (all_keys.to_vec(), style_name.to_string());
    if let Ok(mut cache) = CITATION_CACHE.lock() {
        cache.insert(cache_key, result.clone());
    }

    Ok(result)
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

fn load_csl_style(csl_path: Option<&str>, meta: &crate::config::Metadata) -> Result<IndependentStyle> {
    // Build the cache key from the CSL path (or "default" for fallback)
    let cache_key = csl_path.unwrap_or("__default__").to_string();

    // Check cache first
    if let Ok(cache) = CSL_CACHE.lock() {
        if let Some(style) = cache.get(&cache_key) {
            return Ok(style.clone());
        }
    }

    let style = load_csl_style_uncached(csl_path, meta)?;

    // Store in cache
    if let Ok(mut cache) = CSL_CACHE.lock() {
        cache.insert(cache_key, style.clone());
    }

    Ok(style)
}

fn load_csl_style_uncached(csl_path: Option<&str>, meta: &crate::config::Metadata) -> Result<IndependentStyle> {
    use hayagriva::archive::ArchivedStyle;

    // Build candidate list: explicit name first, then default
    let default_name = meta.csl.as_deref()
        .or_else(|| crate::config::builtin_metadata().csl.as_deref())
        .unwrap_or("chicago-author-date");
    let candidates: Vec<&str> = match csl_path {
        Some(name) => vec![name, default_name],
        None => vec![default_name],
    };

    for name in &candidates {
        // Try as file path
        let path = Path::new(name);
        if path.exists() {
            let xml = fs::read_to_string(path)
                .with_context(|| format!("Failed to read CSL file: {}", name))?;
            if let Ok(style) = IndependentStyle::from_xml(&xml) {
                return Ok(style);
            }
        }
        // Try as archive name
        if let Some(archived) = ArchivedStyle::by_name(name) {
            if let citationberg::Style::Independent(style) = archived.get() {
                return Ok(style);
            }
        }
    }

    // Only warn if an explicit CSL was given but not found
    if let Some(name) = csl_path {
        cwarn!("CSL '{}' not found or not usable, no fallback available", name);
    }
    anyhow::bail!("No usable CSL style found")
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
