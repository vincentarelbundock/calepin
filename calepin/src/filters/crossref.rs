// Cross-reference resolution as a post-processing pass on rendered output.
//
// - resolve_html()  — Collect IDs from headings/figures/theorems/equations in HTML,
//                     then resolve @ref-id, [@ref-id], [-@ref-id] to <a> links.
// - resolve_latex() — Same for LaTeX with \hyperref and \ref commands.
// - resolve_plain() — Same for Typst/Markdown with plain "Type N" text.
//
// All resolution happens after rendering (not during element processing).
// The three reference forms: @id (bare → "Type N"), [@id] (bracketed → "Type N"),
// [-@id] (suppress → "N" only).

use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

use crate::render::markers;

/// All cross-referenceable prefixes (used in regex patterns).
const REF_PREFIXES: &str = "fig|sec|tbl|eq|thm|lem|cor|prp|cnj|def|exm|exr|sol|rem|alg";

// Cached regex patterns for cross-reference resolution.
// References use @prefix-label syntax (Quarto-compatible).
// The prefix is captured separately from the label.
static RE_REF_SUPPRESS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"\[-@((?:{})-[-a-zA-Z0-9_]+)\]", REF_PREFIXES)).unwrap()
});
static RE_REF_BRACKET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"\[@((?:{})-[-a-zA-Z0-9_]+)\]", REF_PREFIXES)).unwrap()
});
/// Grouped bracketed references: [@fig-one; @fig-two; @fig-three]
static RE_REF_GROUPED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"\[(@(?:{})-[-a-zA-Z0-9_]+(?:\s*;\s*@(?:{})-[-a-zA-Z0-9_]+)+)\]",
        REF_PREFIXES, REF_PREFIXES
    )).unwrap()
});
static RE_REF_BARE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"@((?:{})-[-a-zA-Z0-9_]+)", REF_PREFIXES)).unwrap()
});
// HTML-specific
static RE_HTML_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r##"<h([1-6])(?:\s[^>]*)?>.*?</h[1-6]>"##).unwrap()
});
static RE_HTML_ID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r##"\bid="([^"]*)""##).unwrap()
});
static RE_HTML_FIG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r##"id="fig-([^"]+)""##).unwrap()
});
static RE_HTML_EQ: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r##"id="eq-([^"]+)""##).unwrap()
});
// LaTeX-specific
static RE_LATEX_SECTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\\(?:sub)*(?:section|paragraph)\{[^}]*\}\s*\\label\{([^}]+)\}").unwrap()
});
static RE_LATEX_FIG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\\label\{fig-([^}]+)\}").unwrap()
});
static RE_LATEX_BRACKET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"\{{(\[)\}}(-?@(?:{})-[-a-zA-Z0-9_]+)\{{(\])\}}", REF_PREFIXES)).unwrap()
});
static RE_LATEX_SUPPRESS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"\[-@((?:{})-[-a-zA-Z0-9_]+)\]", REF_PREFIXES)).unwrap()
});
// Plain/Typst-specific
static RE_PLAIN_FIG_TYPST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<fig-([^>]+)>").unwrap()
});
static RE_PLAIN_FIG_MD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r##"id="fig-([^"]+)""##).unwrap()
});
static RE_PLAIN_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^(=+|#+)\s+(.+?)(?:\s+<([^>]+)>)?$").unwrap()
});
static RE_PLAIN_SUPPRESS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"\\?\[-@((?:{})-[-a-zA-Z0-9_]+)\\?\]", REF_PREFIXES)).unwrap()
});
static RE_PLAIN_BRACKET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"\\?\[@((?:{})-[-a-zA-Z0-9_]+)\\?\]", REF_PREFIXES)).unwrap()
});

/// Compute hierarchical section number from counters.
fn format_section_number(counters: &[usize], level: usize) -> String {
    counters[..level]
        .iter()
        .filter(|n| **n > 0)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(".")
}

/// Increment section counters at the given level and reset deeper levels.
fn advance_section_counter(counters: &mut [usize; 6], level: usize) {
    counters[level - 1] += 1;
    for l in level..6 { counters[l] = 0; }
}

