//! Metadata types: Metadata struct, Author, Affiliation, Copyright, License, etc.

use std::collections::HashMap;

use crate::value::{self, Value as MetaValue};

// ---------------------------------------------------------------------------
// Scholarly front matter: authors & affiliations
// ---------------------------------------------------------------------------

/// A parsed author name, split into components.
#[derive(Debug, Clone, Default)]
pub struct AuthorName {
    pub literal: String,
    pub given: Option<String>,
    pub family: Option<String>,
}

/// A rich author record with structured metadata.
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

/// Unified document/project metadata.
///
/// Both `_calepin.toml` (project config) and front matter (YAML/TOML preamble)
/// parse into this type. Document-level fields override project-level fields
/// via `Metadata::merge()`.
#[derive(Debug, Clone, Default)]
pub struct Metadata {
    // -- Document identity --
    pub title: Option<String>,
    pub subtitle: Option<String>,
    /// Simple author name list (always populated from structured or plain author data).
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

    // -- Rendering --
    pub target: Option<String>,
    pub theme: Option<String>,
    pub number_sections: bool,
    pub toc: Option<bool>,
    pub toc_depth: u8,
    pub toc_title: Option<String>,
    pub date_format: Option<String>,
    pub bibliography: Vec<String>,
    pub csl: Option<String>,
    pub plugins: Vec<String>,
    pub convert_math: bool,
    pub html_math_method: Option<String>,

    // -- Project-level fields (also settable in front matter) --
    pub output: Option<String>,
    pub lang: Option<String>,
    pub url: Option<String>,
    pub favicon: Option<String>,
    pub logo: Option<String>,
    pub logo_dark: Option<String>,
    pub orchestrator: Option<String>,
    pub global_crossref: bool,
    pub static_dirs: Vec<String>,
    pub embed_resources: Option<bool>,

    // -- Collection structure --
    pub contents: Vec<crate::project::ContentSection>,
    pub languages: Vec<crate::project::Language>,
    pub targets: HashMap<String, crate::project::Target>,
    pub post: Vec<crate::project::PostCommand>,

    // -- Extra variables (custom key-value pairs) --
    pub var: HashMap<String, MetaValue>,
}

impl Metadata {
    /// Apply command-line overrides in "key=value" format.
    pub fn apply_overrides(&mut self, overrides: &[String]) {
        for item in overrides {
            // Support append syntax: "key+=value" appends to list fields
            if let Some((key, value)) = item.split_once("+=") {
                let key = key.trim();
                let value = value.trim();
                match key {
                    "bibliography" => {
                        self.bibliography.push(value.to_string());
                    }
                    _ => {}
                }
                continue;
            }
            let (raw_key, value) = match item.split_once('=') {
                Some((k, v)) => (k.trim(), v.trim()),
                None => continue,
            };
            let key = crate::util::normalize_key(raw_key);
            match key.as_str() {
                "title" => self.title = Some(value.to_string()),
                "subtitle" => self.subtitle = Some(value.to_string()),
                "author" => self.author = Some(vec![value.to_string()]),
                "date" => self.date = Some(value.to_string()),
                "abstract" => self.abstract_text = Some(value.to_string()),
                "target" | "format" => self.target = Some(value.to_string()),
                "number_sections" => self.number_sections = value::coerce_value(value).as_bool() == Some(true),
                "toc" => self.toc = Some(value::coerce_value(value).as_bool() == Some(true)),
                "bibliography" => self.bibliography = vec![value.to_string()],
                "csl" => self.csl = Some(value.to_string()),
                _ => {
                    // Support dot-notation for nested keys: "a.b.c=val"
                    let parts: Vec<&str> = key.split('.').collect();
                    let coerced = value::coerce_value(value);
                    if parts.len() == 1 {
                        self.var.insert(key.to_string(), coerced);
                    } else {
                        let leaf = coerced;
                        let nested = value::build_nested_value(&parts, leaf);
                        value::merge_value(&mut self.var, parts[0], nested);
                    }
                }
            }
        }
    }

    /// Resolve date keywords: `today`/`now` -> current date, `last-modified` -> file mtime.
    /// Applies `date-format` if set.
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

