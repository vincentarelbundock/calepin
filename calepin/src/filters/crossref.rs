// Cross-reference resolution as a post-processing pass on rendered output.
//
// - resolve_html()  — Collect IDs from headings/figures/theorems/equations in HTML,
//                     then resolve @ref-id, [@ref-id], [-@ref-id] to <a> links.
// - resolve_latex() — Same for LaTeX with \hyperref and \ref commands.
// - resolve_plain() — Same for Typst/Markdown with plain "Type N" text.
//
// For collections (multi-file sites), cross-file resolution uses a two-pass
// pipeline (HTML only -- LaTeX/Typst handle global refs natively):
//   Pass 1: collect_ids_html() extracts IDs and local numbers from each page.
//   Between: CrossRefRegistry::build() merges all pages with chapter-prefixed numbers.
//   Pass 2: resolve_html_global() resolves refs using the global registry.
//
// All resolution happens after rendering (not during element processing).
// The three reference forms: @id (bare → "Type N"), [@id] (bracketed → "Type N"),
// [-@id] (suppress → "N" only).

use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

use crate::render::markers;

// ---------------------------------------------------------------------------
// Data structures for cross-file cross-reference resolution
// ---------------------------------------------------------------------------

/// IDs and counters collected from a single rendered page (pass 1).
#[derive(Debug, Clone, Default)]
pub struct PageRefData {
    /// All discovered IDs with their local (within-page) numbers.
    /// Key: full ID like "fig-scatter", Value: local number like "3".
    pub ids: HashMap<String, String>,
    /// Figure counter at end of page.
    pub fig_count: usize,
    /// Table counter at end of page.
    pub tbl_count: usize,
    /// Equation counter at end of page.
    pub eq_count: usize,
    /// Section counters at end of page.
    pub section_counters_end: [usize; 6],
}

/// A single entry in the global cross-reference registry.
#[derive(Debug, Clone)]
pub struct CrossRefEntry {
    /// Chapter-prefixed number: "2.3".
    pub number: String,
    /// Output file URL relative to site root: "guides/chapter2.html".
    pub source_url: String,
    /// Prefix type: "fig", "sec", "thm", etc.
    #[allow(dead_code)]
    pub prefix: String,
}

/// Global cross-reference registry built from all pages in a collection.
#[derive(Debug, Clone, Default)]
pub struct CrossRefRegistry {
    pub entries: HashMap<String, CrossRefEntry>,
}

impl CrossRefRegistry {
    /// Build a global registry from per-page ref data with chapter-prefixed numbering.
    ///
    /// `pages` is ordered by chapter: `(chapter_number, output_url, page_ref_data)`.
    /// Chapter numbers are 1-based. Duplicate IDs are warned about and the first
    /// definition wins.
    pub fn build(pages: &[(usize, String, PageRefData)]) -> Self {
        let mut entries = HashMap::new();
        for (chapter, url, ref_data) in pages {
            for (id, local_num) in &ref_data.ids {
                let (prefix, _) = split_ref_id(id);
                let number = renumber_with_chapter(local_num, *chapter);
                if let Some(existing) = entries.get(id) {
                    let existing: &CrossRefEntry = existing;
                    eprintln!(
                        "Warning: duplicate cross-reference ID '{}': defined in both '{}' and '{}' (keeping first)",
                        id, existing.source_url, url
                    );
                    continue;
                }
                entries.insert(id.clone(), CrossRefEntry {
                    number,
                    source_url: url.clone(),
                    prefix: prefix.to_string(),
                });
            }
        }
        Self { entries }
    }
}

/// Prepend a chapter number to a local number.
/// - Simple counter: "3" + chapter 2 -> "2.3"
/// - Section number: "1.3" + chapter 2 -> "2.1.3" (prepend)
fn renumber_with_chapter(local_num: &str, chapter: usize) -> String {
    format!("{}.{}", chapter, local_num)
}

