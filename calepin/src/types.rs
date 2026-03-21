use std::collections::HashMap;
use std::path::PathBuf;

use crate::parse::yaml::{coerce_yaml_value, build_nested_yaml, merge_yaml_value};

/// A parsed block from the .qmd file
#[derive(Debug, Clone)]
pub enum Block {
    Text(TextBlock),
    /// Executable code chunk: `{r}`, `{r, label}` with pipe options.
    Code(CodeChunk),
    /// Opaque fenced code block: ` ```python `, ` ``` ` — displayed but not executed.
    /// Bypasses Jinja, citation, and cross-reference processing.
    CodeBlock(CodeBlock),
    Div(DivBlock),
    /// A raw block: `` ```{=format} `` content `` ``` ``.
    /// Content is passed through verbatim when the output format matches.
    Raw(RawBlock),
}

/// An opaque (non-executable) fenced code block.
#[derive(Debug, Clone)]
pub struct CodeBlock {
    pub code: String,
    pub lang: String,
    pub filename: String,
}

#[derive(Debug, Clone)]
pub struct RawBlock {
    pub format: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct DivBlock {
    pub classes: Vec<String>,
    pub id: Option<String>,
    pub attrs: HashMap<String, String>,
    pub children: Vec<Block>,
}

#[derive(Debug, Clone)]
pub struct TextBlock {
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct CodeChunk {
    /// Lines of code (fences removed)
    pub source: Vec<String>,
    /// Chunk options (from header + pipe comments)
    pub options: ChunkOptions,
    /// Auto-generated or user-specified label
    pub label: String,
}

/// Chunk options stored as a string-keyed map with typed access
#[derive(Debug, Clone, Default)]
pub struct ChunkOptions {
    pub inner: HashMap<String, OptionValue>,
}

#[derive(Debug, Clone)]
pub enum OptionValue {
    Bool(bool),
    String(String),
    Number(f64),
    Null,
}

impl ChunkOptions {
    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        match self.inner.get(key) {
            Some(OptionValue::Bool(b)) => *b,
            Some(OptionValue::String(s)) => !s.is_empty() && s != "FALSE" && s != "false",
            _ => default,
        }
    }

    pub fn get_string(&self, key: &str, default: &str) -> String {
        match self.inner.get(key) {
            Some(OptionValue::String(s)) => s.clone(),
            Some(OptionValue::Bool(b)) => b.to_string(),
            Some(OptionValue::Number(n)) => n.to_string(),
            _ => default.to_string(),
        }
    }

    pub fn get_number(&self, key: &str, default: f64) -> f64 {
        match self.inner.get(key) {
            Some(OptionValue::Number(n)) => *n,
            Some(OptionValue::String(s)) => s.parse().unwrap_or(default),
            _ => default,
        }
    }

    pub fn get_opt_string(&self, key: &str) -> Option<String> {
        match self.inner.get(key) {
            Some(OptionValue::String(s)) => Some(s.clone()),
            Some(OptionValue::Null) | None => None,
            Some(OptionValue::Bool(b)) => Some(b.to_string()),
            Some(OptionValue::Number(n)) => Some(n.to_string()),
        }
    }

    // Convenience accessors mirroring calepin's reactor defaults
    pub fn cache(&self) -> bool { self.get_bool("cache", true) }
    pub fn eval(&self) -> bool { self.get_bool("eval", true) }
    #[allow(dead_code)]
    pub fn echo(&self) -> bool { self.get_bool("echo", true) }
    pub fn include(&self) -> bool { self.get_bool("include", true) }
    pub fn warning(&self) -> bool { self.get_bool("warning", true) }
    pub fn message(&self) -> bool { self.get_bool("message", true) }
    pub fn comment(&self) -> String { self.get_string("comment", "> ") }
    pub fn results(&self) -> ResultsMode {
        match self.get_string("results", "markup").as_str() {
            "asis" => ResultsMode::Asis,
            "hide" => ResultsMode::Hide,
            _ => ResultsMode::Markup,
        }
    }
    pub fn engine(&self) -> String { self.get_string("engine", "r") }
    pub fn fig_width(&self) -> f64 { self.get_number("fig.width", 7.0) }
    pub fn fig_height(&self) -> f64 { self.get_number("fig.height", 5.0) }
    pub fn fig_cap(&self) -> Option<String> { self.get_opt_string("fig.cap") }
    pub fn tbl_cap(&self) -> Option<String> { self.get_opt_string("tbl.cap") }
    pub fn fig_alt(&self) -> Option<String> { self.get_opt_string("fig.alt") }
    pub fn dev(&self) -> String { self.get_string("dev", "png") }
    pub fn fig_align(&self) -> Option<String> { self.get_opt_string("fig.align") }
    pub fn fig_scap(&self) -> Option<String> { self.get_opt_string("fig.scap") }
    pub fn fig_env(&self) -> Option<String> { self.get_opt_string("fig.env") }
    pub fn fig_pos(&self) -> Option<String> { self.get_opt_string("fig.pos") }
    pub fn fig_cap_location(&self) -> Option<String> { self.get_opt_string("fig.cap.location") }
    pub fn out_width(&self) -> Option<String> { self.get_opt_string("out.width") }
    pub fn out_height(&self) -> Option<String> { self.get_opt_string("out.height") }
    pub fn fig_link(&self) -> Option<String> { self.get_opt_string("fig.link") }

