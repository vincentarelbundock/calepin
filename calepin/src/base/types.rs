use std::collections::HashMap;
use std::path::PathBuf;

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
    /// Raw `#| key: value` lines (without the `#| ` prefix), for `echo: fenced`
    pub pipe_comments: Vec<String>,
}

/// Chunk options stored as a string-keyed map with typed access.
/// Keys in `defaults_keys` came from front matter / TOML defaults (not chunk-level `#|`).
#[derive(Debug, Clone, Default)]
pub struct ChunkOptions {
    pub inner: HashMap<String, OptionValue>,
    /// Keys that were merged from document-level defaults (not set per-chunk).
    pub defaults_keys: std::collections::HashSet<String>,
    /// Resolved rendering metadata for fallback values.
    pub metadata: crate::metadata::Metadata,
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
    pub fn cache(&self) -> bool {
        let d = self.metadata.execute.as_ref().and_then(|c| c.cache).unwrap_or(true);
        self.get_bool("cache", d)
    }
    pub fn eval(&self) -> bool {
        let d = self.metadata.execute.as_ref().and_then(|c| c.eval).unwrap_or(true);
        self.get_bool("eval", d)
    }
    #[allow(dead_code)]
    pub fn echo(&self) -> bool {
        let d = self.metadata.execute.as_ref().and_then(|c| c.echo).unwrap_or(true);
        self.get_bool("echo", d)
    }
    pub fn include(&self) -> bool {
        let d = self.metadata.execute.as_ref().and_then(|c| c.include).unwrap_or(true);
        self.get_bool("include", d)
    }
    pub fn warning(&self) -> bool {
        let d = self.metadata.execute.as_ref().and_then(|c| c.warning).unwrap_or(true);
        self.get_bool("warning", d)
    }
    pub fn message(&self) -> bool {
        let d = self.metadata.execute.as_ref().and_then(|c| c.message).unwrap_or(true);
        self.get_bool("message", d)
    }
    pub fn comment(&self) -> String {
        let d = self.metadata.execute.as_ref().and_then(|c| c.comment.clone()).unwrap_or_else(|| "> ".to_string());
        self.get_string("comment", &d)
    }
    pub fn results(&self) -> ResultsMode {
        let d = self.metadata.execute.as_ref().and_then(|c| c.results.clone()).unwrap_or_else(|| "markup".to_string());
        match self.get_string("results", &d).as_str() {
            "asis" => ResultsMode::Asis,
            "hide" => ResultsMode::Hide,
            _ => ResultsMode::Markup,
        }
    }
    pub fn engine(&self) -> String {
        self.get_opt_string("engine")
            .expect("engine must be set by the parser (e.g., {r} or {python})")
    }

    fn default_fig_width(&self) -> f64 {
        self.metadata.figure.as_ref().and_then(|f| f.fig_width).unwrap_or(6.0)
    }
    fn default_out_width_frac(&self) -> f64 {
        self.metadata.figure.as_ref().and_then(|f| f.out_width).unwrap_or(0.70)
    }
    fn default_fig_asp(&self) -> f64 {
        self.metadata.figure.as_ref().and_then(|f| f.fig_asp).unwrap_or(0.618)
    }

    /// Graphics device width in inches.
    /// When `out-width` is set but `fig-width` is not set per-chunk, auto-scales
    /// to keep text size consistent: `default_fig_width * (out_width / default_out_width)`.
    /// Document-level defaults (from TOML/front matter) don't suppress auto-scaling.
    pub fn fig_width(&self) -> f64 {
        let fig_width_set_per_chunk = self.get_opt_string("fig_width").is_some()
            && !self.defaults_keys.contains("fig_width");
        if fig_width_set_per_chunk {
            return self.get_number("fig_width", self.default_fig_width());
        }
        // Auto-scale from out-width if set (per-chunk out-width takes priority)
        if let Some(frac) = self.out_width_fraction() {
            let base = self.get_number("fig_width", self.default_fig_width());
            return base * (frac / self.default_out_width_frac());
        }
        self.get_number("fig_width", self.default_fig_width())
    }

    /// Graphics device height in inches.
    /// Derived from `fig-width * fig-asp` unless explicitly set.
    pub fn fig_height(&self) -> f64 {
        if self.get_opt_string("fig_height").is_some() {
            return self.get_number("fig_height", self.default_fig_width() * self.default_fig_asp());
        }
        self.fig_width() * self.fig_asp()
    }

    /// Aspect ratio (height / width). Defaults to golden ratio.
    pub fn fig_asp(&self) -> f64 {
        self.get_number("fig_asp", self.default_fig_asp())
    }

    /// Parse out-width as a fraction (e.g., "70%" -> 0.70, "0.5" -> 0.5).
    /// Returns None if out-width is not set or not a percentage/fraction.
    fn out_width_fraction(&self) -> Option<f64> {
        let s = self.get_opt_string("out_width")?;
        if let Some(pct) = s.strip_suffix('%') {
            pct.trim().parse::<f64>().ok().map(|v| v / 100.0)
        } else {
            let v = s.trim().parse::<f64>().ok()?;
            if v > 0.0 && v <= 1.0 { Some(v) } else { None }
        }
    }
    pub fn fig_cap(&self) -> Option<String> { self.get_opt_string("fig_cap") }
    pub fn tbl_cap(&self) -> Option<String> { self.get_opt_string("tbl_cap") }
    pub fn lst_cap(&self) -> Option<String> { self.get_opt_string("lst_cap") }
    pub fn fig_alt(&self) -> Option<String> { self.get_opt_string("fig_alt") }
    pub fn dev(&self) -> String {
        let default = self.metadata.figure.as_ref().and_then(|f| f.device.clone()).unwrap_or_else(|| "png".to_string());
        self.get_string("dev", &default)
    }
    pub fn fig_align(&self) -> Option<String> { self.get_opt_string("fig_align") }
    pub fn fig_scap(&self) -> Option<String> { self.get_opt_string("fig_scap") }
    pub fn fig_env(&self) -> Option<String> { self.get_opt_string("fig_env") }
    pub fn fig_pos(&self) -> Option<String> { self.get_opt_string("fig_pos") }
    pub fn fig_cap_location(&self) -> Option<String> { self.get_opt_string("fig_cap_location") }
    pub fn out_width(&self) -> Option<String> { self.get_opt_string("out_width") }
    pub fn out_height(&self) -> Option<String> { self.get_opt_string("out_height") }
    pub fn fig_link(&self) -> Option<String> { self.get_opt_string("fig_link") }

    /// Build figure rendering attributes from chunk options.
    pub fn to_figure_attrs(&self) -> FigureAttrs {
        let default_out_width = format!("{}%", (self.default_out_width_frac() * 100.0) as u32);
        FigureAttrs {
            width: self.out_width().or_else(|| Some(default_out_width)),
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
    /// Content to inject into the document preamble (e.g. \usepackage lines from knitr::knit_meta).
    Preamble(String),
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
    CodeSource { code: String, lang: String, label: String, filename: String, lst_cap: Option<String> },
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

