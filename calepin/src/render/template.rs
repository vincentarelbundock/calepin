use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::types::Metadata;

static RE_TOC_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<h([1-6])\s[^>]*id="([^"]+)"[^>]*>(.*?)</h[1-6]>"#).unwrap()
});
static RE_TOC_TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<[^>]+>").unwrap()
});
static RE_TEMPLATE_VAR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{\{([a-zA-Z_][-_a-zA-Z0-9]*)\}\}").unwrap()
});

/// Build an HTML table of contents from rendered headings in the body.
fn build_html_toc(body: &str, depth: u8, title: &str) -> String {
    let re = &RE_TOC_HEADING;
    let tag_re = &RE_TOC_TAG;

    let mut items: Vec<(u8, String, String)> = Vec::new();
    for cap in re.captures_iter(body) {
        let level: u8 = cap[1].parse().unwrap_or(1);
        if level > depth { continue; }
        let id = cap[2].to_string();
        let text = tag_re.replace_all(&cap[3], "").trim().to_string();
        if text.is_empty() { continue; }
        items.push((level, id, text));
    }

    if items.is_empty() { return String::new(); }

    let min_level = items.iter().map(|(l, _, _)| *l).min().unwrap_or(1);
    let mut html = format!("<nav class=\"toc\" aria-label=\"{}\">\n<p class=\"toc-title\">{}</p>\n<ul>\n", title, title);
    let mut current_level = min_level;
    let mut first = true;

    for (level, id, text) in &items {
        if *level > current_level {
            // Going deeper: nest inside the current <li> (which was left open)
            while current_level < *level {
                html.push_str("\n<ul>\n");
                current_level += 1;
            }
        } else {
            // Close the previous <li> before siblings or shallower items
            if !first {
                html.push_str("</li>\n");
            }
            // Going shallower: close nested lists
            while current_level > *level {
                html.push_str("</ul>\n</li>\n");
                current_level -= 1;
            }
        }
        html.push_str(&format!("<li><a href=\"#{}\">{}</a>", id, text));
        first = false;
    }

    // Close all remaining open tags
    if !first {
        html.push_str("</li>\n");
    }
    while current_level > min_level {
        html.push_str("</ul>\n</li>\n");
        current_level -= 1;
    }

    html.push_str("</ul>\n</nav>");
    html
}

