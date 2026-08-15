use anyhow::{anyhow, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::typst::paths::ENTRY_FILE_NAMES;

pub const RESULT_SCHEMA_VERSION: u8 = 2;
pub const DEFAULT_FIG_DEVICE_FORMAT: &str = "svg";
pub const DEFAULT_FIG_DEVICE_DPI: u32 = 150;
pub const DEFAULT_FIG_DEVICE_WIDTH: f64 = 6.0;
pub const DEFAULT_FIG_DEVICE_HEIGHT: Option<f64> = None;
pub const DEFAULT_FIG_DEVICE_ASPECT: f64 = 0.618;

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
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for EngineName {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(Self::from_name(&s))
    }
}

impl EngineName {
    pub fn parse(value: &str) -> Result<Self> {
        let value = value.trim();
        if value.is_empty() {
            return Err(anyhow!("engine name cannot be empty"));
        }
        Ok(Self::from_name(value))
    }

    pub fn from_name(value: &str) -> Self {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
            "hide" | "hidden" => Ok(Self::Hide),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScriptDestination {
    Enabled(bool),
    Path(String),
}

impl Default for ScriptDestination {
    fn default() -> Self {
        Self::Enabled(true)
    }
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
    #[serde(default)]
    pub script: ScriptDestination,
    pub echo: bool,
    pub eval: bool,
    pub output: bool,
    pub results: ResultsMode,
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
    #[serde(default)]
    pub fig_height: Option<Value>,
    pub fig_align: Option<Value>,
    pub fig_responsive: Option<bool>,
    #[serde(default)]
    pub fig_link: Option<Value>,
    #[serde(default)]
    pub fig_caption: Option<String>,
    #[serde(default)]
    pub fig_cap_location: Option<Value>,
    #[serde(default)]
    pub fig_alt_text: Option<String>,
    #[serde(default)]
    pub fig_subcaptions: Option<Vec<String>>,
    #[serde(default)]
    pub fig_layout_columns: Option<Value>,
    #[serde(default)]
    pub fig_layout_rows: Option<Value>,
    #[serde(default)]
    pub tbl_caption: Option<String>,
    #[serde(default)]
    pub lst_caption: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    pub fenced_chunks: FencedChunks,
    /// Document-level theme from `calepin.setup(theme: ...)`: a builtin name, a
    /// project-relative path, or `"typst"` for raw Typst output.
    #[serde(default)]
    pub theme: Option<Value>,
}

impl Default for SetupDefaults {
    fn default() -> Self {
        Self {
            script: ScriptDestination::default(),
            echo: true,
            eval: true,
            output: true,
            results: ResultsMode::Render,
            warning: true,
            message: true,
            error: false,
            placeholder: true,
            fig_device_format: DEFAULT_FIG_DEVICE_FORMAT.to_string(),
            fig_device_dpi: DEFAULT_FIG_DEVICE_DPI,
            fig_device_width: DEFAULT_FIG_DEVICE_WIDTH,
            fig_device_height: DEFAULT_FIG_DEVICE_HEIGHT,
            fig_device_aspect: DEFAULT_FIG_DEVICE_ASPECT,
            fig_width: Some(Value::String("70%".to_string())),
            fig_height: None,
            fig_align: Some(Value::String("center".to_string())),
            fig_responsive: Some(true),
            fig_link: None,
            fig_caption: None,
            fig_cap_location: None,
            fig_alt_text: None,
            fig_subcaptions: None,
            fig_layout_columns: None,
            fig_layout_rows: None,
            tbl_caption: None,
            lst_caption: None,
            kind: None,
            fenced_chunks: FencedChunks::All,
            theme: None,
        }
    }
}

impl SetupDefaults {
    pub fn theme_selection(&self, root: &Path) -> Result<Option<crate::theme::ThemeSelection>> {
        match &self.theme {
            None | Some(Value::Null) => Ok(None),
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
    #[serde(default, rename = "store-get")]
    pub store_get: Vec<String>,
    #[serde(default, rename = "store-set")]
    pub store_set: Vec<String>,
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
    /// Caption for a `tbl-` anchored chunk: the chunk's non-image output.
    pub tbl_caption: Option<String>,
    /// Caption for an `lst-` anchored chunk: the chunk's echoed source.
    pub lst_caption: Option<String>,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkSpec {
    pub label: String,
    pub engine: EngineName,
    pub code: String,
    #[serde(default)]
    pub script: ScriptDestination,
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

/// Encodes a semantic chunk label as one collision-resistant filename stem.
/// Labels remain unchanged in results and cross-reference metadata.
pub(crate) fn artifact_label_stem(label: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    let mut stem = String::with_capacity(label.len());
    for byte in label.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_') {
            stem.push(char::from(byte));
        } else {
            stem.push('%');
            stem.push(char::from(HEX[(byte >> 4) as usize]));
            stem.push(char::from(HEX[(byte & 0x0f) as usize]));
        }
    }
    stem
}

impl FigureSpec {
    pub fn from_exec_options(engine: &EngineName, options: &ExecOptions) -> Result<Self> {
        let format = if engine.is_diagram() {
            "svg".to_string()
        } else {
            options.fig_device_format.clone()
        };
        validate_figure_format(&format)?;
        validate_figure_dimension("fig-device-width", options.fig_device_width)?;
        validate_figure_dimension("fig-device-aspect", options.fig_device_aspect)?;
        if let Some(height) = options.fig_device_height {
            validate_figure_dimension("fig-device-height", height)?;
        }
        let height = options
            .fig_device_height
            .unwrap_or(options.fig_device_width * options.fig_device_aspect);
        Ok(Self {
            format,
            dpi: options.fig_device_dpi,
            width: options.fig_device_width,
            height,
        })
    }

    pub fn extension(&self) -> &'static str {
        figure_extension(&self.format).expect("FigureSpec format is validated")
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
        format!("{}-1.{}", artifact_label_stem(label), self.extension())
    }

    pub fn artifact_filename(&self, label: &str) -> String {
        format!("{}.{}", artifact_label_stem(label), self.extension())
    }
}

fn validate_figure_format(format: &str) -> Result<()> {
    figure_extension(format)
        .map(|_| ())
        .ok_or_else(|| anyhow!("unsupported figure device format `{format}`"))
}

fn validate_figure_dimension(field: &str, value: f64) -> Result<()> {
    if value.is_finite() && value > 0.0 {
        return Ok(());
    }
    Err(anyhow!("`{field}` must be a positive finite number"))
}

fn figure_extension(format: &str) -> Option<&'static str> {
    match format {
        "png" => Some("png"),
        "jpeg" | "jpg" => Some("jpg"),
        "pdf" | "cairo_pdf" => Some("pdf"),
        "svg" => Some("svg"),
        _ => None,
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

impl ResultItem {
    pub fn stream(name: ResultItemName, text: impl Into<String>) -> Self {
        Self {
            item_type: ResultItemType::Stream,
            name: Some(name),
            text: Some(text.into()),
            ..Self::default()
        }
    }

    pub fn diagnostic(level: DiagnosticLevel, text: impl Into<String>) -> Self {
        Self {
            item_type: ResultItemType::Diagnostic,
            text: Some(text.into()),
            level: Some(level),
            ..Self::default()
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            item_type: ResultItemType::Error,
            name: Some(ResultItemName::Error),
            message: Some(message.into()),
            ..Self::default()
        }
    }

    pub fn rich_data(kind: ResultItemType, mime: impl Into<String>, value: Value) -> Self {
        let mut data = MimeData::new();
        data.insert(mime.into(), value);
        Self {
            item_type: kind,
            data: Some(data),
            ..Self::default()
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
    /// Canonical executable source after fence/header normalization. The
    /// Typst runtime uses this for echoing and falls back for schema-v1 files
    /// written by older Calepin versions.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
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
    /// No engine was available to run this block. Distinct from `Skipped`,
    /// which means Calepin could have run it and was told not to: the runtime
    /// renders an unavailable block as ordinary fenced code rather than as
    /// chunk output, since a fence tagged with a language Calepin has no engine
    /// for is prose.
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultsDocument {
    pub schema: u8,
    pub calepin_version: String,
    pub input: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub generation: String,
    #[serde(default)]
    pub store: serde_json::Map<String, Value>,
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
    /// Root directory containing the shared Calepin runtime and all notebook
    /// artifact directories for this project.
    pub fn artifact_root(&self) -> PathBuf {
        let mut root = self.artifact_dir.clone();
        for _ in self.input_rel.components() {
            root.pop();
        }
        root
    }

    pub fn artifact_path(&self, name: impl AsRef<Path>) -> PathBuf {
        self.artifact_dir.join(name)
    }

    pub fn artifact_relative_path(&self, name: impl AsRef<Path>) -> PathBuf {
        let path = self.artifact_path(name);
        path.strip_prefix(&self.root)
            .map(Path::to_path_buf)
            .unwrap_or(path)
    }

    /// Generated Typst entry files carry the user's document body, so they live
    /// beside the source document rather than under the artifact directory.
    /// Typst resolves relative paths from the file that contains them: a body
    /// staged under `.calepin/` would look for `#import "utils.typ"` or
    /// `#image("fig.png")` in the artifact directory instead of next to the
    /// document. Everything that does not carry the body (results, figures,
    /// metadata) stays under `artifact_dir`.
    pub fn entry_path(&self, name: &str) -> PathBuf {
        self.work_dir.join(self.entry_file_name(name))
    }

    pub fn entry_relative_path(&self, name: &str) -> PathBuf {
        let path = self.entry_path(name);
        path.strip_prefix(&self.root)
            .map(Path::to_path_buf)
            .unwrap_or(path)
    }

    pub fn entry_file_name(&self, name: &str) -> String {
        let stem = self
            .input_rel
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_default();
        crate::typst::paths::entry_file_name_for(&stem, name)
    }

    /// Every generated entry file for this document, for cleanup.
    pub fn entry_paths(&self) -> Vec<PathBuf> {
        ENTRY_FILE_NAMES
            .iter()
            .map(|name| self.entry_path(name))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exec_options() -> ExecOptions {
        ExecOptions {
            fig_device_format: DEFAULT_FIG_DEVICE_FORMAT.to_string(),
            fig_device_dpi: DEFAULT_FIG_DEVICE_DPI,
            fig_device_width: DEFAULT_FIG_DEVICE_WIDTH,
            fig_device_height: DEFAULT_FIG_DEVICE_HEIGHT,
            fig_device_aspect: DEFAULT_FIG_DEVICE_ASPECT,
            eval: true,
            error: false,
            store_get: Vec::new(),
            store_set: Vec::new(),
        }
    }

    #[test]
    fn chunk_result_document_serializes_crossref_labels() {
        let doc = ChunkResultDocument {
            label: "fig-x".to_string(),
            engine: EngineName::R,
            source: "plot(x)".to_string(),
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
        assert!(json.contains(r#""source":"plot(x)""#), "{json}");
    }

    #[test]
    fn artifact_label_stems_are_safe_and_collision_resistant() {
        assert_eq!(artifact_label_stem("fig-demo_1.2"), "fig-demo_1.2");
        assert_eq!(artifact_label_stem("../fig/demo"), "..%2Ffig%2Fdemo");
        assert_eq!(artifact_label_stem(r"fig\demo"), "fig%5Cdemo");
        assert_eq!(artifact_label_stem("fig%2Fdemo"), "fig%252Fdemo");
        assert_eq!(artifact_label_stem("café"), "caf%C3%A9");
    }

    #[test]
    fn chunk_result_document_accepts_legacy_documents_without_source() {
        let json = r#"{
          "label":"legacy",
          "engine":"python",
          "status":"ok",
          "options":{
            "echo":true,"output":true,"results":"render","warning":true,
            "message":true,"placeholder":true,"fig-width":null,"fig-height":null,
            "fig-align":null,"fig-responsive":null,"fig-link":null,"fig-caption":null,
            "fig-cap-location":null,"fig-alt-text":null,"fig-subcaptions":null,
            "fig-layout-columns":null,"fig-layout-rows":null,"kind":null
          },
          "items":[]
        }"#;

        let doc: ChunkResultDocument = serde_json::from_str(json).unwrap();
        assert_eq!(doc.source, "");
        assert_eq!(doc.label, "legacy");
    }

    #[test]
    fn result_item_constructors_populate_expected_fields() {
        let stream = ResultItem::stream(ResultItemName::Stdout, "hello");
        assert_eq!(stream.item_type, ResultItemType::Stream);
        assert_eq!(stream.name, Some(ResultItemName::Stdout));
        assert_eq!(stream.text.as_deref(), Some("hello"));

        let diagnostic = ResultItem::diagnostic(DiagnosticLevel::Warning, "careful");
        assert_eq!(diagnostic.item_type, ResultItemType::Diagnostic);
        assert_eq!(diagnostic.level, Some(DiagnosticLevel::Warning));
        assert_eq!(diagnostic.text.as_deref(), Some("careful"));

        let error = ResultItem::error("boom");
        assert_eq!(error.item_type, ResultItemType::Error);
        assert_eq!(error.name, Some(ResultItemName::Error));
        assert_eq!(error.message.as_deref(), Some("boom"));

        let rich = ResultItem::rich_data(
            ResultItemType::Display,
            "text/plain",
            Value::String("rendered".to_string()),
        );
        let data = rich.data.as_ref().unwrap();
        assert_eq!(rich.item_type, ResultItemType::Display);
        assert_eq!(
            data.get("text/plain"),
            Some(&Value::String("rendered".to_string()))
        );
    }

    #[test]
    fn parses_typed_diagram_engines() {
        for name in ["mermaid", "tikz", "dot", "d2"] {
            let engine = EngineName::from_name(name);
            assert_eq!(engine.as_str(), name);
        }
    }

    #[test]
    fn setup_defaults_store_typed_results_mode() {
        assert_eq!(SetupDefaults::default().results, ResultsMode::Render);
    }

    #[test]
    fn parses_hidden_results_mode_alias() {
        assert_eq!(ResultsMode::parse("hidden").unwrap(), ResultsMode::Hide);
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
        let engine = EngineName::from_name("octave");
        assert_eq!(engine, EngineName::Jupyter("octave".to_string()));
        assert_eq!(engine.as_str(), "octave");
        assert!(!engine.is_diagram());
    }

    #[test]
    fn engine_name_parse_rejects_blank_names() {
        for value in ["", " ", "\t\n"] {
            let err = EngineName::parse(value).unwrap_err().to_string();
            assert!(err.contains("engine name"), "{value:?}: {err}");
            assert!(err.contains("empty"), "{value:?}: {err}");
        }
    }

    #[test]
    fn engine_name_parse_accepts_known_and_jupyter_names() {
        assert_eq!(EngineName::parse("python").unwrap(), EngineName::Python);
        assert_eq!(
            EngineName::parse("octave").unwrap(),
            EngineName::Jupyter("octave".to_string())
        );
    }

    #[test]
    fn figure_spec_rejects_unknown_formats() {
        let options = ExecOptions {
            fig_device_format: "bmp".to_string(),
            ..exec_options()
        };

        let err = FigureSpec::from_exec_options(&EngineName::R, &options).unwrap_err();

        assert!(err.to_string().contains("unsupported figure device format"));
    }

    #[test]
    fn figure_spec_rejects_invalid_dimensions() {
        let cases = [
            (
                "fig-device-width",
                ExecOptions {
                    fig_device_width: 0.0,
                    ..exec_options()
                },
            ),
            (
                "fig-device-width",
                ExecOptions {
                    fig_device_width: f64::INFINITY,
                    ..exec_options()
                },
            ),
            (
                "fig-device-height",
                ExecOptions {
                    fig_device_height: Some(-1.0),
                    ..exec_options()
                },
            ),
            (
                "fig-device-height",
                ExecOptions {
                    fig_device_height: Some(f64::NAN),
                    ..exec_options()
                },
            ),
            (
                "fig-device-aspect",
                ExecOptions {
                    fig_device_aspect: 0.0,
                    ..exec_options()
                },
            ),
            (
                "fig-device-aspect",
                ExecOptions {
                    fig_device_aspect: f64::NEG_INFINITY,
                    ..exec_options()
                },
            ),
        ];

        for (field, options) in cases {
            let err = FigureSpec::from_exec_options(&EngineName::R, &options)
                .unwrap_err()
                .to_string();
            assert!(err.contains(field), "{field}: {err}");
            assert!(err.contains("positive finite number"), "{field}: {err}");
        }
    }

    #[test]
    fn setup_theme_typst_selects_raw_typst_output() {
        let defaults = SetupDefaults {
            theme: Some(Value::String("typst".to_string())),
            ..SetupDefaults::default()
        };

        assert_eq!(
            defaults.theme_selection(Path::new("/tmp")).unwrap(),
            Some(crate::theme::ThemeSelection::Typst)
        );
    }

    #[test]
    fn setup_theme_rejects_boolean_values() {
        for value in [Value::Bool(false), Value::Bool(true)] {
            let defaults = SetupDefaults {
                theme: Some(value),
                ..SetupDefaults::default()
            };

            let err = defaults.theme_selection(Path::new("/tmp")).unwrap_err();
            assert!(err.to_string().contains("invalid setup theme value"));
        }
    }
}
