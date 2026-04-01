//! Metadata formatting: author blocks, citations, appendix, funding, copyright.
//!
//! Extracted from `template.rs` to separate domain-specific metadata rendering
//! from the generic template engine machinery.

use std::sync::LazyLock;

use regex::Regex;

use crate::config::Metadata;
use crate::render::elements::resolve_element_template;
use crate::render::template::{apply_template, TemplateVars};

/// Format primitives driven by element templates.
/// Each method renders the appropriate markup via `templates/{engine}/` templates,
/// making output user-overridable.
struct Fmt;

impl Fmt {
    fn emphasis(text: &str, writer: &str) -> String {
        let mut vars = TemplateVars::new();
        vars.cfg.insert("text".to_string(), minijinja::Value::from(text.to_string()));
        render_fmt_template("emphasis", writer, &vars)
    }

    fn url(url: &str, label: Option<&str>, writer: &str) -> String {
        let label = label.unwrap_or(url);
        let mut vars = TemplateVars::new();
        vars.cfg.insert("url".to_string(), minijinja::Value::from(url.to_string()));
        vars.cfg.insert("label".to_string(), minijinja::Value::from(label.to_string()));
        render_fmt_template("url", writer, &vars)
    }
}

/// Render a format-primitive template. Falls back to empty string if not found.
fn render_fmt_template(name: &str, writer: &str, vars: &TemplateVars) -> String {
    if let Some(tpl) = resolve_element_template(name, writer) {
        apply_template(&tpl, vars)
    } else {
        String::new()
    }
}

/// Strip markdown image/link syntax to produce plain text for <title> etc.
/// `![alt](url)` -> `alt`, `[text](url)` -> `text`.
/// For images with no alt text, extracts the filename stem from the URL.
pub fn strip_markdown_formatting(text: &str) -> String {
    static RE_IMG: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"!\[([^\]]*)\]\(([^)]*)\)(\{[^}]*\})?").unwrap()
    });
    static RE_LINK: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\[([^\]]*)\]\([^)]*\)(\{[^}]*\})?").unwrap()
    });
    let result = RE_IMG.replace_all(text, |caps: &regex::Captures| {
        let alt = caps.get(1).map_or("", |m| m.as_str());
        if !alt.is_empty() {
            return alt.to_string();
        }
        let url = caps.get(2).map_or("", |m| m.as_str());
        std::path::Path::new(url)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string()
    });
    let result = RE_LINK.replace_all(&result, "$1");
    result.trim().to_string()
}

/// Resolve an element template and render it with the given extra vars,
/// automatically injecting `base` and `writer`. Returns `None` if the
/// template does not exist.
fn render_section(name: &str, writer: &str, extra_vars: Vec<(&str, minijinja::Value)>) -> Option<String> {
    let tpl = resolve_element_template(name, writer)?;
    let mut vars = TemplateVars::with_writer(writer);
    for (k, v) in extra_vars {
        vars.clp.insert(k.to_string(), v);
    }
    Some(apply_template(&tpl, &vars))
}

/// Build the appendix block for any format using element templates.
/// Format-specific content (links, lists) is computed in Rust;
/// structural markup comes from overridable templates.
pub fn build_appendix(meta: &Metadata, writer: &str) -> String {
    let style = meta.appendix_style.as_deref().unwrap_or("default");
    if style == "none" {
        return String::new();
    }

    let mut sections: Vec<String> = Vec::new();

    // License
    if let Some(ref lic) = meta.license {
        if let Some(ref text) = lic.text {
            if let Some(s) = render_section("license", writer, vec![
                ("content", minijinja::Value::from(text.clone())),
                ("url", minijinja::Value::from(lic.url.as_deref().unwrap_or("").to_string())),
            ]) {
                sections.push(s);
            }
        }
    }

    // Citation
    if let Some(ref cite) = meta.citation {
        if let Some(s) = render_section("citation", writer, vec![
            ("content", minijinja::Value::from(build_citation_text(meta, cite, writer))),
        ]) {
            sections.push(s);
        }
    }

    // Copyright
    if let Some(ref cr) = meta.copyright {
        let text = build_copyright_text(cr);
        if !text.is_empty() {
            if let Some(s) = render_section("copyright", writer, vec![
                ("content", minijinja::Value::from(text)),
            ]) {
                sections.push(s);
            }
        }
    }

    // Funding
    if !meta.funding.is_empty() {
        let items = build_funding_items(&meta.funding, writer);
        if !items.is_empty() {
            if let Some(s) = render_section("funding", writer, vec![
                ("items", minijinja::Value::from_iter(items)),
            ]) {
                sections.push(s);
            }
        }
    }

    if sections.is_empty() {
        String::new()
    } else if let Some(s) = render_section("appendix", writer, vec![
        ("content", minijinja::Value::from(sections.join("\n"))),
    ]) {
        s
    } else {
        sections.join("\n\n")
    }
}

