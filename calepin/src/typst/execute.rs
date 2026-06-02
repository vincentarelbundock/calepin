use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::{ExecuteConfig, FigureConfig, Metadata};
use crate::engines::{self, python::PythonSession, r::RSession, sh::ShSession, EngineContext};
use crate::types::{ChunkOptions, ChunkResult as LegacyChunkResult, OptionValue};
use crate::typst::model::{
    ChunkResultDocument, ChunkSpec, ChunkStatus, EngineName, MimeData, ResultItem,
};

#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    pub cwd: PathBuf,
    pub rscript: PathBuf,
    pub python: PathBuf,
    pub shell: PathBuf,
    pub timeout: Option<Duration>,
}

pub struct EnginePool {
    r: Option<RSession>,
    python: Option<PythonSession>,
    sh: Option<ShSession>,
    config: ExecutionConfig,
}

impl EnginePool {
    pub fn new(config: ExecutionConfig) -> Self {
        Self {
            r: None,
            python: None,
            sh: None,
            config,
        }
    }

    pub fn execute_chunk(
        &mut self,
        chunk: &ChunkSpec,
        figures_dir: &Path,
        artifact_path: impl Fn(&Path) -> String,
    ) -> Result<ChunkResultDocument> {
        if !chunk.exec_options.eval {
            return Ok(ChunkResultDocument {
                label: chunk.label.clone(),
                engine: chunk.engine,
                status: ChunkStatus::Skipped,
                cached: false,
                items: Vec::new(),
            });
        }

        let options = legacy_options(chunk);
        let source = lines(&chunk.code);
        let fig_ext = figure_extension(&chunk.exec_options.dev);
        let mut ctx = self.context_for(chunk.engine)?;
        let legacy = engines::execute_chunk(
            &source,
            &options,
            &chunk.label,
            figures_dir,
            &fig_ext,
            &mut ctx,
        )?;
        let items = normalize_legacy_results(chunk, figures_dir, &fig_ext, legacy, artifact_path)?;
        let has_error = items.iter().any(|item| item.item_type == "error");
        if has_error && !chunk.exec_options.error {
            let message = items
                .iter()
                .find(|item| item.item_type == "error")
                .and_then(|item| item.message.as_deref())
                .unwrap_or("execution failed");
            return Err(anyhow!("chunk `{}` failed: {}", chunk.label, message));
        }
        Ok(ChunkResultDocument {
            label: chunk.label.clone(),
            engine: chunk.engine,
            status: if has_error { ChunkStatus::Error } else { ChunkStatus::Ok },
            cached: false,
            items,
        })
    }

    fn context_for(&mut self, engine: EngineName) -> Result<EngineContext<'_>> {
        match engine {
            EngineName::R => {
                if self.r.is_none() {
                    let program = self.config.rscript.to_string_lossy().to_string();
                    self.r = Some(RSession::init_with_program(
                        &program,
                        "typst",
                        Some(&self.config.cwd),
                        self.config.timeout,
                    )?);
                }
            }
            EngineName::Python => {
                if self.python.is_none() {
                    let program = self.config.python.to_string_lossy().to_string();
                    self.python = Some(PythonSession::init_with_program(
                        &program,
                        Some(&self.config.cwd),
                        self.config.timeout,
                    )?);
                }
            }
            EngineName::Sh => {
                if self.sh.is_none() {
                    let program = self.config.shell.to_string_lossy().to_string();
                    self.sh = Some(ShSession::init_with_program(
                        &program,
                        Some(&self.config.cwd),
                        self.config.timeout,
                    )?);
                }
            }
        }

        Ok(EngineContext {
            r: self.r.as_mut(),
            python: self.python.as_mut(),
            sh: self.sh.as_mut(),
        })
    }
}

