use anyhow::{anyhow, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

pub const RESULT_SCHEMA_VERSION: u8 = 1;

// Copy is not derived: Jupyter(String) is not Copy.
// Serde is implemented manually so Jupyter("julia") serializes as "julia".
// Neither "julia" nor "sh"/"bash" have named variants: they are all routed
// through the Jupyter bridge like any other third-party kernel.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EngineName {
    R,
    Python,
    Diagram(String),
    Jupyter(String),
}

impl serde::Serialize for EngineName {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for EngineName {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(Self::from_name(&s))
    }
}

impl EngineName {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        Ok(Self::from_name(value))
    }

    fn from_name(value: &str) -> Self {
        match value {
            "r" => Self::R,
            "python" => Self::Python,
            name if crate::engines::diagram::is_known_diagram_engine_name(name) => {
                Self::Diagram(name.to_string())
            }
            other => Self::Jupyter(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::R => "r",
            Self::Python => "python",
            Self::Diagram(name) => name.as_str(),
            Self::Jupyter(name) => name.as_str(),
        }
    }

    pub fn is_diagram(&self) -> bool {
        matches!(self, Self::Diagram(_))
    }
}

impl fmt::Display for EngineName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResultsMode {
    Verbatim,
    Render,
    Typst,
    Hide,
}

impl ResultsMode {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            "verbatim" => Ok(Self::Verbatim),
            "render" => Ok(Self::Render),
            "typst" => Ok(Self::Typst),
            "hide" => Ok(Self::Hide),
            other => Err(anyhow::anyhow!("unsupported results mode `{}`", other)),
        }
    }
}

/// Which plain fenced blocks auto-run as chunks: none, every engine, or a
/// specific set of engine names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FencedChunks {
    Off,
    All,
    Only(Vec<String>),
}