/// All cross-referenceable prefixes (used in regex patterns).
const REF_PREFIXES: &str = "fig|sec|tbl|eq|thm|lem|cor|prp|cnj|def|exm|exr|sol|rem|alg|lst|tip|nte|wrn|imp|cau";

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
static RE_HTML_TBL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r##"id="tbl-([^"]+)""##).unwrap()
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
static RE_PLAIN_TBL_TYPST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<tbl-([^>]+)>").unwrap()
});
static RE_PLAIN_TBL_MD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r##"id="tbl-([^"]+)""##).unwrap()
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
fn resolve_grouped_html(ids: &[String], db: &HashMap<String, String>, make_href: &dyn Fn(&str) -> String) -> String {
    let parts: Vec<String> = ids.iter().map(|id| {
        let (typ, _) = split_ref_id(id);
        let label = type_label(typ);
        match db.get(id) {
            Some(num) => {
                let href = make_href(id);
                format!("<a class=\"cross-ref-{}\" href=\"{}\">{} {}</a>", typ, href, label, num)
            }
            None => { warn_unresolved(id); format!("@{}", id) }
        }
    }).collect();
    format!("({})", parts.join("; "))
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
    format!("({})", parts.join("; "))
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
    format!("({})", parts.join("; "))
}

/// Post-process rendered HTML: resolve all cross-references (no pre-collected IDs).
#[inline(never)]
#[allow(dead_code)]
pub fn resolve_html(
    html: &str,
    theorem_nums: &HashMap<String, String>,
) -> String {
    resolve_html_with_ids(html, theorem_nums, &HashMap::new())
}

/// Post-process rendered HTML with pre-collected IDs from the AST walk.
#[inline(never)]
pub fn resolve_html_with_ids(
    html: &str,
    theorem_nums: &HashMap<String, String>,
    walk_ids: &HashMap<String, String>,
) -> String {
    let ref_data = collect_ids_html(html, theorem_nums, walk_ids);
    let make_href = |id: &str| format!("#{}", id);
    resolve_html_refs(html, &ref_data.ids, &make_href)
}

/// Shared HTML cross-reference resolution: inject equation numbers, protect code blocks,
/// resolve all ref forms (@id, [@id], [-@id], grouped), and restore code blocks.
/// `make_href` builds the href for a given ID (local: `#id`, global: relative URL).
fn resolve_html_refs(
    html: &str,
    db: &HashMap<String, String>,
    make_href: &dyn Fn(&str) -> String,
) -> String {
    // Inject equation numbers
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

    // Protect code blocks from cross-ref resolution
    let mut code_blocks: Vec<String> = Vec::new();
    static RE_HTML_CODE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?s)<pre[^>]*>.*?</pre>").unwrap()
    });
    result = RE_HTML_CODE.replace_all(&result, |caps: &regex::Captures| {
        markers::wrap_raw(&mut code_blocks, caps[0].to_string())
    }).to_string();

    // Resolve cross-references (grouped first, then suppress, bracket, bare)
    result = RE_REF_GROUPED
        .replace_all(&result, |caps: &regex::Captures| {
            let ids = parse_grouped_refs(&caps[1]);
            resolve_grouped_html(&ids, db, make_href)
        }).to_string();

    result = RE_REF_SUPPRESS
        .replace_all(&result, |caps: &regex::Captures| {
            let id = &caps[1];
            match db.get(id) {
                Some(num) => {
                    let (typ, _) = split_ref_id(id);
                    let href = make_href(id);
                    format!("<a class=\"cross-ref-{}\" href=\"{}\">{}</a>", typ, href, num)
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
                    let href = make_href(id);
                    format!("[<a class=\"cross-ref-{}\" href=\"{}\">{} {}</a>]", typ, href, label, num)
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
                    let href = make_href(id);
                    format!("<a class=\"cross-ref-{}\" href=\"{}\">{} {}</a>", typ, href, label, num)
                }
                None => { warn_unresolved(id); caps[0].to_string() }
            }
        }).to_string();

    // Restore code blocks
    markers::resolve_raw(&result, &code_blocks)
}