/// Strip markdown image/link syntax to produce plain text for <title> etc.
/// `![alt](url)` → `alt`, `[text](url)` → `text`.
/// For images with no alt text, extracts the filename stem from the URL.
fn strip_markdown_formatting(text: &str) -> String {
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

const BUILTIN_HTML_TEMPLATE: &str = include_str!("../templates/pages/calepin.html");
const BUILTIN_LATEX_TEMPLATE: &str = include_str!("../templates/pages/calepin.latex");
const BUILTIN_TYPST_TEMPLATE: &str = include_str!("../templates/pages/calepin.typst");
const BUILTIN_CSS: &str = include_str!("../templates/pages/calepin.css");

/// Load a page template with override lookup:
///   1. _calepin/templates/{filename}
///   2. ~/.config/calepin/templates/{filename}
///   3. Built-in (compiled into binary)
pub fn load_page_template(filename: &str) -> String {
    if let Some(path) = crate::util::resolve_path("templates", filename) {
        if let Ok(s) = std::fs::read_to_string(&path) {
            return s;
        }
    }
    match filename {
        "calepin.html" => BUILTIN_HTML_TEMPLATE.to_string(),
        "calepin.latex" => BUILTIN_LATEX_TEMPLATE.to_string(),
        "calepin.typst" => BUILTIN_TYPST_TEMPLATE.to_string(),
        "calepin.css" => BUILTIN_CSS.to_string(),
        _ => String::new(),
    }
}

pub fn html_template() -> String { load_page_template("calepin.html") }
pub fn latex_template() -> String { load_page_template("calepin.latex") }
pub fn typst_template() -> String { load_page_template("calepin.typst") }
pub fn default_css() -> String { load_page_template("calepin.css") }

/// Apply {{variable}} substitution to a template string.
/// This is the single templating engine used by both page templates
/// and element templates.
///
/// Two-pass: first replace all non-body vars (body may contain {{var}} patterns),
/// then replace {{body}}.
pub fn apply_template(template: &str, vars: &HashMap<String, String>) -> String {
    let result = RE_TEMPLATE_VAR.replace_all(template, |caps: &regex::Captures| {
        let name = &caps[1];
        if name == "body" {
            return caps[0].to_string(); // defer body to second pass
        }
        vars.get(name).cloned().unwrap_or_default()
    });
    // Second pass: replace {{body}}
    match vars.get("body") {
        Some(body) => result.replace("{{body}}", body),
        None => result.replace("{{body}}", ""),
    }
}

/// Render a metadata field through an element template if available,
/// falling back to the provided default.
fn render_block(name: &str, ext: &str, vars: &HashMap<String, String>, fallback: &str) -> String {
    use crate::render::elements::resolve_element_template;
    if let Some(tpl) = resolve_element_template(name, ext) {
        apply_template(&tpl, vars)
    } else {
        fallback.to_string()
    }
}

/// Build page template variables from metadata and rendered body.
/// Shared across all output formats; format-specific blocks are rendered
/// through overridable element templates.
pub fn build_template_vars(meta: &Metadata, body: &str, ext: &str) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    vars.insert("body".to_string(), body.to_string());
    vars.insert(
        "generator".to_string(),
        format!("calepin {}", env!("CARGO_PKG_VERSION")),
    );
    vars.insert("preamble".to_string(), String::new());

    // Plain title (used in <title> etc.) — strip markdown image/link syntax
    let plain_title = meta.title.as_deref().unwrap_or("Untitled");
    let plain_title = strip_markdown_formatting(plain_title);
    vars.insert("plain-title".to_string(), plain_title);
    vars.insert("title".to_string(),
        meta.title.as_deref()
            .map(|t| crate::render::markdown::render_inline(t, ext))
            .unwrap_or_default(),
    );
    vars.insert(
        "author".to_string(),
        meta.author.as_ref()
            .map(|a| a.iter()
                .map(|name| crate::render::markdown::render_inline(name, ext))
                .collect::<Vec<_>>()
                .join(", "))
            .unwrap_or_default(),
    );
    vars.insert("date".to_string(), meta.date.clone().unwrap_or_default());

    // Title block
    if let Some(ref title) = meta.title {
        let rendered_title = crate::render::markdown::render_inline(title, ext);
        let mut bvars = HashMap::new();
        bvars.insert("title".to_string(), rendered_title.clone());
        vars.insert("title-block".to_string(), render_block("title-block", ext, &bvars, &rendered_title));
    } else {
        // LaTeX requires \title{} even if empty (for \maketitle)
        vars.insert("title-block".to_string(), match ext {
            "latex" => "\\title{}".to_string(),
            _ => String::new(),
        });
    }

    // Subtitle block (HTML only by default, but templates can provide others)
    if let Some(ref subtitle) = meta.subtitle {
        let rendered_subtitle = crate::render::markdown::render_inline(subtitle, ext);
        let mut bvars = HashMap::new();
        bvars.insert("subtitle".to_string(), rendered_subtitle.clone());
        vars.insert("subtitle-block".to_string(), render_block("subtitle-block", ext, &bvars, &rendered_subtitle));
    } else {
        vars.insert("subtitle-block".to_string(), String::new());
    }

    // Author block
    vars.insert("author-block".to_string(), build_author_block(meta, ext));

    // Date block
    if let Some(ref date) = meta.date {
        let mut bvars = HashMap::new();
        bvars.insert("date".to_string(), date.clone());
        let fallback = match ext {
            "latex" => format!("\\date{{{}}}", date),
            _ => date.clone(),
        };
        vars.insert("date-block".to_string(), render_block("date-block", ext, &bvars, &fallback));
    } else {
        // LaTeX needs an empty \date{} to suppress "today"
        vars.insert("date-block".to_string(), match ext {
            "latex" => "\\date{}".to_string(),
            _ => String::new(),
        });
    }

    // Abstract block
    if let Some(ref abs) = meta.abstract_text {
        let rendered_abs = crate::render::markdown::render_inline(abs, ext);
        let mut bvars = HashMap::new();
        bvars.insert("abstract".to_string(), rendered_abs.clone());
        vars.insert("abstract-block".to_string(), render_block("abstract-block", ext, &bvars, &rendered_abs));
    } else {
        vars.insert("abstract-block".to_string(), String::new());
    }

    // Keywords block
    if !meta.keywords.is_empty() {
        let joined = meta.keywords.join(", ");
        let mut bvars = HashMap::new();
        bvars.insert("keywords".to_string(), joined.clone());
        vars.insert("keywords-block".to_string(), render_block("keywords-block", ext, &bvars, &joined));

        // HTML meta tag for keywords
        if ext == "html" {
            vars.insert(
                "preamble".to_string(),
                format!(
                    "{}<meta name=\"keywords\" content=\"{}\">",
                    vars.get("preamble").cloned().unwrap_or_default(),
                    joined
                ),
            );
        }
    } else {
        vars.insert("keywords-block".to_string(), String::new());
    }

    // Appendix
    vars.insert("appendix-block".to_string(), build_appendix(meta, ext));

    // CSS (HTML only)
    if ext == "html" {
        let mut css_parts = vec![format!("<style>\n{}\n</style>", default_css())];
        for css_file in &meta.css {
            css_parts.push(format!("<link rel=\"stylesheet\" href=\"{}\">", css_file));
        }
        vars.insert("css".to_string(), css_parts.join("\n"));
        vars.insert("js".to_string(), String::new());
        vars.insert("body-class".to_string(), "body".to_string());
    }

    // LaTeX-specific defaults
    if ext == "latex" {
        vars.insert("documentclass".to_string(), "article".to_string());
        vars.insert("classoption".to_string(), "11pt".to_string());
        vars.insert("bib-preamble".to_string(), String::new());
        vars.insert("bib-end".to_string(), String::new());
    }

    // Common include variables
    vars.insert(
        "header-includes".to_string(),
        meta.header_includes.clone().unwrap_or_default(),
    );
    vars.insert(
        "include-before".to_string(),
        meta.include_before.clone().unwrap_or_default(),
    );
    vars.insert(
        "include-after".to_string(),
        meta.include_after.clone().unwrap_or_default(),
    );

    // Table of contents
    if meta.toc {
        let toc_depth = if meta.toc_depth == 0 { 3 } else { meta.toc_depth };
        let toc_title = meta.toc_title.as_deref().unwrap_or("Contents");
        let toc = match ext {
            "html" => build_html_toc(body, toc_depth, toc_title),
            "latex" => format!("\\setcounter{{tocdepth}}{{{}}}\n\\tableofcontents", toc_depth),
            "typst" => format!("#outline(depth: {})", toc_depth),
            _ => String::new(),
        };
        vars.insert("toc".to_string(), toc);
    } else {
        vars.insert("toc".to_string(), String::new());
    }

    // Extra YAML fields override defaults (e.g., classoption, documentclass)
    for (key, value) in &meta.extra {
        let s = if let Some(s) = value.as_str() {
            s.to_string()
        } else if let Some(b) = value.as_bool() {
            b.to_string()
        } else if let Some(n) = value.as_integer() {
            n.to_string()
        } else if let Some(f) = value.as_floating_point() {
            f.to_string()
        } else {
            continue
        };
        vars.insert(key.clone(), s);
    }

    vars
}

