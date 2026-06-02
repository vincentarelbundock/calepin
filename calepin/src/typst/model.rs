use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

pub const RESULT_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineName {
    R,
    Python,
    Sh,
}

impl EngineName {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value {
            "r" => Ok(Self::R),
            "python" => Ok(Self::Python),
            "sh" | "bash" => Ok(Self::Sh),
            other => Err(anyhow::anyhow!("unsupported engine `{}`", other)),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::R => "r",
            Self::Python => "python",
            Self::Sh => "sh",
        }
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
            return Err(anyhow::anyhow!("item must be `all`, `first`, `last`, or an integer"));
        };
        match s {
            "all" => Ok(Self::ALL),
            "first" => Ok(Self::FIRST),
            "last" => Ok(Self::LAST),
            other => Err(anyhow::anyhow!("unsupported item selector `{}`", other)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetupDefaults {
    pub cache: bool,
    pub echo: bool,
    pub eval: bool,
    pub include: bool,
    pub results: String,
    pub warning: bool,
    pub message: bool,
    pub error: bool,
    pub format: Vec<String>,
    pub item: ItemSelector,
    pub placeholder: bool,
    pub dev: String,
    pub dpi: u32,
    pub fig_width: f64,
    pub fig_height: Option<f64>,
}

impl Default for SetupDefaults {
    fn default() -> Self {
        Self {
            cache: true,
            echo: true,
            eval: true,
            include: true,
            results: "verbatim".to_string(),
            warning: true,
            message: true,
            error: false,
            format: default_format_order(),
            item: ItemSelector::ALL,
            placeholder: true,
            dev: "svg".to_string(),
            dpi: 150,
            fig_width: 6.0,
            fig_height: None,
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
    pub cache: bool,
    pub eval: bool,
    pub error: bool,
    pub dev: String,
    pub dpi: u32,
    pub fig_width: f64,
    pub fig_height: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayOptions {
    pub echo: bool,
    pub include: bool,
    pub results: ResultsMode,
    pub warning: bool,
    pub message: bool,
    pub format: Vec<String>,
    pub item: ItemSelector,
    pub placeholder: bool,
    pub out_width: Option<String>,
    pub out_height: Option<String>,
    pub fig_cap: Option<String>,
    pub fig_alt: Option<String>,
    pub tbl_cap: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultItem {
    #[serde(rename = "type")]
    pub item_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
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
    pub cached: bool,
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
    pub work_dir: PathBuf,
    pub results_path: PathBuf,
    pub figures_dir: PathBuf,
    pub cache_dir: PathBuf,
}