/// Split a ref id like "thm-cauchy-schwarz" into prefix "thm" and label "cauchy-schwarz".
fn split_ref_id(id: &str) -> (&str, &str) {
    if let Some(pos) = id.find('-') {
        (&id[..pos], &id[pos + 1..])
    } else {
        (id, "")
    }
}

/// Parse grouped ref IDs from a string like "@fig-a; @fig-b; @tbl-c".
fn parse_grouped_refs(s: &str) -> Vec<String> {
    s.split(';')
        .map(|part| part.trim().trim_start_matches('@').to_string())
        .filter(|id| !id.is_empty())
        .collect()
}

/// Resolve grouped refs for HTML: produces comma-separated linked refs.
fn resolve_grouped_html(ids: &[String], db: &HashMap<String, String>) -> String {
    let parts: Vec<String> = ids.iter().map(|id| {
        let (typ, _) = split_ref_id(id);
        let label = type_label(typ);
        match db.get(id) {
            Some(num) => format!("<a class=\"cross-ref-{}\" href=\"#{}\">{} {}</a>", typ, id, label, num),
            None => { warn_unresolved(id); format!("@{}", id) }
        }
    }).collect();
    format!("[{}]", parts.join("; "))
}

/// Resolve grouped refs for LaTeX: produces comma-separated hyperrefs.
fn resolve_grouped_latex(ids: &[String], db: &HashMap<String, String>) -> String {
    let parts: Vec<String> = ids.iter().map(|id| {
        let (typ, _) = split_ref_id(id);
        let label = type_label(typ);
        match db.get(id) {
            Some(num) => format!("\\hyperref[{}]{{{} {}}}", id, label, num),
            None => format!("{} \\ref{{{}}}", label, id),
        }
    }).collect();
    format!("[{}]", parts.join("; "))
}

/// Resolve grouped refs for plain text (Typst/Markdown).
fn resolve_grouped_plain(ids: &[String], db: &HashMap<String, String>) -> String {
    let parts: Vec<String> = ids.iter().map(|id| {
        let (typ, _) = split_ref_id(id);
        let label = type_label(typ);
        match db.get(id) {
            Some(num) => format!("{} {}", label, num),
            None => format!("@{}", id),
        }
    }).collect();
    format!("[{}]", parts.join("; "))
}