/// Convenience: build HTML template variables.
pub fn build_html_vars(meta: &Metadata, body: &str) -> HashMap<String, String> {
    build_template_vars(meta, body, "html")
}

/// Build the appendix block for any format using element templates.
/// Format-specific content (links, lists) is computed in Rust;
/// structural markup comes from overridable templates.
fn build_appendix(meta: &Metadata, ext: &str) -> String {
    use crate::render::elements::resolve_element_template;

    let style = meta.appendix_style.as_deref().unwrap_or("default");
    if style == "none" {
        return String::new();
    }

    let mut sections: Vec<String> = Vec::new();

    // License
    if let Some(ref lic) = meta.license {
        if let Some(ref text) = lic.text {
            let content = format_link(text, lic.url.as_deref(), ext);
            if let Some(tpl) = resolve_element_template("appendix-license", ext) {
                let mut vars = HashMap::new();
                vars.insert("content".to_string(), content);
                sections.push(apply_template(&tpl, &vars));
            }
        }
    }

    // Citation
    if let Some(ref cite) = meta.citation {
        let content = build_citation_text(meta, cite, ext);
        if let Some(tpl) = resolve_element_template("appendix-citation", ext) {
            let mut vars = HashMap::new();
            vars.insert("content".to_string(), content);
            sections.push(apply_template(&tpl, &vars));
        }
    }

    // Copyright
    if let Some(ref cr) = meta.copyright {
        let text = build_copyright_text(cr);
        if !text.is_empty() {
            if let Some(tpl) = resolve_element_template("appendix-copyright", ext) {
                let mut vars = HashMap::new();
                vars.insert("content".to_string(), text);
                sections.push(apply_template(&tpl, &vars));
            }
        }
    }

    // Funding
    if !meta.funding.is_empty() {
        let items = build_funding_items(&meta.funding, ext);
        if !items.is_empty() {
            if let Some(tpl) = resolve_element_template("appendix-funding", ext) {
                let mut vars = HashMap::new();
                vars.insert("items".to_string(), items);
                sections.push(apply_template(&tpl, &vars));
            }
        }
    }

    if sections.is_empty() {
        String::new()
    } else if let Some(tpl) = resolve_element_template("appendix", ext) {
        let mut vars = HashMap::new();
        vars.insert("sections".to_string(), sections.join("\n"));
        apply_template(&tpl, &vars)
    } else {
        sections.join("\n\n")
    }
}