impl FencedChunks {
    pub fn allows(&self, lang: &str) -> bool {
        match self {
            FencedChunks::Off => false,
            FencedChunks::All => true,
            FencedChunks::Only(langs) => langs.iter().any(|l| l == lang),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetupDefaults {
    pub echo: bool,
    pub eval: bool,
    pub output: bool,
    pub results: String,
    pub warning: bool,
    pub message: bool,
    pub error: bool,
    pub placeholder: bool,
    pub fig_device_format: String,
    pub fig_device_dpi: u32,
    pub fig_device_width: f64,
    pub fig_device_height: Option<f64>,
    pub fig_device_aspect: f64,
    pub fig_width: Option<Value>,
    pub fig_align: Option<Value>,
    pub fig_responsive: Option<bool>,
    pub fenced_chunks: FencedChunks,
    /// Document-level parameters from `calepin.setup(params: (...))`, kept as a
    /// JSON object. Injected once per engine so chunks can read a `params` value.
    #[serde(default)]
    pub params: Value,
    /// Document-level theme from `calepin.setup(theme: ...)`: a builtin name, a
    /// project-relative path, or `false` to disable.
    #[serde(default)]
    pub theme: Option<Value>,
}

impl Default for SetupDefaults {
    fn default() -> Self {
        Self {
            echo: true,
            eval: true,
            output: true,
            results: "render".to_string(),
            warning: true,
            message: true,
            error: false,
            placeholder: true,
            fig_device_format: "svg".to_string(),
            fig_device_dpi: 150,
            fig_device_width: 6.0,
            fig_device_height: None,
            fig_device_aspect: 0.618,
            fig_width: Some(Value::String("70%".to_string())),
            fig_align: Some(Value::String("center".to_string())),
            fig_responsive: Some(true),
            fenced_chunks: FencedChunks::All,
            params: Value::Object(serde_json::Map::new()),
            theme: None,
        }
    }
}

impl SetupDefaults {
    pub fn theme_selection(&self, root: &Path) -> Result<Option<crate::theme::ThemeSelection>> {
        match &self.theme {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Bool(false)) => Ok(Some(crate::theme::ThemeSelection::Disabled)),
            Some(Value::Bool(true)) => Ok(Some(crate::theme::ThemeSelection::Default)),
            Some(Value::String(value)) => {
                Ok(Some(crate::theme::ThemeSelection::parse(value, root)?))
            }
            Some(other) => Err(anyhow!("invalid setup theme value: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecOptions {
    pub eval: bool,
    pub error: bool,
    pub fig_device_format: String,
    pub fig_device_dpi: u32,
    pub fig_device_width: f64,
    pub fig_device_height: Option<f64>,
    pub fig_device_aspect: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DisplayOptions {
    pub echo: bool,
    pub output: bool,
    pub results: ResultsMode,
    pub warning: bool,
    pub message: bool,
    pub placeholder: bool,
    pub fig_width: Option<Value>,
    pub fig_height: Option<Value>,
    pub fig_align: Option<Value>,
    pub fig_responsive: Option<bool>,
    pub fig_link: Option<Value>,
    pub fig_caption: Option<String>,
    pub fig_cap_location: Option<Value>,
    pub fig_alt_text: Option<String>,
    pub fig_subcaptions: Option<Vec<String>>,
    pub fig_layout_columns: Option<Value>,
    pub fig_layout_rows: Option<Value>,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkSpec {
    pub label: String,
    pub engine: EngineName,
    pub code: String,
    pub exec_options: ExecOptions,
    pub display_options: DisplayOptions,
    pub ordinal: usize,
    #[serde(default)]
    pub crossref_labels: Vec<CrossrefLabelDoc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FigureSpec {
    pub format: String,
    pub dpi: u32,
    pub width: f64,
    pub height: f64,
}

impl FigureSpec {
    pub fn from_exec_options(engine: EngineName, options: &ExecOptions) -> Self {
        let format = if engine.is_diagram() {
            "svg".to_string()
        } else {
            options.fig_device_format.clone()
        };
        let height = options
            .fig_device_height
            .unwrap_or(options.fig_device_width * options.fig_device_aspect);
        Self {
            format,
            dpi: options.fig_device_dpi,
            width: options.fig_device_width,
            height,
        }
    }

    pub fn extension(&self) -> &'static str {
        match self.format.as_str() {
            "png" => "png",
            "jpeg" | "jpg" => "jpg",
            "pdf" | "cairo_pdf" => "pdf",
            _ => "svg",
        }
    }

    pub fn mime_type(&self) -> &'static str {
        match self.extension() {
            "png" => "image/png",
            "jpg" => "image/jpeg",
            "pdf" => "application/pdf",
            _ => "image/svg+xml",
        }
    }

    pub fn r_device(&self) -> &str {
        match self.format.as_str() {
            "pdf" => "cairo_pdf",
            "jpg" => "jpeg",
            value => value,
        }
    }

    pub fn numbered_filename(&self, label: &str) -> String {
        format!("{}-1.{}", label, self.extension())
    }

    pub fn artifact_filename(&self, label: &str) -> String {
        format!("{}.{}", label, self.extension())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResultItemType {
    Stream,
    Diagnostic,
    Error,
    Display,
    Result,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResultItemName {
    Stdout,
    Stderr,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticLevel {
    Warning,
    Message,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultItem {
    #[serde(rename = "type")]
    pub item_type: ResultItemType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<ResultItemName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<DiagnosticLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traceback: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<MimeData>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, Value>,
}

impl Default for ResultItem {
    fn default() -> Self {
        Self {
            item_type: ResultItemType::Display,
            name: None,
            text: None,
            level: None,
            message: None,
            traceback: None,
            data: None,
            metadata: BTreeMap::new(),
        }
    }
}

pub type MimeData = IndexMap<String, Value>;

/// Serialized form of a routed cross-reference label, written into results.json
/// and read back by the Typst runtime. `kind` is one of "fig" | "tbl" | "lst".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossrefLabelDoc {
    pub kind: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkResultDocument {
    pub label: String,
    pub engine: EngineName,
    pub status: ChunkStatus,
    #[serde(rename = "options")]
    pub display_options: DisplayOptions,
    pub items: Vec<ResultItem>,
    #[serde(
        rename = "crossref-labels",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub crossref_labels: Vec<CrossrefLabelDoc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChunkStatus {
    Ok,
    Error,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultsDocument {
    pub schema: u8,
    pub calepin_version: String,
    pub input: String,
    pub chunks: IndexMap<String, ChunkResultDocument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutPaths {
    pub root: PathBuf,
    pub input: PathBuf,
    pub input_rel: PathBuf,
    pub render_input: PathBuf,
    pub work_dir: PathBuf,
    pub artifact_dir: PathBuf,
    pub results_path: PathBuf,
    pub figures_dir: PathBuf,
}

impl LayoutPaths {
    pub fn artifact_path(&self, name: impl AsRef<Path>) -> PathBuf {
        self.artifact_dir.join(name)
    }

    pub fn artifact_relative_path(&self, name: impl AsRef<Path>) -> PathBuf {
        let path = self.artifact_path(name);
        path.strip_prefix(&self.root)
            .map(Path::to_path_buf)
            .unwrap_or(path)
    }

    pub fn sibling_path(&self, name: impl AsRef<Path>) -> PathBuf {
        self.artifact_path(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_result_document_serializes_crossref_labels() {
        let doc = ChunkResultDocument {
            label: "fig-x".to_string(),
            engine: EngineName::R,
            status: ChunkStatus::Ok,
            display_options: serde_json::from_str(
                r#"{"echo":true,"output":true,"results":"render","warning":true,
                    "message":true,"placeholder":true,"fig-width":null,"fig-height":null,
                    "fig-align":null,"fig-responsive":null,"fig-link":null,"fig-caption":null,
                    "fig-cap-location":null,"fig-alt-text":null,"fig-subcaptions":null,
                    "fig-layout-columns":null,"fig-layout-rows":null,"kind":null}"#,
            )
            .unwrap(),
            items: vec![],
            crossref_labels: vec![CrossrefLabelDoc {
                kind: "fig".to_string(),
                name: "fig-x".to_string(),
            }],
        };
        let json = serde_json::to_string(&doc).unwrap();
        assert!(json.contains(r#""crossref-labels""#), "{json}");
        assert!(json.contains(r#""fig-x""#), "{json}");
    }

    #[test]
    fn parses_typed_diagram_engines() {
        for name in ["mermaid", "tikz", "dot", "d2"] {
            let engine = EngineName::parse(name).unwrap();
            assert_eq!(engine.as_str(), name);
        }
    }

    #[test]
    fn jupyter_engine_roundtrips_as_string() {
        let engine = EngineName::Jupyter("octave".to_string());
        let json = serde_json::to_string(&engine).unwrap();
        assert_eq!(json, r#""octave""#);
        let back: EngineName = serde_json::from_str(&json).unwrap();
        assert_eq!(back, engine);
    }

    #[test]
    fn unknown_engine_name_parses_as_jupyter() {
        let engine = EngineName::parse("octave").unwrap();
        assert_eq!(engine, EngineName::Jupyter("octave".to_string()));
        assert_eq!(engine.as_str(), "octave");
        assert!(!engine.is_diagram());
    }
}