/// Post-process rendered HTML: resolve all cross-references.
/// `theorem_nums` provides theorem numbers from the rendering phase,
/// avoiding the need to scrape them from rendered HTML.
pub fn resolve_html(html: &str, theorem_nums: &HashMap<String, String>) -> String {
    let mut db: HashMap<String, String> = HashMap::new();

    // Pass 1: Collect section IDs + numbers from headings
    let mut counters = [0usize; 6];
    for caps in RE_HTML_HEADING.captures_iter(html) {
        let level: usize = caps[1].parse().unwrap_or(1);
        let tag = caps.get(0).map_or("", |m| m.as_str());
        let id = RE_HTML_ID.captures(tag)
            .and_then(|c| c.get(1))
            .map_or("", |m| m.as_str());
        advance_section_counter(&mut counters, level);
        if !id.is_empty() {
            let key = if id.starts_with("sec-") {
                id.to_string()
            } else {
                format!("sec-{}", id)
            };
            db.insert(key, format_section_number(&counters, level));
        }
    }

    // Pass 2: Count figures from id="fig-*"
    let mut fig_counter = 0usize;
    for caps in RE_HTML_FIG.captures_iter(html) {
        fig_counter += 1;
        db.insert(format!("fig-{}", &caps[1]), fig_counter.to_string());
    }

    // Pass 3: Theorem numbers from rendering phase
    db.extend(theorem_nums.iter().map(|(k, v)| (k.clone(), v.clone())));

    // Pass 4: Count equations from id="eq-*"
    let mut eq_counter = 0usize;
    for caps in RE_HTML_EQ.captures_iter(html) {
        eq_counter += 1;
        db.insert(format!("eq-{}", &caps[1]), eq_counter.to_string());
    }

    // Pass 5: Inject equation numbers (single regex pass)
    static RE_EQ_DIV: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"<div class="equation" id="(eq-[^"]+)">"#).unwrap()
    });
    let mut result = RE_EQ_DIV.replace_all(html, |caps: &regex::Captures| {
        let label = &caps[1];
        match db.get(label) {
            Some(num) => format!(
                "<div class=\"equation\" id=\"{}\">\n<span class=\"eq-number\">({})</span>",
                label, num
            ),
            None => caps[0].to_string(),
        }
    }).to_string();

    // Pass 6: Protect code blocks from cross-ref resolution
    let mut code_blocks: Vec<String> = Vec::new();
    static RE_HTML_CODE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?s)<(?:pre|code)[^>]*>.*?</(?:pre|code)>").unwrap()
    });
    result = RE_HTML_CODE.replace_all(&result, |caps: &regex::Captures| {
        markers::wrap_raw(&mut code_blocks, caps[0].to_string())
    }).to_string();

    // Pass 7: Resolve cross-references

    // Grouped refs first (before single refs consume the @ids)
    result = RE_REF_GROUPED
        .replace_all(&result, |caps: &regex::Captures| {
            let ids = parse_grouped_refs(&caps[1]);
            resolve_grouped_html(&ids, &db)
        }).to_string();

    result = RE_REF_SUPPRESS
        .replace_all(&result, |caps: &regex::Captures| {
            let id = &caps[1];
            match db.get(id) {
                Some(num) => {
                    let (typ, _) = split_ref_id(id);
                    format!("<a class=\"cross-ref-{}\" href=\"#{}\">{}</a>", typ, id, num)
                }
                None => { warn_unresolved(id); caps[0].to_string() }
            }
        }).to_string();

    result = RE_REF_BRACKET
        .replace_all(&result, |caps: &regex::Captures| {
            let id = &caps[1];
            match db.get(id) {
                Some(num) => {
                    let (typ, _) = split_ref_id(id);
                    let label = type_label(typ);
                    format!("[<a class=\"cross-ref-{}\" href=\"#{}\">{} {}</a>]", typ, id, label, num)
                }
                None => { warn_unresolved(id); caps[0].to_string() }
            }
        }).to_string();

    result = RE_REF_BARE
        .replace_all(&result, |caps: &regex::Captures| {
            let id = &caps[1];
            match db.get(id) {
                Some(num) => {
                    let (typ, _) = split_ref_id(id);
                    let label = type_label(typ);
                    format!("<a class=\"cross-ref-{}\" href=\"#{}\">{} {}</a>", typ, id, label, num)
                }
                None => { warn_unresolved(id); caps[0].to_string() }
            }
        }).to_string();

    // Restore code blocks
    result = markers::resolve_raw(&result, &code_blocks);

    result
}

