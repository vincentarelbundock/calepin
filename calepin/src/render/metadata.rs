//! Metadata formatting: author blocks, citations, appendix, funding, copyright.
//!
//! Extracted from `template.rs` to separate domain-specific metadata rendering
//! from the generic template engine machinery.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::types::Metadata;
use crate::render::elements::resolve_element_template;
use crate::render::template::apply_template;

/// Format primitives that dispatch on output format.
/// Each method produces the appropriate markup for html/latex/typst/markdown.
struct Fmt;

impl Fmt {
    fn link(text: &str, url: &str, ext: &str) -> String {
        match ext {
            "html" => format!("<a href=\"{}\">{}</a>", url, text),
            "latex" => format!("{} (\\url{{{}}})", text, url),
            "typst" => format!("#link(\"{}\")[{}]", url, text),
            _ => format!("[{}]({})", text, url),
        }
    }

    fn superscript(text: &str, ext: &str) -> String {
        match ext {
            "html" => format!("<sup>{}</sup>", text),
            "latex" => format!("\\textsuperscript{{{}}}", text),
            "typst" => format!("#super[{}]", text),
            _ => String::new(),
        }
    }

    fn emphasis(text: &str, ext: &str) -> String {
        match ext {
            "html" => format!("<em>{}</em>", text),
            "latex" => format!("\\emph{{{}}}", text),
            "typst" => format!("#emph[{}]", text),
            _ => format!("*{}*", text),
        }
    }

    fn url(url: &str, label: Option<&str>, ext: &str) -> String {
        let label = label.unwrap_or(url);
        match ext {
            "html" => format!("<a href=\"{}\">{}</a>", url, label),
            "latex" => format!("\\url{{{}}}", url),
            "typst" => format!("#link(\"{}\")[{}]", url, label),
            _ => format!("[{}]({})", label, url),
        }
    }
}

