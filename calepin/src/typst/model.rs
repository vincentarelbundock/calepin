use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

pub const RESULT_SCHEMA_VERSION: u8 = 1;

// Copy is not derived: Jupyter(String) is not Copy.
// Serde is implemented manually so Jupyter("julia") serializes as "julia".
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EngineName {
    R,
    Python,
    Julia,
    Sh,
    Mermaid,
    Tikz,
    Dot,
    D2,
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
        Ok(match s.as_str() {
            "r" => Self::R,
            "python" => Self::Python,
            "julia" => Self::Julia,
            "sh" | "bash" => Self::Sh,
            "mermaid" => Self::Mermaid,
            "tikz" => Self::Tikz,
            "dot" => Self::Dot,
            "d2" => Self::D2,
            _ => Self::Jupyter(s),
        })
    }
}

impl EngineName {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        Ok(match value {
            "r" => Self::R,
            "python" => Self::Python,
            "julia" => Self::Julia,
            "sh" | "bash" => Self::Sh,
            "mermaid" => Self::Mermaid,
            "tikz" => Self::Tikz,
            "dot" => Self::Dot,
            "d2" => Self::D2,
            other => Self::Jupyter(other.to_string()),
        })
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::R => "r",
            Self::Python => "python",
            Self::Julia => "julia",
            Self::Sh => "sh",
            Self::Mermaid => "mermaid",
            Self::Tikz => "tikz",
            Self::Dot => "dot",
            Self::D2 => "d2",
            Self::Jupyter(name) => name.as_str(),
        }
    }

    pub fn is_diagram(&self) -> bool {
        matches!(self, Self::Mermaid | Self::Tikz | Self::Dot | Self::D2)
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
    Asis,
    Hide,
}

impl ResultsMode {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            "verbatim" | "markup" => Ok(Self::Verbatim),
            "asis" => Ok(Self::Asis),
            "hide" => Ok(Self::Hide),
            other => Err(anyhow::anyhow!("unsupported results mode `{}`", other)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ItemSelector {
    Named(ItemSelectorName),
    Index(isize),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemSelectorName {
    All,
    First,
    Last,
}

impl ItemSelector {
    pub const ALL: Self = Self::Named(ItemSelectorName::All);
    pub const FIRST: Self = Self::Named(ItemSelectorName::First);
    pub const LAST: Self = Self::Named(ItemSelectorName::Last);

    pub fn parse(value: &Value) -> anyhow::Result<Self> {
        if let Some(n) = value.as_i64() {
            return Ok(Self::Index(n as isize));
        }
        let Some(s) = value.as_str() else {
            return Err(anyhow::anyhow!(
                "item must be `all`, `first`, `last`, or an integer"
            ));
        };
        match s {
            "all" => Ok(Self::ALL),
            "first" => Ok(Self::FIRST),
            "last" => Ok(Self::LAST),
            other => Err(anyhow::anyhow!("unsupported item selector `{}`", other)),
        }
    }
}

/// Which plain fenced blocks auto-run as chunks: none, every engine, or a
/// specific set of engine names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RawChunks {
    Off,
    All,
    Only(Vec<String>),
}

impl RawChunks {
    pub fn allows(&self, lang: &str) -> bool {
        match self {
            RawChunks::Off => false,
            RawChunks::All => true,
            RawChunks::Only(langs) => langs.iter().any(|l| l == lang),
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
    pub format: Vec<String>,
    pub item: ItemSelector,
    pub placeholder: bool,
    pub fig_device_format: String,
    pub fig_device_dpi: u32,
    pub fig_device_width: f64,
    pub fig_device_height: Option<f64>,
    pub fig_device_aspect: f64,
    pub fig_display_width: Option<Value>,
    pub fig_display_align: Option<Value>,
    pub fig_display_responsive: Option<bool>,
    pub raw_chunks: RawChunks,
}

impl Default for SetupDefaults {
    fn default() -> Self {
        Self {
            echo: true,
            eval: true,
            output: true,
            results: "verbatim".to_string(),
            warning: true,
            message: true,
            error: false,
            format: default_format_order(),
            item: ItemSelector::ALL,
            placeholder: true,
            fig_device_format: "svg".to_string(),
            fig_device_dpi: 150,
            fig_device_width: 6.0,
            fig_device_height: None,
            fig_device_aspect: 0.618,
            fig_display_width: Some(Value::String("70%".to_string())),
            fig_display_align: Some(Value::String("center".to_string())),
            fig_display_responsive: Some(true),
            raw_chunks: RawChunks::Off,
        }
    }
}

pub fn default_format_order() -> Vec<String> {
    vec![
        "image/svg+xml".to_string(),
        "image/png".to_string(),
        "text/x-typst".to_string(),
        "text/plain".to_string(),
        "application/json".to_string(),
    ]
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
pub struct DisplayOptions {
    pub echo: bool,
    pub output: bool,
    pub results: ResultsMode,
    pub warning: bool,
    pub message: bool,
    pub format: Vec<String>,
    pub item: ItemSelector,
    pub placeholder: bool,
    pub fig_display_width: Option<Value>,
    pub fig_display_height: Option<Value>,
    pub fig_display_align: Option<Value>,
    pub fig_display_responsive: Option<bool>,
    pub fig_display_link: Option<Value>,
    pub fig_caption: Option<String>,
    pub fig_caption_position: Option<Value>,
    pub fig_alt_text: Option<String>,
    pub fig_subcaptions: Option<Vec<String>>,
    pub fig_layout_columns: Option<Value>,
    pub fig_layout_rows: Option<Value>,
    pub fig_layout_design: Option<Value>,
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

pub type MimeData = IndexMap<String, Value>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkResultDocument {
    pub label: String,
    pub engine: EngineName,
    pub status: ChunkStatus,
    pub items: Vec<ResultItem>,
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
    pub results_path: PathBuf,
    pub figures_dir: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

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