    /// Merge two Metadata values: `overlay` fields win over `self` (base).
    /// For Option fields, overlay replaces base when Some.
    /// For Vec fields, overlay replaces base when non-empty.
    /// For bool fields, overlay wins if it differs from default.
    pub fn merge(mut self, overlay: Metadata) -> Metadata {
        macro_rules! merge_opt {
            ($field:ident) => {
                if overlay.$field.is_some() { self.$field = overlay.$field; }
            };
        }
        macro_rules! merge_vec {
            ($field:ident) => {
                if !overlay.$field.is_empty() { self.$field = overlay.$field; }
            };
        }

        // Document identity
        merge_opt!(title);
        merge_opt!(subtitle);
        merge_opt!(author);
        if !overlay.authors.is_empty() { self.authors = overlay.authors; }
        if !overlay.affiliations.is_empty() { self.affiliations = overlay.affiliations; }
        merge_opt!(date);
        merge_opt!(abstract_text);
        merge_vec!(keywords);
        merge_opt!(copyright);
        merge_opt!(license);
        merge_opt!(citation);
        merge_vec!(funding);
        merge_opt!(appendix_style);

        // Rendering
        merge_opt!(target);
        merge_opt!(theme);
        if overlay.number_sections { self.number_sections = true; }
        merge_opt!(toc);
        if overlay.toc_depth != 0 { self.toc_depth = overlay.toc_depth; }
        merge_opt!(toc_title);
        merge_opt!(date_format);
        merge_vec!(bibliography);
        merge_opt!(csl);
        merge_vec!(plugins);
        if overlay.convert_math { self.convert_math = true; }
        merge_opt!(html_math_method);

        // Project-level
        merge_opt!(output);
        merge_opt!(lang);
        merge_opt!(url);
        merge_opt!(favicon);
        merge_opt!(logo);
        merge_opt!(logo_dark);
        merge_opt!(orchestrator);
        if overlay.global_crossref { self.global_crossref = true; }
        merge_vec!(static_dirs);
        merge_opt!(embed_resources);

        // Collection structure
        merge_vec!(contents);
        merge_vec!(languages);
        if !overlay.targets.is_empty() { self.targets = overlay.targets; }
        merge_vec!(post);

        // Extra variables: overlay keys win
        for (k, v) in overlay.var {
            self.var.insert(k, v);
        }

        self
    }
}

// ---------------------------------------------------------------------------
// Date helpers
// ---------------------------------------------------------------------------

/// Convert days since Unix epoch to YYYY-MM-DD string.
fn epoch_days_to_date(total_days: u64) -> String {
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

/// Format a YYYY-MM-DD date string with a strftime-style format string.
/// Supports: `%Y`, `%m`, `%d`, `%e`, `%B`, `%b`, `%A`, `%a`.
/// Returns the original string unchanged if parsing fails.
pub fn format_date_str(date: &str, fmt: &str) -> String {
    let parts: Vec<&str> = date.trim().split('-').collect();
    if parts.len() != 3 { return date.to_string(); }
    let (y, m, d) = match (parts[0].parse::<i64>(), parts[1].parse::<usize>(), parts[2].parse::<usize>()) {
        (Ok(y), Ok(m), Ok(d)) if m >= 1 && m <= 12 && d >= 1 && d <= 31 => (y, m, d),
        _ => return date.to_string(),
    };
    format_ymd(y, m, d, fmt)
}

fn format_date(secs: u64, fmt: &str) -> String {
    let days = secs / 86400;
    let ymd = epoch_days_to_date(days);
    let parts: Vec<&str> = ymd.split('-').collect();
    let (y, m, d) = (
        parts[0].parse::<i64>().unwrap_or(1970),
        parts[1].parse::<usize>().unwrap_or(1),
        parts[2].parse::<usize>().unwrap_or(1),
    );
    format_ymd(y, m, d, fmt)
}

fn format_ymd(y: i64, m: usize, d: usize, fmt: &str) -> String {
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

    static T: [usize; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let yy = if m < 3 { y - 1 } else { y } as usize;
    let dow = (yy + yy / 4 - yy / 100 + yy / 400 + T[m - 1] + d) % 7;

    fmt.replace("%Y", &format!("{:04}", y))
        .replace("%m", &format!("{:02}", m))
        .replace("%d", &format!("{:02}", d))
        .replace("%e", &d.to_string())
        .replace("%B", MONTHS[m - 1])
        .replace("%b", MONTHS_SHORT[m - 1])
        .replace("%A", DAYS[dow])
        .replace("%a", DAYS_SHORT[dow])
}