/// Collect all cross-referenceable IDs from rendered HTML without resolving refs.
/// Used as pass 1 in the two-pass collection pipeline.
pub fn collect_ids_html(
    html: &str,
    theorem_nums: &HashMap<String, String>,
    walk_ids: &HashMap<String, String>,
) -> PageRefData {
    let mut data = PageRefData::default();

    // Section IDs from AST walk (preferred) or fallback HTML scan
    if !walk_ids.is_empty() {
        for (k, v) in walk_ids {
            data.ids.insert(k.clone(), v.clone());
        }
    } else {
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
                data.ids.insert(key, format_section_number(&counters, level));
            }
        }
        data.section_counters_end = counters;
    }

    // Figures
    for caps in RE_HTML_FIG.captures_iter(html) {
        data.fig_count += 1;
        data.ids.insert(format!("fig-{}", &caps[1]), data.fig_count.to_string());
    }

    // Tables
    for caps in RE_HTML_TBL.captures_iter(html) {
        data.tbl_count += 1;
        data.ids.insert(format!("tbl-{}", &caps[1]), data.tbl_count.to_string());
    }

    // Theorems
    for (k, v) in theorem_nums {
        data.ids.insert(k.clone(), v.clone());
    }

    // Equations
    for caps in RE_HTML_EQ.captures_iter(html) {
        data.eq_count += 1;
        data.ids.insert(format!("eq-{}", &caps[1]), data.eq_count.to_string());
    }

    data
}

/// Resolve cross-references in HTML using a global CrossRefRegistry.
/// `current_page_url` is the output URL of the page being resolved (e.g., "chapter2.html").
/// Cross-file refs link to `{target_url}#{id}`, same-page refs link to `#{id}`.
#[inline(never)]
pub fn resolve_html_global(
    html: &str,
    registry: &CrossRefRegistry,
    current_page_url: &str,
) -> String {
    let db: HashMap<String, String> = registry.entries.iter()
        .map(|(id, entry)| (id.clone(), entry.number.clone()))
        .collect();

    let make_href = |id: &str| -> String {
        if let Some(entry) = registry.entries.get(id) {
            if entry.source_url == current_page_url {
                format!("#{}", id)
            } else {
                let relative = relative_url(current_page_url, &entry.source_url);
                format!("{}#{}", relative, id)
            }
        } else {
            format!("#{}", id)
        }
    };

    resolve_html_refs(html, &db, &make_href)
}

/// Patch in-page display numbers in rendered HTML to use chapter-prefixed numbers.
/// Replaces "Figure N" in figcaptions, "Table N" in table captions,
/// equation numbers in eq-number spans, and theorem/callout headers.
pub fn renumber_display_html(html: &str, registry: &CrossRefRegistry) -> String {
    let mut result = html.to_string();

    // Build a reverse map: (prefix, local_number) is not sufficient since we
    // don't know which page we're on. Instead, iterate registry entries and
    // replace by ID.

    // Figure captions: <figcaption>Figure N: ... where id="fig-xxx" appears nearby
    // Strategy: find id="fig-xxx" in the output and replace the corresponding
    // "Figure {local}" with "Figure {global}" in the surrounding context.
    static RE_FIG_CAPTION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?s)id="(fig-[^"]+)"[^>]*>.*?<figcaption[^>]*>\s*Figure\s+(\d+)"#).unwrap()
    });
    result = RE_FIG_CAPTION.replace_all(&result, |caps: &regex::Captures| {
        let id = &caps[1];
        if let Some(entry) = registry.entries.get(id) {
            caps[0].replace(
                &format!("Figure {}", &caps[2]),
                &format!("Figure {}", entry.number),
            )
        } else {
            caps[0].to_string()
        }
    }).to_string();

    // Table captions: similar pattern with id="tbl-xxx"
    static RE_TBL_CAPTION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?s)id="(tbl-[^"]+)"[^>]*>.*?Table\s+(\d+)"#).unwrap()
    });
    result = RE_TBL_CAPTION.replace_all(&result, |caps: &regex::Captures| {
        let id = &caps[1];
        if let Some(entry) = registry.entries.get(id) {
            caps[0].replace(
                &format!("Table {}", &caps[2]),
                &format!("Table {}", entry.number),
            )
        } else {
            caps[0].to_string()
        }
    }).to_string();

    // Equation numbers: <span class="eq-number">(N)</span> preceded by id="eq-xxx"
    static RE_EQ_NUM: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?s)id="(eq-[^"]+)"[^>]*>.*?<span class="eq-number">\((\d+)\)</span>"#).unwrap()
    });
    result = RE_EQ_NUM.replace_all(&result, |caps: &regex::Captures| {
        let id = &caps[1];
        if let Some(entry) = registry.entries.get(id) {
            caps[0].replace(
                &format!("({})", &caps[2]),
                &format!("({})", entry.number),
            )
        } else {
            caps[0].to_string()
        }
    }).to_string();

    // Theorem/callout headers: id="thm-xxx" followed by "Theorem N" etc.
    static RENUMBER_REGEXES: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| {
        let prefixes = ["thm", "lem", "cor", "prp", "cnj", "def", "exm", "exr", "sol", "rem", "alg", "lst", "tip", "nte", "wrn", "imp", "cau"];
        prefixes.iter().filter_map(|prefix| {
            let label = type_label(prefix);
            if label.is_empty() { return None; }
            let pattern = format!(r#"(?s)id="({prefix}-[^"]+)"[^>]*>.*?{label}\s+(\d+)"#);
            Regex::new(&pattern).ok().map(|re| (label, re))
        }).collect()
    });
    for (label, re) in RENUMBER_REGEXES.iter() {
        result = re.replace_all(&result, |caps: &regex::Captures| {
            let id = &caps[1];
            if let Some(entry) = registry.entries.get(id) {
                caps[0].replace(
                    &format!("{} {}", label, &caps[2]),
                    &format!("{} {}", label, entry.number),
                )
            } else {
                caps[0].to_string()
            }
        }).to_string();
    }

    result
}