/// Build the copyright text from a Copyright struct.
fn build_copyright_text(cr: &crate::config::Copyright) -> String {
    if let Some(ref stmt) = cr.statement {
        stmt.clone()
    } else {
        let holder = cr.holder.as_deref().unwrap_or("");
        let year = cr.year.as_deref().unwrap_or("");
        match (holder.is_empty(), year.is_empty()) {
            (false, false) => format!("Copyright {} {}", year, holder),
            (false, true) => format!("Copyright {}", holder),
            (true, false) => format!("Copyright {}", year),
            (true, true) => String::new(),
        }
    }
}

/// Build the citation text from metadata and citation info.
fn build_citation_text(meta: &Metadata, cite: &crate::config::CitationConfig, writer: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    {
        let names = meta.author_names();
        if !names.is_empty() {
            parts.push(names.join(", "));
        }
    }
    if let Some(ref t) = meta.title {
        parts.push(format!("\"{}\"", t));
    }
    if let Some(ref ct) = cite.container_title {
        parts.push(Fmt::emphasis(ct, writer));
    }
    if let Some(ref vol) = cite.volume {
        let vol_str = if let Some(ref iss) = cite.issue {
            format!("{}({})", vol, iss)
        } else {
            vol.clone()
        };
        parts.push(vol_str);
    }
    if let Some(ref pg) = cite.page {
        parts.push(format!("pp. {}", pg));
    }
    if let Some(ref issued) = cite.issued {
        parts.push(format!("({})", issued));
    }
    let mut citation = parts.join(". ");
    if !citation.is_empty() {
        citation.push('.');
    }
    if let Some(ref doi) = cite.doi {
        let doi_url = if doi.starts_with("http") { doi.clone() } else { format!("https://doi.org/{}", doi) };
        citation.push_str(&format!(" DOI: {}", Fmt::url(&doi_url, Some(doi), writer)));
    }
    if let Some(ref url) = cite.url {
        citation.push_str(&format!(" URL: {}", Fmt::url(url, None, writer)));
    }
    citation
}

/// Build funding items as a list of display strings.
fn build_funding_items(funding: &[crate::config::Funding], writer: &str) -> Vec<String> {
    funding.iter().filter_map(|f| {
        let text = if let Some(ref stmt) = f.statement {
            stmt.clone()
        } else {
            let mut parts = Vec::new();
            if let Some(ref src) = f.source { parts.push(src.as_str()); }
            if let Some(ref award) = f.award { parts.push(award.as_str()); }
            if let Some(ref recip) = f.recipient { parts.push(recip.as_str()); }
            if parts.is_empty() { return None; }
            parts.join(", ")
        };
        if writer == "latex" {
            Some(text.replace('#', "\\#").replace('%', "\\%").replace('&', "\\&").replace('_', "\\_"))
        } else {
            Some(text)
        }
    }).collect()
}

/// Build the author block for any format using element templates.
/// Passes structured data (authors, affiliations, corresponding) to the
/// `authors` template, which handles iteration and formatting via Jinja.
pub fn build_authors(meta: &Metadata, writer: &str) -> String {
    if meta.authors.is_empty() {
        return String::new();
    }

    let tpl = match resolve_element_template("authors", writer) {
        Some(t) => t,
        None => return meta.author_names().join(", "),
    };

    let mut vars = TemplateVars::with_writer(writer);

    // Build structured author list
    let multi_aff = meta.affiliations.len() > 1;
    let authors: Vec<minijinja::Value> = meta.authors.iter().map(|author| {
        let sups = if !author.affiliation_ids.is_empty() && multi_aff {
            author.affiliation_ids.iter()
                .filter_map(|&i| meta.affiliations.get(i).map(|a| a.number.to_string()))
                .collect::<Vec<_>>().join(",")
        } else {
            String::new()
        };
        let orcid_url = author.orcid.as_ref().map(|o| {
            if o.starts_with("http") { o.clone() } else { format!("https://orcid.org/{}", o) }
        }).unwrap_or_default();
        let email = author.email.clone().unwrap_or_default();
        let mailto = if email.is_empty() { String::new() } else { format!("mailto:{}", email) };

        minijinja::Value::from_serialize(&std::collections::BTreeMap::from([
            ("name", author.name.literal.as_str()),
            ("superscripts", &sups),
            ("corresponding", if author.corresponding { "true" } else { "" }),
            ("orcid_url", &orcid_url),
            ("email", &email),
            ("mailto", &mailto),
        ]))
    }).collect();
    vars.cfg.insert("authors".to_string(), minijinja::Value::from_iter(authors));

    // Build structured affiliation list
    let affiliations: Vec<minijinja::Value> = meta.affiliations.iter().filter_map(|aff| {
        let display = aff.display();
        if display.is_empty() { return None; }
        let number = if multi_aff { aff.number.to_string() } else { String::new() };
        Some(minijinja::Value::from_serialize(&std::collections::BTreeMap::from([
            ("number".to_string(), number),
            ("display".to_string(), display),
        ])))
    }).collect();
    vars.cfg.insert("affiliations".to_string(), minijinja::Value::from_iter(affiliations));

    apply_template(&tpl, &vars)
}
