//! Metadata types: Metadata struct, Author, Affiliation, Copyright, License, etc.

use std::collections::HashMap;

use serde::Deserialize;

use crate::value::Value as MetaValue;

// ---------------------------------------------------------------------------
// Defaults sub-types (rendering defaults for figures, code execution, etc.)
// ---------------------------------------------------------------------------

/// Default syntax highlighting theme configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct Highlight {
    /// Theme for light mode.
    pub light: Option<String>,
    /// Theme for dark mode.
    pub dark: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FigureConfig {
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub out_width: Option<f64>,
    pub out_height: Option<f64>,
    pub aspect_ratio: Option<f64>,
    pub device: Option<String>,
    pub alignment: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExecuteConfig {
    pub cache: Option<bool>,
    pub eval: Option<bool>,
    pub echo: Option<bool>,
    pub include: Option<bool>,
    pub warning: Option<bool>,
    pub message: Option<bool>,
    pub error: Option<bool>,
    pub comment: Option<String>,
    pub results: Option<String>,
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TocConfig {
    pub enabled: Option<bool>,
    pub depth: Option<u32>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CalloutConfig {
    pub appearance: Option<String>,
    pub note: Option<String>,
    pub tip: Option<String>,
    pub warning: Option<String>,
    pub important: Option<String>,
    pub caution: Option<String>,
    pub icon_note: Option<String>,
    pub icon_tip: Option<String>,
    pub icon_warning: Option<String>,
    pub icon_important: Option<String>,
    pub icon_caution: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct VideoConfig {
    pub width: Option<String>,
    pub height: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlaceholderConfig {
    pub width: Option<String>,
    pub height: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LipsumConfig {
    pub paragraphs: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LayoutConfig {
    pub valign: Option<String>,
    pub columns: Option<usize>,
    pub rows: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LatexConfig {
    pub documentclass: Option<String>,
    pub fontsize: Option<String>,
    pub linkcolor: Option<String>,
    pub urlcolor: Option<String>,
    pub citecolor: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TypstConfig {
    pub fontsize: Option<String>,
    pub leading: Option<String>,
    pub justify: Option<bool>,
    pub heading_numbering: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RevealJsConfig {
    pub theme: Option<String>,
    pub code_theme: Option<String>,
    pub transition: Option<String>,
    pub slide_number: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LabelsConfig {
    pub abstract_title: Option<String>,
    pub keywords: Option<String>,
    pub appendix: Option<String>,
    pub citation: Option<String>,
    pub reuse: Option<String>,
    pub funding: Option<String>,
    pub copyright: Option<String>,
    pub listing: Option<String>,
    pub proof: Option<String>,
    pub contents: Option<String>,
}


// ---------------------------------------------------------------------------
// Scholarly front matter: authors & affiliations
// ---------------------------------------------------------------------------

/// A parsed author name, split into components.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
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
pub struct CitationConfig {
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
    /// Author metadata. Each entry has at minimum a name.
    pub authors: Vec<Author>,
    /// Deduplicated affiliations referenced by authors.
    pub affiliations: Vec<Affiliation>,
    pub date: Option<String>,
    pub abstract_text: Option<String>,
    pub keywords: Vec<String>,
    pub copyright: Option<Copyright>,
    pub license: Option<License>,
    pub citation: Option<CitationConfig>,
    pub funding: Vec<Funding>,
    pub appendix_style: Option<String>,

    // -- Rendering --
    pub target: Option<String>,
    pub theme: Option<String>,
    pub number_sections: bool,
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

    // -- Rendering defaults (figures, code execution, etc.) --
    pub preview_port: Option<u16>,
    pub dpi: Option<f64>,
    pub math: Option<String>,
    pub highlight: Option<Highlight>,
    pub figure: Option<FigureConfig>,
    pub execute: Option<ExecuteConfig>,
    pub toc: Option<TocConfig>,
    pub callout: Option<CalloutConfig>,
    pub video: Option<VideoConfig>,
    pub placeholder: Option<PlaceholderConfig>,
    pub lipsum: Option<LipsumConfig>,
    pub layout: Option<LayoutConfig>,
    pub latex: Option<LatexConfig>,
    pub typst: Option<TypstConfig>,
    pub revealjs: Option<RevealJsConfig>,
    pub labels: Option<LabelsConfig>,

    // -- Collection structure --
    pub contents: Vec<crate::project::ContentSection>,
    pub languages: Vec<crate::project::LanguageConfig>,
    pub targets: HashMap<String, crate::project::Target>,
    pub post: Vec<crate::project::PostCommand>,

    // -- Extra variables (custom key-value pairs) --
    pub var: HashMap<String, MetaValue>,
}

impl Metadata {
    /// Author display names extracted from the structured author list.
    pub fn author_names(&self) -> Vec<&str> {
        self.authors.iter().map(|a| a.name.literal.as_str()).collect()
    }

    /// The default language code, or None if no languages are configured.
    pub fn default_language(&self) -> Option<&str> {
        if self.languages.is_empty() {
            return None;
        }
        self.languages.iter()
            .find(|l| l.default)
            .or(self.languages.first())
            .map(|l| l.abbreviation.as_str())
    }

    /// Resolve date keywords (`today`, `now`, `last-modified`) to actual dates.
    pub fn resolve_date(&mut self, input_path: Option<&std::path::Path>) {
        if let Some(ref date) = self.date {
            if let Some(resolved) = crate::date::resolve_date(date, self.date_format.as_deref(), input_path) {
                self.date = Some(resolved);
            }
        }
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
        for author in self.authors.iter_mut() {
            if author.name.literal.contains("`{") {
                if let Ok(result) = crate::engines::inline::evaluate_inline(&author.name.literal, ctx) {
                    author.name.literal = result;
                }
            }
        }
    }

}