pub fn normalize_legacy_results(
    chunk: &ChunkSpec,
    figures_dir: &Path,
    fig_ext: &str,
    legacy: Vec<LegacyChunkResult>,
    artifact_path: impl Fn(&Path) -> String,
) -> Result<Vec<ResultItem>> {
    let mut items = Vec::new();
    for result in legacy {
        match result {
            LegacyChunkResult::Source(_) | LegacyChunkResult::Preamble(_) => {}
            LegacyChunkResult::Output(text) => items.push(stream_item("stdout", text)),
            LegacyChunkResult::Asis(text) => {
                items.push(rich_text_item("display", "text/x-typst", Value::String(text)));
            }
            LegacyChunkResult::Warning(text) => items.push(diagnostic_item("warning", text)),
            LegacyChunkResult::Message(text) => items.push(diagnostic_item("message", text)),
            LegacyChunkResult::Error(text) => items.push(error_item(text)),
            LegacyChunkResult::Plot(path) => {
                let artifact = normalize_plot_path(&chunk.label, figures_dir, fig_ext, &path)
                    .context("failed to normalize plot artifact path")?;
                let mut data = MimeData::new();
                let mime = if fig_ext == "png" {
                    "image/png"
                } else {
                    "image/svg+xml"
                };
                data.insert(mime.to_string(), json!({ "path": artifact_path(&artifact) }));
                items.push(ResultItem {
                    item_type: "display".to_string(),
                    name: None,
                    text: None,
                    level: None,
                    message: None,
                    traceback: None,
                    data: Some(data),
                    metadata: BTreeMap::new(),
                });
            }
        }
    }
    Ok(items)
}