/// Post-process rendered LaTeX: resolve cross-references.
/// LaTeX has its own \label/\ref system, so unresolved refs emit \ref{} instead
/// of warnings — LaTeX will resolve them during compilation.
pub fn resolve_latex(latex: &str, theorem_nums: &HashMap<String, String>) -> String {
    let mut db: HashMap<String, String> = HashMap::new();

    // Collect section numbers
    let mut counters = [0usize; 6];
    for caps in RE_LATEX_SECTION.captures_iter(latex) {
        let label = &caps[1];
        let cmd = &caps[0];
        let level = if cmd.contains("subsubsection") { 3 }
            else if cmd.contains("subsection") { 2 }
            else if cmd.contains("paragraph") { 4 }
            else { 1 };
        advance_section_counter(&mut counters, level);
        let key = if label.starts_with("sec-") {
            label.to_string()
        } else {
            format!("sec-{}", label)
        };
        db.insert(key, format_section_number(&counters, level));
    }

    // Count figures
    let mut fig_counter = 0usize;
    for caps in RE_LATEX_FIG.captures_iter(latex) {
        fig_counter += 1;
        db.insert(format!("fig-{}", &caps[1]), fig_counter.to_string());
    }

    // Theorem numbers from rendering phase
    db.extend(theorem_nums.iter().map(|(k, v)| (k.clone(), v.clone())));

    // Count equations from \label{eq-*}
    static RE_LATEX_EQ: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\\label\{eq-([^}]+)\}").unwrap()
    });
    let mut eq_counter = 0usize;
    for caps in RE_LATEX_EQ.captures_iter(latex) {
        eq_counter += 1;
        db.insert(format!("eq-{}", &caps[1]), eq_counter.to_string());
    }

    // Protect verbatim/code blocks from cross-ref resolution
    let mut result = latex.to_string();
    let mut verbatim_blocks: Vec<String> = Vec::new();
    static RE_VERBATIM: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?s)\\begin\{(?:Verbatim|verbatim)\}.*?\\end\{(?:Verbatim|verbatim)\}").unwrap()
    });
    result = RE_VERBATIM.replace_all(&result, |caps: &regex::Captures| {
        markers::wrap_raw(&mut verbatim_blocks, caps[0].to_string())
    }).to_string();

    // Normalize LaTeX-escaped brackets: {[}-@key{]} → [-@key]
    result = RE_LATEX_BRACKET
        .replace_all(&result, "$1$2$3")
        .to_string();

    // Grouped refs
    result = RE_REF_GROUPED
        .replace_all(&result, |caps: &regex::Captures| {
            let ids = parse_grouped_refs(&caps[1]);
            resolve_grouped_latex(&ids, &db)
        }).to_string();

    result = RE_LATEX_SUPPRESS
        .replace_all(&result, |caps: &regex::Captures| {
            let id = &caps[1];
            match db.get(id) {
                Some(num) => format!("\\hyperref[{}]{{{}}}", id, num),
                None => format!("\\ref{{{}}}", id),
            }
        }).to_string();

    result = RE_REF_BRACKET
        .replace_all(&result, |caps: &regex::Captures| {
            let id = &caps[1];
            let (typ, _) = split_ref_id(id);
            let label = type_label(typ);
            match db.get(id) {
                Some(num) => format!("[\\hyperref[{}]{{{} {}}}]", id, label, num),
                None => format!("{} \\ref{{{}}}", label, id),
            }
        }).to_string();

    result = RE_REF_BARE
        .replace_all(&result, |caps: &regex::Captures| {
            let id = &caps[1];
            let (typ, _) = split_ref_id(id);
            let label = type_label(typ);
            match db.get(id) {
                Some(num) => format!("\\hyperref[{}]{{{} {}}}", id, label, num),
                None => format!("{} \\ref{{{}}}", label, id),
            }
        }).to_string();

    // Restore verbatim blocks
    result = markers::resolve_raw(&result, &verbatim_blocks);

    result
}

