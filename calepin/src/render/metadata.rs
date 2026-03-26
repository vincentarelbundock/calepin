//! Metadata formatting: author blocks, citations, appendix, funding, copyright.
//!
//! Extracted from `template.rs` to separate domain-specific metadata rendering
//! from the generic template engine machinery.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::config::Metadata;
use crate::render::elements::resolve_element_partial;
use crate::render::template::apply_template;

/// Format primitives driven by element templates.
/// Each method renders the appropriate markup via `partials/{engine}/` templates,
/// making output user-overridable.
struct Fmt;

impl Fmt {
    fn superscript(text: &str, ext: &str) -> String {
        let mut vars = HashMap::new();
        vars.insert("text".to_string(), text.to_string());
        render_fmt_template("superscript", ext, &vars)
    }

    fn emphasis(text: &str, ext: &str) -> String {
        let mut vars = HashMap::new();
        vars.insert("text".to_string(), text.to_string());
        render_fmt_template("emphasis", ext, &vars)
    }

    fn url(url: &str, label: Option<&str>, ext: &str) -> String {
        let label = label.unwrap_or(url);
        let mut vars = HashMap::new();
        vars.insert("url".to_string(), url.to_string());
        vars.insert("label".to_string(), label.to_string());
        render_fmt_template("url", ext, &vars)
    }
}