/// Compute a relative URL from `from` to `to`.
/// Both are paths relative to site root (e.g., "guides/intro.html", "chapter2.html").
fn relative_url(from: &str, to: &str) -> String {
    use std::path::Path;
    let from_dir = Path::new(from).parent().unwrap_or(Path::new(""));
    let to_path = Path::new(to);

    // Count how many directories to go up from `from_dir`
    let from_components: Vec<_> = from_dir.components().collect();
    let to_components: Vec<_> = to_path.components().collect();

    // Find common prefix length
    let common = from_components.iter().zip(to_components.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let ups = from_components.len() - common;
    let mut parts: Vec<String> = (0..ups).map(|_| "..".to_string()).collect();
    for comp in &to_components[common..] {
        parts.push(comp.as_os_str().to_string_lossy().to_string());
    }

    if parts.is_empty() {
        to_path.file_name().unwrap_or_default().to_string_lossy().to_string()
    } else {
        parts.join("/")
    }
}

/// Post-process rendered LaTeX: resolve cross-references.
/// LaTeX has its own \label/\ref system, so unresolved refs emit \ref{} instead
/// of warnings — LaTeX will resolve them during compilation.
#[inline(never)]
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

    // Count tables
    static RE_LATEX_TBL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\\label\{tbl-([^}]+)\}").unwrap()
    });
    let mut tbl_counter = 0usize;
    for caps in RE_LATEX_TBL.captures_iter(latex) {
        tbl_counter += 1;
        db.insert(format!("tbl-{}", &caps[1]), tbl_counter.to_string());
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

    // Count tables — collect from both syntaxes but deduplicate by label
    let mut tbl_counter = 0usize;
    let mut seen_tbls: std::collections::HashSet<String> = std::collections::HashSet::new();
    for caps in RE_PLAIN_TBL_TYPST.captures_iter(text).chain(RE_PLAIN_TBL_MD.captures_iter(text)) {
        let label = caps[1].to_string();
        if seen_tbls.insert(label.clone()) {
            tbl_counter += 1;
            db.insert(format!("tbl-{}", label), tbl_counter.to_string());
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
        "lst" => "Listing",
        "tip" => "Tip",
        "nte" => "Note",
        "wrn" => "Warning",
        "imp" => "Important",
        "cau" => "Caution",
        _ => "",
    }
}

fn warn_unresolved(id: &str) {
    use std::sync::Mutex;
    static WARNED: LazyLock<Mutex<std::collections::HashSet<String>>> =
        LazyLock::new(|| Mutex::new(std::collections::HashSet::new()));
    if let Ok(mut set) = WARNED.lock() {
        if !set.insert(id.to_string()) {
            return;
        }
    }
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

    #[test]
    fn test_resolve_callout_tip() {
        let mut nums = HashMap::new();
        nums.insert("tip-example".to_string(), "1".to_string());
        let html = "<div class=\"callout\" id=\"tip-example\"></div>\n\
                     <p>See @tip-example</p>";
        let result = resolve_html(html, &nums);
        assert!(result.contains("Tip 1"), "result: {}", result);
        assert!(result.contains("href=\"#tip-example\""), "result: {}", result);
    }

    #[test]
    fn test_resolve_callout_note() {
        let mut nums = HashMap::new();
        nums.insert("nte-important-info".to_string(), "1".to_string());
        let html = "<div class=\"callout\" id=\"nte-important-info\"></div>\n\
                     <p>See @nte-important-info</p>";
        let result = resolve_html(html, &nums);
        assert!(result.contains("Note 1"), "result: {}", result);
    }

    #[test]
    fn test_resolve_callout_warning() {
        let mut nums = HashMap::new();
        nums.insert("wrn-danger".to_string(), "1".to_string());
        let html = "<div class=\"callout\" id=\"wrn-danger\"></div>\n\
                     <p>See @wrn-danger</p>";
        let result = resolve_html(html, &nums);
        assert!(result.contains("Warning 1"), "result: {}", result);
    }

    #[test]
    fn test_resolve_callout_suppress() {
        let mut nums = HashMap::new();
        nums.insert("tip-example".to_string(), "1".to_string());
        let html = "<div class=\"callout\" id=\"tip-example\"></div>\n\
                     <p>number [-@tip-example]</p>";
        let result = resolve_html(html, &nums);
        assert!(result.contains(">1<"), "result: {}", result);
        assert!(!result.contains("Tip"), "result: {}", result);
    }

    #[test]
    fn test_resolve_listing() {
        let mut nums = HashMap::new();
        nums.insert("lst-pyplot".to_string(), "1".to_string());
        let html = "<div class=\"code-listing\" id=\"lst-pyplot\"></div>\n\
                     <p>See @lst-pyplot</p>";
        let result = resolve_html(html, &nums);
        assert!(result.contains("Listing 1"), "result: {}", result);
        assert!(result.contains("href=\"#lst-pyplot\""), "result: {}", result);
    }

    #[test]
    fn test_resolve_listing_bracket() {
        let mut nums = HashMap::new();
        nums.insert("lst-sort".to_string(), "2".to_string());
        let html = "<div class=\"code-listing\" id=\"lst-sort\"></div>\n\
                     <p>see [@lst-sort]</p>";
        let result = resolve_html(html, &nums);
        assert!(result.contains("[<a"), "result: {}", result);
        assert!(result.contains("Listing 2"), "result: {}", result);
    }

    // --- Cross-file cross-reference tests ---

    #[test]
    fn test_collect_ids_html() {
        let html = r#"<div id="fig-scatter"><figcaption>Figure 1: Scatter</figcaption></div>
                       <div id="fig-bar"><figcaption>Figure 2: Bar</figcaption></div>
                       <div class="equation" id="eq-euler"><span class="eq-number">(1)</span></div>"#;
        let data = collect_ids_html(html, &HashMap::new(), &HashMap::new());
        assert_eq!(data.ids.get("fig-scatter"), Some(&"1".to_string()));
        assert_eq!(data.ids.get("fig-bar"), Some(&"2".to_string()));
        assert_eq!(data.ids.get("eq-euler"), Some(&"1".to_string()));
        assert_eq!(data.fig_count, 2);
        assert_eq!(data.eq_count, 1);
    }

    #[test]
    fn test_collect_ids_html_with_walk_ids() {
        let html = r#"<h1 id="sec-intro">Intro</h1>"#;
        let mut walk_ids = HashMap::new();
        walk_ids.insert("sec-intro".to_string(), "1".to_string());
        let data = collect_ids_html(html, &HashMap::new(), &walk_ids);
        assert_eq!(data.ids.get("sec-intro"), Some(&"1".to_string()));
    }

    #[test]
    fn test_collect_ids_html_with_theorems() {
        let html = "";
        let mut thm_nums = HashMap::new();
        thm_nums.insert("thm-cauchy".to_string(), "1".to_string());
        let data = collect_ids_html(html, &thm_nums, &HashMap::new());
        assert_eq!(data.ids.get("thm-cauchy"), Some(&"1".to_string()));
    }

    #[test]
    fn test_renumber_with_chapter() {
        assert_eq!(renumber_with_chapter("3", 2), "2.3");
        assert_eq!(renumber_with_chapter("1.3", 2), "2.1.3");
    }

    #[test]
    fn test_registry_build() {
        let page1 = PageRefData {
            ids: [("fig-a".to_string(), "1".to_string())].into(),
            fig_count: 1,
            ..Default::default()
        };
        let page2 = PageRefData {
            ids: [("fig-b".to_string(), "1".to_string())].into(),
            fig_count: 1,
            ..Default::default()
        };
        let input = vec![
            (1, "chapter1.html".to_string(), page1),
            (2, "chapter2.html".to_string(), page2),
        ];
        let registry = CrossRefRegistry::build(&input);
        assert_eq!(registry.entries.get("fig-a").unwrap().number, "1.1");
        assert_eq!(registry.entries.get("fig-a").unwrap().source_url, "chapter1.html");
        assert_eq!(registry.entries.get("fig-b").unwrap().number, "2.1");
        assert_eq!(registry.entries.get("fig-b").unwrap().source_url, "chapter2.html");
    }

    #[test]
    fn test_registry_build_duplicate_ids_warns_keeps_first() {
        let page1 = PageRefData {
            ids: [("fig-a".to_string(), "1".to_string())].into(),
            ..Default::default()
        };
        let page2 = PageRefData {
            ids: [("fig-a".to_string(), "1".to_string())].into(),
            ..Default::default()
        };
        let input = vec![
            (1, "ch1.html".to_string(), page1),
            (2, "ch2.html".to_string(), page2),
        ];
        let registry = CrossRefRegistry::build(&input);
        // First definition wins
        assert_eq!(registry.entries.get("fig-a").unwrap().source_url, "ch1.html");
    }

    #[test]
    fn test_resolve_html_global_same_page() {
        // Build a registry with one figure on the same page
        let mut entries = HashMap::new();
        entries.insert("fig-scatter".to_string(), CrossRefEntry {
            number: "2.3".to_string(),
            source_url: "chapter2.html".to_string(),
            prefix: "fig".to_string(),
        });
        let registry = CrossRefRegistry { entries };
        let html = r#"<p>See @fig-scatter</p>"#;
        let result = resolve_html_global(html, &registry, "chapter2.html");
        assert!(result.contains("Figure 2.3"), "result: {}", result);
        assert!(result.contains("href=\"#fig-scatter\""), "result: {}", result);
    }

    #[test]
    fn test_resolve_html_global_cross_file() {
        let mut entries = HashMap::new();
        entries.insert("fig-scatter".to_string(), CrossRefEntry {
            number: "1.5".to_string(),
            source_url: "chapter1.html".to_string(),
            prefix: "fig".to_string(),
        });
        let registry = CrossRefRegistry { entries };
        let html = r#"<p>See @fig-scatter</p>"#;
        let result = resolve_html_global(html, &registry, "chapter2.html");
        assert!(result.contains("Figure 1.5"), "result: {}", result);
        assert!(result.contains("href=\"chapter1.html#fig-scatter\""), "result: {}", result);
    }

    #[test]
    fn test_resolve_html_global_cross_file_subdir() {
        let mut entries = HashMap::new();
        entries.insert("fig-a".to_string(), CrossRefEntry {
            number: "1.1".to_string(),
            source_url: "guides/intro.html".to_string(),
            prefix: "fig".to_string(),
        });
        let registry = CrossRefRegistry { entries };
        let html = r#"<p>See @fig-a</p>"#;
        let result = resolve_html_global(html, &registry, "chapters/ch2.html");
        assert!(result.contains("Figure 1.1"), "result: {}", result);
        assert!(result.contains("href=\"../guides/intro.html#fig-a\""), "result: {}", result);
    }

    #[test]
    fn test_resolve_html_global_suppress() {
        let mut entries = HashMap::new();
        entries.insert("fig-a".to_string(), CrossRefEntry {
            number: "2.1".to_string(),
            source_url: "ch2.html".to_string(),
            prefix: "fig".to_string(),
        });
        let registry = CrossRefRegistry { entries };
        let html = r#"<p>number [-@fig-a]</p>"#;
        let result = resolve_html_global(html, &registry, "ch1.html");
        assert!(result.contains(">2.1<"), "result: {}", result);
        assert!(!result.contains("Figure"), "result: {}", result);
    }

    #[test]
    fn test_relative_url_same_dir() {
        assert_eq!(relative_url("chapter1.html", "chapter2.html"), "chapter2.html");
    }

    #[test]
    fn test_relative_url_subdir() {
        assert_eq!(relative_url("chapters/ch2.html", "guides/intro.html"), "../guides/intro.html");
    }

    #[test]
    fn test_relative_url_parent() {
        assert_eq!(relative_url("guides/intro.html", "index.html"), "../index.html");
    }
}
