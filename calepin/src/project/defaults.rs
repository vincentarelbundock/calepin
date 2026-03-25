//! Defaults merging and thread-local state management.

use std::cell::RefCell;
use serde::Deserialize;
use super::HighlightDefaults;

// ---------------------------------------------------------------------------
// Defaults types
// ---------------------------------------------------------------------------

/// Resolved defaults, stored in thread-local for the current render.
/// Fields are kept flat (no nesting) for easy access by consumers.
/// The TOML structure groups some of these under [shortcodes] and [formats],
/// but `ProjectConfig::as_defaults()` flattens them back.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Defaults {
    pub format: Option<String>,
    pub lang: Option<String>,
    #[serde(alias = "preview-port")]
    pub preview_port: Option<u16>,
    pub csl: Option<String>,
    pub dpi: Option<f64>,
    pub timeout: Option<u64>,
    pub math: Option<String>,
    pub highlight: Option<HighlightDefaults>,
    pub figure: Option<FigureDefaults>,
    pub execute: Option<ExecuteDefaults>,
    pub toc: Option<TocDefaults>,
    pub callout: Option<CalloutDefaults>,
    pub video: Option<VideoDefaults>,
    pub placeholder: Option<PlaceholderDefaults>,
    pub lipsum: Option<LipsumDefaults>,
    pub layout: Option<LayoutDefaults>,
    pub latex: Option<LatexDefaults>,
    pub typst: Option<TypstDefaults>,
    pub revealjs: Option<RevealJsDefaults>,
    pub labels: Option<LabelsDefaults>,
    #[serde(alias = "embed-resources")]
    pub embed_resources: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FigureDefaults {
    pub width: Option<f64>,
    #[serde(alias = "out-width")]
    pub out_width: Option<f64>,
    #[serde(alias = "aspect-ratio")]
    pub aspect_ratio: Option<f64>,
    pub device: Option<String>,
    pub alignment: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExecuteDefaults {
    pub cache: Option<bool>,
    pub eval: Option<bool>,
    pub echo: Option<bool>,
    pub include: Option<bool>,
    pub warning: Option<bool>,
    pub message: Option<bool>,
    pub comment: Option<String>,
    pub results: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TocDefaults {
    pub enabled: Option<bool>,
    pub depth: Option<u32>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CalloutDefaults {
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
pub struct VideoDefaults {
    pub width: Option<String>,
    pub height: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlaceholderDefaults {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LipsumDefaults {
    pub paragraphs: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LayoutDefaults {
    pub valign: Option<String>,
    pub columns: Option<usize>,
    pub rows: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LatexDefaults {
    pub documentclass: Option<String>,
    pub fontsize: Option<String>,
    pub linkcolor: Option<String>,
    pub urlcolor: Option<String>,
    pub citecolor: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TypstDefaults {
    pub fontsize: Option<String>,
    pub leading: Option<String>,
    pub justify: Option<bool>,
    #[serde(alias = "heading-numbering")]
    pub heading_numbering: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RevealJsDefaults {
    pub theme: Option<String>,
    #[serde(alias = "code-theme")]
    pub code_theme: Option<String>,
    pub transition: Option<String>,
    #[serde(alias = "slide-number")]
    pub slide_number: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LabelsDefaults {
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

/// Wrapper for [shortcodes] TOML section. Flattened into Defaults by `as_defaults()`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ShortcodesConfig {
    pub video: Option<VideoDefaults>,
    pub placeholder: Option<PlaceholderDefaults>,
    pub lipsum: Option<LipsumDefaults>,
}

/// Wrapper for [formats] TOML section. Flattened into Defaults by `as_defaults()`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FormatsConfig {
    pub latex: Option<LatexDefaults>,
    pub typst: Option<TypstDefaults>,
    pub revealjs: Option<RevealJsDefaults>,
}

// ---------------------------------------------------------------------------
// Defaults merging
// ---------------------------------------------------------------------------

/// Merge two `Option<T>` where `Some(inner)` structs are merged field-by-field.
macro_rules! merge_option_struct {
    ($user:expr, $builtin:expr, { $( $field:ident ),* $(,)? }) => {
        match (&$user, &$builtin) {
            (Some(u), Some(b)) => Some({
                let mut merged = b.clone();
                $(
                    if u.$field.is_some() {
                        merged.$field = u.$field.clone();
                    }
                )*
                merged
            }),
            (Some(u), None) => Some(u.clone()),
            (None, b) => b.clone(),
        }
    };
}

impl Defaults {
    /// Merge user defaults on top of built-in defaults.
    /// User values take priority; missing fields fall back to built-in.
    pub fn merge(builtin: &Defaults, user: &Defaults) -> Defaults {
        macro_rules! or {
            ($u:expr, $b:expr) => { $u.clone().or_else(|| $b.clone()) };
        }
        Defaults {
            format: or!(user.format, builtin.format),
            lang: or!(user.lang, builtin.lang),
            preview_port: user.preview_port.or(builtin.preview_port),
            csl: or!(user.csl, builtin.csl),
            dpi: user.dpi.or(builtin.dpi),
            timeout: user.timeout.or(builtin.timeout),
            math: or!(user.math, builtin.math),
            embed_resources: user.embed_resources.or(builtin.embed_resources),
            highlight: merge_option_struct!(user.highlight, builtin.highlight, { light, dark }),
            figure: merge_option_struct!(user.figure, builtin.figure, { width, out_width, aspect_ratio, device, alignment }),
            execute: merge_option_struct!(user.execute, builtin.execute, { cache, eval, echo, include, warning, message, comment, results }),
            toc: merge_option_struct!(user.toc, builtin.toc, { enabled, depth, title }),
            callout: merge_option_struct!(user.callout, builtin.callout, { appearance, note, tip, warning, important, caution, icon_note, icon_tip, icon_warning, icon_important, icon_caution }),
            video: merge_option_struct!(user.video, builtin.video, { width, height, title }),
            placeholder: merge_option_struct!(user.placeholder, builtin.placeholder, { width, height, color }),
            lipsum: merge_option_struct!(user.lipsum, builtin.lipsum, { paragraphs }),
            layout: merge_option_struct!(user.layout, builtin.layout, { valign, columns, rows }),
            latex: merge_option_struct!(user.latex, builtin.latex, { documentclass, fontsize, linkcolor, urlcolor, citecolor }),
            typst: merge_option_struct!(user.typst, builtin.typst, { fontsize, leading, justify, heading_numbering }),
            revealjs: merge_option_struct!(user.revealjs, builtin.revealjs, { theme, code_theme, transition, slide_number }),
            labels: merge_option_struct!(user.labels, builtin.labels, { abstract_title, keywords, appendix, citation, reuse, funding, copyright, listing, proof, contents }),
        }
    }
}

// ---------------------------------------------------------------------------
// Thread-local defaults state
// ---------------------------------------------------------------------------

// Thread-local resolved defaults, set once per render.
thread_local! {
    static ACTIVE_DEFAULTS: RefCell<Option<Defaults>> = RefCell::new(None);
}

/// Set the active defaults for the current render.
pub fn set_active_defaults(defaults: Defaults) {
    ACTIVE_DEFAULTS.with(|d| *d.borrow_mut() = Some(defaults));
}

/// Get a reference to the active defaults. Falls back to built-in if not set.
pub fn get_defaults() -> Defaults {
    ACTIVE_DEFAULTS.with(|d| {
        d.borrow().clone().unwrap_or_else(|| {
            super::builtin_config().as_defaults()
        })
    })
}

/// Get resolved defaults, merging user defaults with built-in.
pub fn resolve_defaults(user: Option<&Defaults>) -> Defaults {
    let builtin = super::builtin_config().as_defaults();
    match user {
        Some(user) => Defaults::merge(&builtin, user),
        None => builtin,
    }
}