/// Format a text+url as a link in the appropriate output format.
fn format_link(text: &str, url: Option<&str>, ext: &str) -> String {
    match url {
        Some(url) => match ext {
            "html" => format!("<a href=\"{}\">{}</a>", url, text),
            "latex" => format!("{} (\\url{{{}}})", text, url),
            "typst" => format!("#link(\"{}\")[{}]", url, text),
            _ => format!("[{}]({})", text, url),
        },
        None => text.to_string(),
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
        match ext {
            "html" => parts.push(format!("<em>{}</em>", ct)),
            "latex" => parts.push(format!("\\emph{{{}}}", ct)),
            "typst" => parts.push(format!("#emph[{}]", ct)),
            _ => parts.push(format!("*{}*", ct)),
        }
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
        match ext {
            "html" => citation.push_str(&format!(" DOI: <a href=\"{}\">{}</a>", doi_url, doi)),
            "latex" => citation.push_str(&format!(" DOI: \\url{{{}}}", doi_url)),
            "typst" => citation.push_str(&format!(" DOI: #link(\"{}\")[{}]", doi_url, doi)),
            _ => citation.push_str(&format!(" DOI: [{}]({})", doi, doi_url)),
        }
    }
    if let Some(ref url) = cite.url {
        match ext {
            "html" => citation.push_str(&format!(" URL: <a href=\"{}\">{}</a>", url, url)),
            "latex" => citation.push_str(&format!(" URL: \\url{{{}}}", url)),
            "typst" => citation.push_str(&format!(" URL: #link(\"{}\")", url)),
            _ => citation.push_str(&format!(" URL: <{}>", url)),
        }
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
fn build_author_block(meta: &Metadata, ext: &str) -> String {
    use crate::render::elements::resolve_element_template;

    let has_rich = !meta.authors.is_empty()
        && meta.authors.iter().any(|a| {
            !a.affiliation_ids.is_empty()
                || a.email.is_some()
                || a.orcid.is_some()
                || a.corresponding
        });

    if has_rich {
        // Render each author through the author-item template
        let author_tpl = resolve_element_template("author-item", ext);
        let authors_rendered: Vec<String> = meta.authors.iter().map(|author| {
            let superscripts = if !author.affiliation_ids.is_empty() && meta.affiliations.len() > 1 {
                let sups: Vec<String> = author.affiliation_ids.iter()
                    .filter_map(|&i| meta.affiliations.get(i).map(|a| a.number.to_string()))
                    .collect();
                match ext {
                    "html" => format!("<sup>{}</sup>", sups.join(",")),
                    "latex" => format!("\\textsuperscript{{{}}}", sups.join(",")),
                    "typst" => format!("#super[{}]", sups.join(",")),
                    _ => String::new(),
                }
            } else {
                String::new()
            };

            let corresponding = if author.corresponding {
                match ext {
                    "html" => "<sup>*</sup>".to_string(),
                    "latex" => "\\textsuperscript{*}".to_string(),
                    "typst" => "#super[\\*]".to_string(),
                    _ => String::new(),
                }
            } else {
                String::new()
            };

            let orcid_link = if let Some(ref orcid) = author.orcid {
                let url = if orcid.starts_with("http") { orcid.clone() } else { format!("https://orcid.org/{}", orcid) };
                match ext {
                    "html" => format!(
                        " <a href=\"{}\" class=\"author-orcid\" title=\"ORCID\">\
                         <svg width=\"16\" height=\"16\" viewBox=\"0 0 256 256\" fill=\"#A6CE39\">\
                         <path d=\"M256,128c0,70.7-57.3,128-128,128S0,198.7,0,128,57.3,0,128,0s128,57.3,128,128Z\"/>\
                         <path fill=\"#fff\" d=\"M86.3,186.2H70.9V79.1h15.4v107.1ZM78.6,56.8c-5.5,0-10,4.5-10,10s4.5,10,10,10,10-4.5,10-10-4.5-10-10-10Z\
                         M162.2,79.1c-24.4,0-37.2,12.8-43.6,23.6V79.1H103.2V186.2h15.4V120.8c0-12.4,14.3-27.3,33.5-27.3,20.3,0,28.3,14.5,28.3,34v58.7h15.4V125.3C195.8,99.6,184.3,79.1,162.2,79.1Z\"/>\
                         </svg></a>",
                        url
                    ),
                    _ => String::new(),
                }
            } else {
                String::new()
            };

            if let Some(ref tpl) = author_tpl {
                let mut vars = HashMap::new();
                vars.insert("name".to_string(), author.name.literal.clone());
                vars.insert("superscripts".to_string(), superscripts);
                vars.insert("corresponding".to_string(), corresponding);
                vars.insert("orcid-link".to_string(), orcid_link);
                apply_template(tpl, &vars)
            } else {
                format!("{}{}{}{}", author.name.literal, superscripts, corresponding, orcid_link)
            }
        }).collect();

        // Render each affiliation through the affiliation-item template
        let aff_tpl = resolve_element_template("affiliation-item", ext);
        let affs_rendered: Vec<String> = meta.affiliations.iter().filter_map(|aff| {
            let display = aff.display();
            if display.is_empty() {
                return None;
            }
            let number = if meta.affiliations.len() > 1 {
                match ext {
                    "html" => format!("<sup>{}</sup> ", aff.number),
                    "latex" => format!("\\textsuperscript{{{}}} ", aff.number),
                    "typst" => format!("#super[{}] ", aff.number),
                    _ => String::new(),
                }
            } else {
                String::new()
            };
            if let Some(ref tpl) = aff_tpl {
                let mut vars = HashMap::new();
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
                    Some(match ext {
                        "html" => format!(
                            "  <p class=\"corresponding\">* Corresponding author: <a href=\"mailto:{}\">{}</a></p>",
                            email, email
                        ),
                        "latex" => format!("\\textsuperscript{{*}} Corresponding author: \\url{{{}}}", email),
                        "typst" => format!("#super[\\*] Corresponding author: #link(\"mailto:{}\")", email),
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

        if let Some(tpl) = resolve_element_template("author-block", ext) {
            let mut vars = HashMap::new();
            vars.insert("authors".to_string(), authors_joined);
            vars.insert("affiliations".to_string(), affiliations_joined);
            vars.insert("corresponding-note".to_string(), corresponding_note);
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

/// Convenience: build LaTeX template variables.
pub fn build_latex_vars(meta: &Metadata, body: &str) -> HashMap<String, String> {
    build_template_vars(meta, body, "latex")
}

/// Convenience: build Typst template variables.
pub fn build_typst_vars(meta: &Metadata, body: &str) -> HashMap<String, String> {
    build_template_vars(meta, body, "typst")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_template() {
        let template = "<title>{{title}}</title>\n<body>{{body}}</body>";
        let mut vars = HashMap::new();
        vars.insert("title".to_string(), "Hello".to_string());
        vars.insert("body".to_string(), "<p>World</p>".to_string());
        let result = apply_template(template, &vars);
        assert_eq!(result, "<title>Hello</title>\n<body><p>World</p></body>");
    }

    #[test]
    fn test_missing_vars_become_empty() {
        let template = "{{title}}: {{missing}}";
        let mut vars = HashMap::new();
        vars.insert("title".to_string(), "Hello".to_string());
        let result = apply_template(template, &vars);
        assert_eq!(result, "Hello: ");
    }
}