/// Render a format-primitive template. Falls back to empty string if not found.
fn render_fmt_template(name: &str, ext: &str, vars: &HashMap<String, String>) -> String {
    if let Some(tpl) = resolve_element_partial(name, ext) {
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

/// Build the appendix block for any format using element templates.
/// Format-specific content (links, lists) is computed in Rust;
/// structural markup comes from overridable templates.
pub fn build_appendix(meta: &Metadata, ext: &str) -> String {
    let style = meta.appendix_style.as_deref().unwrap_or("default");
    if style == "none" {
        return String::new();
    }

    let mut sections: Vec<String> = Vec::new();
    let fmt = ext.to_string();
    let label_defs = meta.labels.clone();

    // License
    if let Some(ref lic) = meta.license {
        if let Some(ref text) = lic.text {
            if let Some(tpl) = resolve_element_partial("license", ext) {
                let mut vars = HashMap::new();
                vars.insert("base".to_string(), fmt.clone());
                vars.insert("writer".to_string(), fmt.clone());
                vars.insert("text".to_string(), text.clone());
                vars.insert("url".to_string(), lic.url.as_deref().unwrap_or("").to_string());
                vars.insert("label_reuse".to_string(), label_defs.as_ref().and_then(|l| l.reuse.clone()).unwrap_or_else(|| "Reuse".to_string()));
                sections.push(apply_template(&tpl, &vars));
            }
        }
    }

    // Citation
    if let Some(ref cite) = meta.citation {
        let content = build_citation_text(meta, cite, ext);
        if let Some(tpl) = resolve_element_partial("citation", ext) {
            let mut vars = HashMap::new();
            vars.insert("base".to_string(), fmt.clone());
            vars.insert("writer".to_string(), fmt.clone());
            vars.insert("content".to_string(), content);
            vars.insert("label_citation".to_string(), label_defs.as_ref().and_then(|l| l.citation.clone()).unwrap_or_else(|| "Citation".to_string()));
            sections.push(apply_template(&tpl, &vars));
        }
    }

    // Copyright
    if let Some(ref cr) = meta.copyright {
        let text = build_copyright_text(cr);
        if !text.is_empty() {
            if let Some(tpl) = resolve_element_partial("copyright", ext) {
                let mut vars = HashMap::new();
                vars.insert("base".to_string(), fmt.clone());
                vars.insert("writer".to_string(), fmt.clone());
                vars.insert("content".to_string(), text);
                vars.insert("label_copyright".to_string(), label_defs.as_ref().and_then(|l| l.copyright.clone()).unwrap_or_else(|| "Copyright".to_string()));
                sections.push(apply_template(&tpl, &vars));
            }
        }
    }

    // Funding
    if !meta.funding.is_empty() {
        let items = build_funding_items(&meta.funding, ext);
        if !items.is_empty() {
            if let Some(tpl) = resolve_element_partial("funding", ext) {
                let mut vars = HashMap::new();
                vars.insert("base".to_string(), fmt.clone());
                vars.insert("writer".to_string(), fmt.clone());
                vars.insert("items".to_string(), items);
                vars.insert("label_funding".to_string(), label_defs.as_ref().and_then(|l| l.funding.clone()).unwrap_or_else(|| "Funding".to_string()));
                sections.push(apply_template(&tpl, &vars));
            }
        }
    }

    if sections.is_empty() {
        String::new()
    } else if let Some(tpl) = resolve_element_partial("appendix", ext) {
        let mut vars = HashMap::new();
        vars.insert("base".to_string(), fmt.clone());
        vars.insert("writer".to_string(), fmt);
        vars.insert("sections".to_string(), sections.join("\n"));
        vars.insert("label_appendix".to_string(), label_defs.as_ref().and_then(|l| l.appendix.clone()).unwrap_or_else(|| "Appendix".to_string()));
        apply_template(&tpl, &vars)
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
fn build_citation_text(meta: &Metadata, cite: &crate::config::CitationConfig, ext: &str) -> String {
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
        parts.push(Fmt::emphasis(ct, ext));
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
        citation.push_str(&format!(" DOI: {}", Fmt::url(&doi_url, Some(doi), ext)));
    }
    if let Some(ref url) = cite.url {
        citation.push_str(&format!(" URL: {}", Fmt::url(url, None, ext)));
    }
    citation
}

/// Build funding items as a pre-formatted list string.
fn build_funding_items(funding: &[crate::config::Funding], ext: &str) -> String {
    let items: Vec<String> = funding.iter().filter_map(|f| {
        if let Some(ref stmt) = f.statement {
            Some(stmt.clone())
        } else {
            let mut parts = Vec::new();
            if let Some(ref src) = f.source { parts.push(src.as_str()); }
            if let Some(ref award) = f.award { parts.push(award.as_str()); }
            if let Some(ref recip) = f.recipient { parts.push(recip.as_str()); }
            if parts.is_empty() { None } else { Some(parts.join(", ")) }
        }
    }).collect();
    if items.is_empty() {
        return String::new();
    }
    items.iter().map(|i| {
        let text = if ext == "latex" {
            i.replace('#', "\\#").replace('%', "\\%").replace('&', "\\&").replace('_', "\\_")
        } else {
            i.clone()
        };
        let mut vars = HashMap::new();
        vars.insert("text".to_string(), text);
        render_fmt_template("funding_item", ext, &vars)
    }).collect::<Vec<_>>().join("\n")
}

/// Build the author block for any format using element templates.
/// Rich metadata (affiliations, ORCID, etc.) renders through author-item and
/// affiliation-item sub-templates; simple author lists use a plain fallback.
pub fn build_authors(meta: &Metadata, ext: &str) -> String {
    let has_rich = !meta.authors.is_empty()
        && meta.authors.iter().any(|a| {
            !a.affiliation_ids.is_empty()
                || a.email.is_some()
                || a.orcid.is_some()
                || a.corresponding
        });

    if has_rich {
        // Render each author through the author-item template
        let author_tpl = resolve_element_partial("author_item", ext);
        let authors_rendered: Vec<String> = meta.authors.iter().map(|author| {
            let superscripts = if !author.affiliation_ids.is_empty() && meta.affiliations.len() > 1 {
                let sups: Vec<String> = author.affiliation_ids.iter()
                    .filter_map(|&i| meta.affiliations.get(i).map(|a| a.number.to_string()))
                    .collect();
                Fmt::superscript(&sups.join(","), ext)
            } else {
                String::new()
            };

            let corresponding = if author.corresponding {
                Fmt::superscript("*", ext)
            } else {
                String::new()
            };

            let orcid_url = if let Some(ref orcid) = author.orcid {
                if orcid.starts_with("http") { orcid.clone() } else { format!("https://orcid.org/{}", orcid) }
            } else {
                String::new()
            };

            if let Some(ref tpl) = author_tpl {
                let mut vars = HashMap::new();
                vars.insert("base".to_string(), ext.to_string());
                vars.insert("writer".to_string(), ext.to_string());
                vars.insert("name".to_string(), author.name.literal.clone());
                vars.insert("superscripts".to_string(), superscripts);
                vars.insert("corresponding".to_string(), corresponding);
                vars.insert("orcid_url".to_string(), orcid_url);
                apply_template(tpl, &vars)
            } else {
                format!("{}{}{}", author.name.literal, superscripts, corresponding)
            }
        }).collect();

        // Render each affiliation through the affiliation-item template
        let aff_tpl = resolve_element_partial("affiliation_item", ext);
        let affs_rendered: Vec<String> = meta.affiliations.iter().filter_map(|aff| {
            let display = aff.display();
            if display.is_empty() {
                return None;
            }
            let number = if meta.affiliations.len() > 1 {
                format!("{} ", Fmt::superscript(&aff.number.to_string(), ext))
            } else {
                String::new()
            };
            if let Some(ref tpl) = aff_tpl {
                let mut vars = HashMap::new();
                vars.insert("base".to_string(), ext.to_string());
                vars.insert("writer".to_string(), ext.to_string());
                vars.insert("number".to_string(), number);
                vars.insert("display".to_string(), display);
                Some(apply_template(tpl, &vars))
            } else {
                Some(format!("{}{}", number, display))
            }
        }).collect();

        // Corresponding author note
        let corresponding_note = meta.authors.iter().filter_map(|author| {
            if author.corresponding {
                if let Some(ref email) = author.email {
                    let mailto = format!("mailto:{}", email);
                    let mut vars = HashMap::new();
                    vars.insert("email".to_string(), email.clone());
                    vars.insert("mailto".to_string(), mailto);
                    Some(render_fmt_template("corresponding", ext, &vars))
                } else {
                    None
                }
            } else {
                None
            }
        }).collect::<Vec<_>>().join("\n");

        // Join authors and affiliations with format-appropriate separators
        let authors_joined = match ext {
            "latex" => authors_rendered.join(" \\and "),
            "typst" => authors_rendered.join(", "),
            _ => authors_rendered.join("\n"),
        };

        let affiliations_items = match ext {
            "latex" => affs_rendered.join(" \\\\\n"),
            "typst" => affs_rendered.join(" \\\n"),
            "html" => affs_rendered.join("\n"),
            _ => affs_rendered.join(", "),
        };

        if let Some(tpl) = resolve_element_partial("authors", ext) {
            let mut vars = HashMap::new();
            vars.insert("base".to_string(), ext.to_string());
            vars.insert("writer".to_string(), ext.to_string());
            vars.insert("authors_cmd".to_string(), format!("\\author{{{}}}", authors_joined));
            vars.insert("authors".to_string(), authors_joined);
            vars.insert("affiliations_items".to_string(), affiliations_items);
            vars.insert("corresponding_note".to_string(), corresponding_note);
            apply_template(&tpl, &vars)
        } else {
            format!("{}\n{}\n{}", authors_joined, affiliations_items, corresponding_note)
        }
    } else {
        let names = meta.author_names();
        if !names.is_empty() {
            let mut vars = HashMap::new();
            vars.insert("names".to_string(), names.join(", "));
            render_fmt_template("authors_simple", ext, &vars)
        } else {
            String::new()
        }
    }
}