/// Format a text+url as a link in the appropriate output format.
pub fn format_link(text: &str, url: Option<&str>, ext: &str) -> String {
    match url {
        Some(url) => Fmt::link(text, url, ext),
        None => text.to_string(),
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

    // License
    if let Some(ref lic) = meta.license {
        if let Some(ref text) = lic.text {
            if let Some(tpl) = resolve_element_template("license", ext) {
                let mut vars = HashMap::new();
                vars.insert("base".to_string(), fmt.clone());
                vars.insert("text".to_string(), text.clone());
                vars.insert("url".to_string(), lic.url.as_deref().unwrap_or("").to_string());
                // Keep content for backward compatibility with overridden templates
                vars.insert("content".to_string(), format_link(text, lic.url.as_deref(), ext));
                sections.push(apply_template(&tpl, &vars));
            }
        }
    }

    // Citation
    if let Some(ref cite) = meta.citation {
        let content = build_citation_text(meta, cite, ext);
        if let Some(tpl) = resolve_element_template("citation", ext) {
            let mut vars = HashMap::new();
            vars.insert("base".to_string(), fmt.clone());
            vars.insert("content".to_string(), content);
            sections.push(apply_template(&tpl, &vars));
        }
    }

    // Copyright
    if let Some(ref cr) = meta.copyright {
        let text = build_copyright_text(cr);
        if !text.is_empty() {
            if let Some(tpl) = resolve_element_template("copyright", ext) {
                let mut vars = HashMap::new();
                vars.insert("base".to_string(), fmt.clone());
                vars.insert("content".to_string(), text);
                sections.push(apply_template(&tpl, &vars));
            }
        }
    }

    // Funding
    if !meta.funding.is_empty() {
        let items = build_funding_items(&meta.funding, ext);
        if !items.is_empty() {
            if let Some(tpl) = resolve_element_template("funding", ext) {
                let mut vars = HashMap::new();
                vars.insert("base".to_string(), fmt.clone());
                vars.insert("items".to_string(), items);
                sections.push(apply_template(&tpl, &vars));
            }
        }
    }

    if sections.is_empty() {
        String::new()
    } else if let Some(tpl) = resolve_element_template("appendix", ext) {
        let mut vars = HashMap::new();
        vars.insert("base".to_string(), fmt);
        vars.insert("sections".to_string(), sections.join("\n"));
        apply_template(&tpl, &vars)
    } else {
        sections.join("\n\n")
    }
}

/// Build the copyright text from a Copyright struct.
fn build_copyright_text(cr: &crate::types::Copyright) -> String {
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
fn build_citation_text(meta: &Metadata, cite: &crate::types::CitationMeta, ext: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(ref a) = meta.author {
        parts.push(a.join(", "));
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
fn build_funding_items(funding: &[crate::types::Funding], ext: &str) -> String {
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
    match ext {
        "html" => items.iter().map(|i| format!("<li>{}</li>", i)).collect::<Vec<_>>().join("\n"),
        "latex" => items.iter().map(|i| {
            let escaped = i.replace('#', "\\#").replace('%', "\\%").replace('&', "\\&").replace('_', "\\_");
            format!("\\item {}", escaped)
        }).collect::<Vec<_>>().join("\n"),
        _ => items.iter().map(|i| format!("- {}", i)).collect::<Vec<_>>().join("\n"),
    }
}

/// Build the author block for any format using element templates.
/// Rich metadata (affiliations, ORCID, etc.) renders through author-item and
/// affiliation-item sub-templates; simple author lists use a plain fallback.
pub fn build_author_block(meta: &Metadata, ext: &str) -> String {
    let has_rich = !meta.authors.is_empty()
        && meta.authors.iter().any(|a| {
            !a.affiliation_ids.is_empty()
                || a.email.is_some()
                || a.orcid.is_some()
                || a.corresponding
        });

    if has_rich {
        // Render each author through the author-item template
        let author_tpl = resolve_element_template("author_item", ext);
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
        let aff_tpl = resolve_element_template("affiliation_item", ext);
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
                    Some(match ext {
                        "html" => format!(
                            "  <p class=\"corresponding\">* Corresponding author: <a href=\"{}\">{}</a></p>",
                            mailto, email
                        ),
                        "latex" => format!("\\textsuperscript{{*}} Corresponding author: \\url{{{}}}", email),
                        "typst" => format!("#super[\\*] Corresponding author: #link(\"{}\")", mailto),
                        _ => format!("* Corresponding author: {}", email),
                    })
                } else {
                    None
                }
            } else {
                None
            }
        }).collect::<Vec<_>>().join("\n");

        // Assemble through author-block template
        let authors_joined = match ext {
            "latex" => authors_rendered.join(" \\and "),
            "typst" => authors_rendered.join(", "),
            _ => authors_rendered.join("\n"),
        };

        let affiliations_joined = match ext {
            "html" => {
                if affs_rendered.is_empty() {
                    String::new()
                } else {
                    format!("  <div class=\"affiliations\">\n{}\n  </div>", affs_rendered.join("\n"))
                }
            }
            "latex" => {
                if affs_rendered.is_empty() {
                    String::new()
                } else {
                    format!(
                        "\\newcommand{{\\affiliationblock}}{{\\begin{{center}}\\small {}\\end{{center}}}}",
                        affs_rendered.join(" \\\\\n")
                    )
                }
            }
            "typst" => {
                if affs_rendered.is_empty() {
                    String::new()
                } else {
                    format!(
                        "\n  #v(0.3em)\n  #text(size: 9pt, style: \"italic\")[{}]",
                        affs_rendered.join(" \\\n")
                    )
                }
            }
            _ => affs_rendered.join(", "),
        };

        if let Some(tpl) = resolve_element_template("author_block", ext) {
            let mut vars = HashMap::new();
            vars.insert("base".to_string(), ext.to_string());
            vars.insert("authors_cmd".to_string(), format!("\\author{{{}}}", authors_joined));
            vars.insert("authors".to_string(), authors_joined);
            vars.insert("affiliations".to_string(), affiliations_joined);
            vars.insert("corresponding_note".to_string(), corresponding_note);
            apply_template(&tpl, &vars)
        } else {
            format!("{}\n{}\n{}", authors_joined, affiliations_joined, corresponding_note)
        }
    } else if let Some(ref authors) = meta.author {
        match ext {
            "html" => format!("<h2>{}</h2>", authors.join(", ")),
            "latex" => format!("\\author{{{}}}", authors.join(" \\and ")),
            "typst" => format!("#text(size: 12pt)[{}]", authors.join(", ")),
            _ => authors.join(", "),
        }
    } else {
        String::new()
    }
}