    /// Build figure rendering attributes from chunk options.
    pub fn to_figure_attrs(&self) -> FigureAttrs {
        FigureAttrs {
            width: self.out_width(),
            height: self.out_height(),
            fig_align: self.fig_align(),
            fig_scap: self.fig_scap(),
            fig_env: self.fig_env(),
            fig_pos: self.fig_pos(),
            cap_location: self.fig_cap_location(),
            link: self.fig_link(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResultsMode {
    Markup,
    Asis,
    Hide,
}

/// The result of executing a code chunk
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ChunkResult {
    Source(Vec<String>),
    Output(String),
    Warning(String),
    Message(String),
    Error(String),
    Plot(PathBuf),
    /// Raw output from knit_print (knit_asis class) — included verbatim, bypassing comment wrapping.
    Asis(String),
}

// ---------------------------------------------------------------------------
// Scholarly front matter: authors & affiliations
// ---------------------------------------------------------------------------

/// A parsed author name, split into components.
#[derive(Debug, Clone, Default)]
pub struct AuthorName {
    pub literal: String,
}

/// A rich author record (Quarto-compatible schema).
#[derive(Debug, Clone, Default)]
pub struct Author {
    pub name: AuthorName,
    pub email: Option<String>,
    pub url: Option<String>,
    pub orcid: Option<String>,
    pub note: Option<String>,
    pub corresponding: bool,
    pub equal_contributor: bool,
    pub deceased: bool,
    pub roles: Vec<String>,
    /// Indices into `Metadata.affiliations`.
    pub affiliation_ids: Vec<usize>,
}

/// An affiliation record.
#[derive(Debug, Clone, Default)]
pub struct Affiliation {
    pub id: Option<String>,
    pub number: usize,
    pub name: Option<String>,
    pub department: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub country: Option<String>,
}

impl Affiliation {
    /// A human-readable display string for this affiliation.
    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref d) = self.department { parts.push(d.as_str()); }
        if let Some(ref n) = self.name { parts.push(n.as_str()); }
        if let Some(ref c) = self.city { parts.push(c.as_str()); }
        if let Some(ref r) = self.region { parts.push(r.as_str()); }
        if let Some(ref co) = self.country { parts.push(co.as_str()); }
        parts.join(", ")
    }
}

/// Copyright metadata.
#[derive(Debug, Clone, Default)]
pub struct Copyright {
    pub holder: Option<String>,
    pub year: Option<String>,
    pub statement: Option<String>,
}

/// License metadata.
#[derive(Debug, Clone, Default)]
pub struct License {
    pub text: Option<String>,
    pub url: Option<String>,
}

/// Funding source metadata.
#[derive(Debug, Clone, Default)]
pub struct Funding {
    pub source: Option<String>,
    pub award: Option<String>,
    pub recipient: Option<String>,
    pub statement: Option<String>,
}

/// Citation metadata for making a document citeable.
#[derive(Debug, Clone, Default)]
pub struct CitationMeta {
    pub container_title: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub issued: Option<String>,
    pub doi: Option<String>,
    pub url: Option<String>,
    pub page: Option<String>,
}

/// Parsed YAML metadata
#[derive(Debug, Clone, Default)]
pub struct Metadata {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    /// Simple author name list (backward-compatible, always populated).
    pub author: Option<Vec<String>>,
    /// Rich author metadata (populated when structured author data is present).
    pub authors: Vec<Author>,
    /// Deduplicated affiliations referenced by authors.
    pub affiliations: Vec<Affiliation>,
    pub date: Option<String>,
    pub abstract_text: Option<String>,
    pub keywords: Vec<String>,
    pub copyright: Option<Copyright>,
    pub license: Option<License>,
    pub citation: Option<CitationMeta>,
    pub funding: Vec<Funding>,
    pub appendix_style: Option<String>,
    pub css: Vec<String>,
    pub header_includes: Option<String>,
    pub include_before: Option<String>,
    pub include_after: Option<String>,
    pub format: Option<String>,
    pub number_sections: bool,
    pub toc: Option<bool>,
    pub toc_depth: u8,
    pub toc_title: Option<String>,
    pub date_format: Option<String>,
    pub bibliography: Vec<String>,
    pub csl: Option<String>,
    pub plugins: Vec<String>,
    pub html_math_method: Option<String>,
    pub brand: Option<crate::brand::Brand>,
    pub var: HashMap<String, saphyr::YamlOwned>,
}

impl Metadata {
    /// Apply command-line overrides in "key=value" format.
    pub fn apply_overrides(&mut self, overrides: &[String]) {
        for item in overrides {
            let (key, value) = match item.split_once('=') {
                Some((k, v)) => (k.trim(), v.trim()),
                None => continue,
            };
            match key {
                "title" => self.title = Some(value.to_string()),
                "subtitle" => self.subtitle = Some(value.to_string()),
                "author" => self.author = Some(vec![value.to_string()]),
                "date" => self.date = Some(value.to_string()),
                "abstract" => self.abstract_text = Some(value.to_string()),
                "css" => self.css = vec![value.to_string()],
                "header-includes" => self.header_includes = Some(value.to_string()),
                "include-before" => self.include_before = Some(value.to_string()),
                "include-after" => self.include_after = Some(value.to_string()),
                "format" => self.format = Some(value.to_string()),
                "number-sections" => self.number_sections = coerce_yaml_value(value).as_bool() == Some(true),
                "toc" => self.toc = Some(coerce_yaml_value(value).as_bool() == Some(true)),
                "bibliography" => self.bibliography = vec![value.to_string()],
                "csl" => self.csl = Some(value.to_string()),
                _ => {
                    // Support dot-notation for nested keys: "a.b.c=val"
                    let parts: Vec<&str> = key.split('.').collect();
                    let coerced = coerce_yaml_value(value);
                    if parts.len() == 1 {
                        self.var.insert(key.to_string(), coerced);
                    } else {
                        let leaf = coerced;
                        let nested = build_nested_yaml(&parts, leaf);
                        merge_yaml_value(&mut self.var, &parts[0].to_string(), nested);
                    }
                }
            }
        }
    }

    /// Resolve date keywords: `today`/`now` → current date, `last-modified` → file mtime.
    /// Applies `date-format` if set (supports `%Y`, `%m`, `%d`, `%B`, `%b`, `%A`, `%a`, `%e`).
    pub fn resolve_date(&mut self, input_path: Option<&std::path::Path>) {
        let date = match self.date.as_deref() {
            Some(d) => d.trim(),
            None => return,
        };
        let secs = match date {
            "today" | "now" => {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            }
            "last-modified" | "last_modified" | "lastmodified" => {
                if let Some(path) = input_path {
                    if let Ok(meta) = std::fs::metadata(path) {
                        if let Ok(modified) = meta.modified() {
                            modified
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs()
                        } else {
                            return;
                        }
                    } else {
                        return;
                    }
                } else {
                    return;
                }
            }
            _ => return,
        };
        let resolved = match &self.date_format {
            Some(fmt) => format_date(secs, fmt),
            None => epoch_days_to_date(secs / 86400),
        };
        self.date = Some(resolved);
    }

    /// Evaluate inline code expressions (`` `{r} expr` ``, `` `{python} expr` ``)
    /// in metadata string fields.
    pub fn evaluate_inline(&mut self, ctx: &mut crate::engines::EngineContext) {
        fn eval(field: &mut Option<String>, ctx: &mut crate::engines::EngineContext) {
            if let Some(ref text) = field {
                if text.contains("`{") {
                    if let Ok(result) = crate::engines::inline::evaluate_inline(text, ctx) {
                        *field = Some(result);
                    }
                }
            }
        }
        eval(&mut self.title, ctx);
        eval(&mut self.subtitle, ctx);
        eval(&mut self.date, ctx);
        eval(&mut self.abstract_text, ctx);
        if let Some(ref mut authors) = self.author {
            for a in authors.iter_mut() {
                if a.contains("`{") {
                    if let Ok(result) = crate::engines::inline::evaluate_inline(a, ctx) {
                        *a = result;
                    }
                }
            }
        }
    }

    /// Check whether any metadata fields contain inline code for the given engine.
    pub fn has_inline_code(&self, engine: &str) -> bool {
        let pattern = format!("`{{{}", engine);
        let check = |s: &Option<String>| s.as_ref().map_or(false, |v| v.contains(&pattern));
        check(&self.title) || check(&self.subtitle) || check(&self.date) || check(&self.abstract_text)
            || self.author.as_ref().map_or(false, |authors| authors.iter().any(|a| a.contains(&pattern)))
    }
}

/// Attributes for figure rendering (sizing, alignment, LaTeX-specific options).
#[derive(Debug, Clone, Default)]
pub struct FigureAttrs {
    /// Output width: "300", "80%", "4in", etc.
    pub width: Option<String>,
    /// Output height.
    pub height: Option<String>,
    /// Alignment: "left", "center", "right", "default".
    pub fig_align: Option<String>,
    /// Short caption for List of Figures (LaTeX).
    pub fig_scap: Option<String>,
    /// LaTeX figure environment override (e.g. "figure*").
    pub fig_env: Option<String>,
    /// LaTeX figure position specifier (e.g. "htbp", "H").
    pub fig_pos: Option<String>,
    /// Caption location: "top", "bottom", "margin".
    pub cap_location: Option<String>,
    /// URL for linked figure.
    pub link: Option<String>,
}

/// An inline code expression embedded in text
#[derive(Debug, Clone)]
pub struct InlineCode {
    pub engine: String,
    pub expr: String,
}

// ---------------------------------------------------------------------------
// Element: intermediate representation between parsing and rendering
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Element {
    Text { content: String },
    CodeSource { code: String, lang: String, label: String, filename: String },
    CodeOutput { text: String },
    CodeWarning { text: String },
    CodeMessage { text: String },
    CodeError { text: String },
    Figure {
        path: PathBuf,
        alt: String,
        caption: Option<String>,
        label: String,
        number: Option<String>,
        attrs: FigureAttrs,
    },
    CodeAsis { text: String },
    Div {
        classes: Vec<String>,
        id: Option<String>,
        attrs: HashMap<String, String>,
        children: Vec<Element>,
    },
}

impl Element {
    pub fn template_name(&self) -> &str {
        match self {
            Element::CodeSource { .. } => "code_source",
            Element::CodeOutput { .. } => "code_output",
            Element::CodeWarning { .. } => "code_warning",
            Element::CodeMessage { .. } => "code_message",
            Element::CodeError { .. } => "code_error",
            Element::Figure { .. } => "figure",
            Element::Div { .. } => "div",
            Element::Text { .. } | Element::CodeAsis { .. } => "",
        }
    }
}

/// Convert days since Unix epoch to YYYY-MM-DD string.
fn epoch_days_to_date(total_days: u64) -> String {
    // Civil calendar from days since 1970-01-01
    let mut y = 1970i64;
    let mut remaining = total_days as i64;

    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }

    let leap = is_leap(y);
    let month_days: [i64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];

    let mut m = 0;
    for (i, &days) in month_days.iter().enumerate() {
        if remaining < days {
            m = i;
            break;
        }
        remaining -= days;
    }

    format!("{:04}-{:02}-{:02}", y, m + 1, remaining + 1)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Format a Unix timestamp according to a format string.
/// Supports: `%Y` (year), `%m` (month 01-12), `%d` (day 01-31), `%e` (day 1-31),
/// `%B` (month name), `%b` (month abbrev), `%A` (weekday name), `%a` (weekday abbrev).
fn format_date(secs: u64, fmt: &str) -> String {
    let days = secs / 86400;
    let ymd = epoch_days_to_date(days);
    let parts: Vec<&str> = ymd.split('-').collect();
    let (y, m, d) = (
        parts[0].parse::<i64>().unwrap_or(1970),
        parts[1].parse::<usize>().unwrap_or(1),
        parts[2].parse::<usize>().unwrap_or(1),
    );

    static MONTHS: [&str; 12] = [
        "January", "February", "March", "April", "May", "June",
        "July", "August", "September", "October", "November", "December",
    ];
    static MONTHS_SHORT: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    static DAYS: [&str; 7] = [
        "Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday",
    ];
    static DAYS_SHORT: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

    // Zeller-like day of week from days since epoch (1970-01-01 was Thursday = 4)
    let dow = ((days + 4) % 7) as usize;

    fmt.replace("%Y", &format!("{:04}", y))
        .replace("%m", &format!("{:02}", m))
        .replace("%d", &format!("{:02}", d))
        .replace("%e", &d.to_string())
        .replace("%B", MONTHS[m - 1])
        .replace("%b", MONTHS_SHORT[m - 1])
        .replace("%A", DAYS[dow])
        .replace("%a", DAYS_SHORT[dow])
}