fn normalize_plot_path(label: &str, figures_dir: &Path, fig_ext: &str, path: &Path) -> Result<PathBuf> {
    let target = figures_dir.join(format!("{}.{}", label, fig_ext));
    if path == target {
        return Ok(target);
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if path.exists() {
        std::fs::copy(path, &target)?;
        let _ = std::fs::remove_file(path);
    }
    Ok(target)
}

fn stream_item(name: &str, text: String) -> ResultItem {
    ResultItem {
        item_type: "stream".to_string(),
        name: Some(name.to_string()),
        text: Some(text),
        level: None,
        message: None,
        traceback: None,
        data: None,
        metadata: BTreeMap::new(),
    }
}

fn diagnostic_item(level: &str, text: String) -> ResultItem {
    ResultItem {
        item_type: "diagnostic".to_string(),
        name: None,
        text: Some(text),
        level: Some(level.to_string()),
        message: None,
        traceback: None,
        data: None,
        metadata: BTreeMap::new(),
    }
}

fn error_item(message: String) -> ResultItem {
    ResultItem {
        item_type: "error".to_string(),
        name: Some("error".to_string()),
        text: None,
        level: None,
        message: Some(message),
        traceback: None,
        data: None,
        metadata: BTreeMap::new(),
    }
}

fn rich_text_item(kind: &str, mime: &str, value: Value) -> ResultItem {
    let mut data = MimeData::new();
    data.insert(mime.to_string(), value);
    ResultItem {
        item_type: kind.to_string(),
        name: None,
        text: None,
        level: None,
        message: None,
        traceback: None,
        data: Some(data),
        metadata: BTreeMap::new(),
    }
}

fn legacy_options(chunk: &ChunkSpec) -> ChunkOptions {
    let fig_height = chunk
        .exec_options
        .fig_height
        .unwrap_or(chunk.exec_options.fig_width * 0.618);
    let mut inner = HashMap::new();
    inner.insert("engine".to_string(), OptionValue::String(chunk.engine.as_str().to_string()));
    inner.insert("cache".to_string(), OptionValue::Bool(false));
    inner.insert("eval".to_string(), OptionValue::Bool(chunk.exec_options.eval));
    inner.insert("warning".to_string(), OptionValue::Bool(true));
    inner.insert("message".to_string(), OptionValue::Bool(true));
    inner.insert("dev".to_string(), OptionValue::String(chunk.exec_options.dev.clone()));
    inner.insert("dpi".to_string(), OptionValue::Number(chunk.exec_options.dpi as f64));
    inner.insert("fig_width".to_string(), OptionValue::Number(chunk.exec_options.fig_width));
    inner.insert("fig_height".to_string(), OptionValue::Number(fig_height));

    ChunkOptions {
        inner,
        metadata: Metadata {
            dpi: Some(chunk.exec_options.dpi as f64),
            figure: Some(FigureConfig {
                fig_width: Some(chunk.exec_options.fig_width),
                fig_height: Some(fig_height),
                device: Some(chunk.exec_options.dev.clone()),
                ..FigureConfig::default()
            }),
            execute: Some(ExecuteConfig {
                eval: Some(chunk.exec_options.eval),
                warning: Some(true),
                message: Some(true),
            }),
            ..Metadata::default()
        },
    }
}

fn lines(code: &str) -> Vec<String> {
    code.lines().map(ToOwned::to_owned).collect()
}

pub fn figure_extension(dev: &str) -> String {
    match dev {
        "png" => "png".to_string(),
        "jpeg" | "jpg" => "jpg".to_string(),
        "pdf" | "cairo_pdf" => "pdf".to_string(),
        _ => "svg".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::model::{
        DisplayOptions, ExecOptions, ItemSelector, ResultsMode, SetupDefaults,
    };

    fn chunk(results: ResultsMode) -> ChunkSpec {
        let defaults = SetupDefaults::default();
        ChunkSpec {
            label: "fig-demo".to_string(),
            engine: EngineName::R,
            code: "x <- 1".to_string(),
            exec_options: ExecOptions {
                cache: true,
                eval: true,
                error: false,
                dev: "svg".to_string(),
                dpi: 150,
                fig_width: 6.0,
                fig_height: None,
            },
            display_options: DisplayOptions {
                echo: true,
                include: true,
                results,
                warning: true,
                message: true,
                format: defaults.format,
                item: ItemSelector::ALL,
                placeholder: true,
                out_width: None,
                out_height: None,
                fig_cap: None,
                fig_alt: None,
                tbl_cap: None,
                kind: None,
            },
            ordinal: 0,
        }
    }

    #[test]
    fn normalizes_verbatim_output_and_diagnostics() {
        let dir = tempfile::tempdir().unwrap();
        let items = normalize_legacy_results(
            &chunk(ResultsMode::Verbatim),
            dir.path(),
            "svg",
            vec![
                LegacyChunkResult::Source(vec!["x <- 1".to_string()]),
                LegacyChunkResult::Output("1".to_string()),
                LegacyChunkResult::Warning("careful".to_string()),
                LegacyChunkResult::Message("note".to_string()),
            ],
            |_| "unused".to_string(),
        )
        .unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].item_type, "stream");
        assert_eq!(items[0].text.as_deref(), Some("1"));
        assert_eq!(items[1].level.as_deref(), Some("warning"));
        assert_eq!(items[2].level.as_deref(), Some("message"));
    }

    #[test]
    fn normalizes_engine_asis_to_typst_mime() {
        let dir = tempfile::tempdir().unwrap();
        let items = normalize_legacy_results(
            &chunk(ResultsMode::Asis),
            dir.path(),
            "svg",
            vec![LegacyChunkResult::Asis("#table()[x]".to_string())],
            |_| "unused".to_string(),
        )
        .unwrap();
        let data = items[0].data.as_ref().unwrap();
        assert_eq!(data.get("text/x-typst").unwrap(), "#table()[x]");
    }

    #[test]
    fn stdout_is_stored_independent_of_results_mode() {
        let dir = tempfile::tempdir().unwrap();
        let items = normalize_legacy_results(
            &chunk(ResultsMode::Hide),
            dir.path(),
            "svg",
            vec![LegacyChunkResult::Output("visible to runtime".to_string())],
            |_| "unused".to_string(),
        )
        .unwrap();
        assert_eq!(items[0].item_type, "stream");
        assert_eq!(items[0].text.as_deref(), Some("visible to runtime"));
    }

    #[test]
    fn normalizes_plot_to_label_artifact() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("fig-demo-1.svg");
        std::fs::write(&source, "<svg></svg>").unwrap();
        let items = normalize_legacy_results(
            &chunk(ResultsMode::Verbatim),
            dir.path(),
            "svg",
            vec![LegacyChunkResult::Plot(source)],
            |path| path.file_name().unwrap().to_string_lossy().to_string(),
        )
        .unwrap();
        let data = items[0].data.as_ref().unwrap();
        assert_eq!(data["image/svg+xml"]["path"], "fig-demo.svg");
        assert!(dir.path().join("fig-demo.svg").exists());
    }
}