/// Post-process for Typst/Markdown: resolve cross-refs as plain text.
/// Must resolve them — Typst interprets bare `@label` as its own ref syntax.
pub fn resolve_plain(text: &str, theorem_nums: &HashMap<String, String>) -> String {
    let mut db: HashMap<String, String> = HashMap::new();

    // Count figures — collect from both syntaxes but deduplicate by label
    let mut fig_counter = 0usize;
    let mut seen_figs: std::collections::HashSet<String> = std::collections::HashSet::new();
    for caps in RE_PLAIN_FIG_TYPST.captures_iter(text).chain(RE_PLAIN_FIG_MD.captures_iter(text)) {
        let label = caps[1].to_string();
        if seen_figs.insert(label.clone()) {
            fig_counter += 1;
            db.insert(format!("fig-{}", label), fig_counter.to_string());
        }
    }

    // Count headings
    let mut counters = [0usize; 6];
    for caps in RE_PLAIN_HEADING.captures_iter(text) {
        let marker = &caps[1];
        let level = marker.len();
        if level >= 1 && level <= 6 {
            advance_section_counter(&mut counters, level);
            let heading_text = &caps[2];
            let slug = caps.get(3)
                .map(|m| m.as_str().to_string())
                .unwrap_or_else(|| slugify(heading_text));
            if !slug.is_empty() {
                let key = if slug.starts_with("sec-") {
                    slug.to_string()
                } else {
                    format!("sec-{}", slug)
                };
                db.insert(key, format_section_number(&counters, level));
            }
        }
    }

    // Theorem numbers from rendering phase
    db.extend(theorem_nums.iter().map(|(k, v)| (k.clone(), v.clone())));

    // Count equations from <eq-*>
    static RE_PLAIN_EQ: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"<eq-([^>]+)>").unwrap()
    });
    let mut eq_counter = 0usize;
    for caps in RE_PLAIN_EQ.captures_iter(text) {
        eq_counter += 1;
        db.insert(format!("eq-{}", &caps[1]), eq_counter.to_string());
    }

    // Protect fenced code blocks from cross-ref resolution
    let mut result = text.to_string();
    let mut code_blocks: Vec<String> = Vec::new();
    static RE_PLAIN_CODE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^```[^\n]*\n[\s\S]*?^```").unwrap()
    });
    result = RE_PLAIN_CODE.replace_all(&result, |caps: &regex::Captures| {
        markers::wrap_raw(&mut code_blocks, caps[0].to_string())
    }).to_string();

    // Resolve refs

    // Grouped refs
    result = RE_REF_GROUPED
        .replace_all(&result, |caps: &regex::Captures| {
            let ids = parse_grouped_refs(&caps[1]);
            resolve_grouped_plain(&ids, &db)
        }).to_string();

    result = RE_PLAIN_SUPPRESS
        .replace_all(&result, |caps: &regex::Captures| {
            db.get(&caps[1]).cloned().unwrap_or_else(|| caps[0].to_string())
        }).to_string();

    result = RE_PLAIN_BRACKET
        .replace_all(&result, |caps: &regex::Captures| {
            let id = &caps[1];
            let (typ, _) = split_ref_id(id);
            let label = type_label(typ);
            match db.get(id) {
                Some(num) => format!("[{} {}]", label, num),
                None => caps[0].to_string(),
            }
        }).to_string();

    result = RE_REF_BARE
        .replace_all(&result, |caps: &regex::Captures| {
            let id = &caps[1];
            let (typ, _) = split_ref_id(id);
            let label = type_label(typ);
            match db.get(id) {
                Some(num) => format!("{} {}", label, num),
                None => caps[0].to_string(),
            }
        }).to_string();

    // Restore code blocks
    result = markers::resolve_raw(&result, &code_blocks);

    result
}

use crate::util::slugify;

// Helpers

fn type_label(typ: &str) -> &str {
    match typ {
        "fig" => "Figure",
        "sec" => "Section",
        "tbl" => "Table",
        "eq" => "Equation",
        "thm" => "Theorem",
        "lem" => "Lemma",
        "cor" => "Corollary",
        "prp" => "Proposition",
        "cnj" => "Conjecture",
        "def" => "Definition",
        "exm" => "Example",
        "exr" => "Exercise",
        "sol" => "Solution",
        "rem" => "Remark",
        "alg" => "Algorithm",
        _ => "",
    }
}

fn warn_unresolved(id: &str) {
    cwarn!("unresolved reference @{}", id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_html_sections() {
        let html = "<h1 id=\"sec-intro\">Intro</h1>\n\
                     <h2 id=\"methods\">Methods</h2>\n\
                     <p>See @sec-intro and @sec-methods</p>";
        let result = resolve_html(html, &HashMap::new());
        assert!(result.contains("Section 1"), "result: {}", result);
        assert!(result.contains("Section 1.1"), "result: {}", result);
    }

    #[test]
    fn test_resolve_html_figures() {
        let html = "<div class=\"figure\" id=\"fig-scatter\"><p>Caption</p></div>\n\
                     <p>See @fig-scatter</p>";
        let result = resolve_html(html, &HashMap::new());
        assert!(result.contains("Figure 1"), "result: {}", result);
        assert!(result.contains("fig-scatter"), "result: {}", result);
    }

    #[test]
    fn test_resolve_suppress() {
        let html = "<div id=\"fig-scatter\"></div>\n\
                     <p>number [-@fig-scatter]</p>";
        let result = resolve_html(html, &HashMap::new());
        assert!(result.contains(">1<"), "result: {}", result);
        assert!(!result.contains("Figure"), "result: {}", result);
    }

    #[test]
    fn test_resolve_bracket() {
        let html = "<h1 id=\"sec-intro\">Intro</h1>\n\
                     <p>see [@sec-intro]</p>";
        let result = resolve_html(html, &HashMap::new());
        assert!(result.contains("[<a"), "result: {}", result);
        assert!(result.contains("Section 1</a>]"), "result: {}", result);
    }
}
