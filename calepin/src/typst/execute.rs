use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::ExecutablePaths;
use crate::engines::{
    self, jupyter::JupyterBridgeSession, python::PythonSession, r::RSession, EngineContext,
    EngineResult,
};
use crate::typst::model::{
    ChunkResultDocument, ChunkSpec, ChunkStatus, DiagnosticLevel, EngineName, FigureSpec, MimeData,
    ResultItem, ResultItemName, ResultItemType, ResultsMode,
};

#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    pub cwd: PathBuf,
    pub executables: ExecutablePaths,
    pub timeout: Option<Duration>,
}

pub struct EnginePool {
    r: Option<RSession>,
    python: Option<PythonSession>,
    jupyter: Option<JupyterBridgeSession>,
    config: ExecutionConfig,
}

impl EnginePool {
    pub fn new(config: ExecutionConfig) -> Self {
        Self {
            r: None,
            python: None,
            jupyter: None,
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
                engine: chunk.engine.clone(),
                status: ChunkStatus::Skipped,
                display_options: chunk.display_options.clone(),
                items: Vec::new(),
            });
        }

        let engine = chunk.engine.clone();
        let source = lines(&chunk.code);
        let figure = FigureSpec::from_exec_options(engine.clone(), &chunk.exec_options);
        let engine_results = if engine.is_diagram() {
            let fig_path = figures_dir.join(format!("{}-1.svg", chunk.label));
            engines::diagram::execute_diagram(
                &chunk.code,
                engine.clone(),
                &fig_path,
                &source,
                &self.config.executables,
            )?
        } else {
            let mut ctx = self.context_for(engine.clone())?;
            engines::execute_chunk(
                &source,
                engine.clone(),
                &chunk.label,
                figures_dir,
                &figure,
                &mut ctx,
            )?
        };
        let items =
            normalize_engine_results(chunk, figures_dir, &figure, engine_results, artifact_path)?;
        let has_error = items
            .iter()
            .any(|item| item.item_type == ResultItemType::Error);
        if has_error && !chunk.exec_options.error {
            let message = items
                .iter()
                .find(|item| item.item_type == ResultItemType::Error)
                .and_then(|item| item.message.as_deref())
                .unwrap_or("execution failed");
            return Err(anyhow!("chunk `{}` failed: {}", chunk.label, message));
        }
        Ok(ChunkResultDocument {
            label: chunk.label.clone(),
            engine,
            status: if has_error {
                ChunkStatus::Error
            } else {
                ChunkStatus::Ok
            },
            display_options: chunk.display_options.clone(),
            items,
        })
    }

    fn ensure_r_session(&mut self) -> Result<()> {
        if self.r.is_none() {
            self.r = Some(RSession::init_with_program(
                &self.config.executables.rscript,
                "typst",
                Some(&self.config.cwd),
                self.config.timeout,
            )?);
        }
        Ok(())
    }

    fn ensure_python_session(&mut self) -> Result<()> {
        if self.python.is_none() {
            self.python = Some(PythonSession::init_with_program(
                &self.config.executables.python,
                Some(&self.config.cwd),
                self.config.timeout,
            )?);
        }
        Ok(())
    }

    fn ensure_jupyter_session(&mut self) -> Result<()> {
        if self.jupyter.is_none() {
            self.jupyter = Some(JupyterBridgeSession::init_with_program(
                &self.config.executables.python,
                Some(&self.config.cwd),
                self.config.timeout,
            )?);
        }
        Ok(())
    }

    fn context_for(&mut self, engine: EngineName) -> Result<EngineContext<'_>> {
        match engine {
            EngineName::R => self.ensure_r_session()?,
            EngineName::Python => self.ensure_python_session()?,
            EngineName::Diagram(_) => {
                return Err(anyhow!(
                    "diagram engine `{}` does not use a persistent context",
                    engine
                ));
            }
            EngineName::Jupyter(_) => self.ensure_jupyter_session()?,
        }

        Ok(EngineContext {
            r: self.r.as_mut(),
            python: self.python.as_mut(),
            jupyter: self.jupyter.as_mut(),
        })
    }
}

pub fn normalize_engine_results(
    chunk: &ChunkSpec,
    figures_dir: &Path,
    figure: &FigureSpec,
    engine_results: Vec<EngineResult>,
    artifact_path: impl Fn(&Path) -> String,
) -> Result<Vec<ResultItem>> {
    let mut items = Vec::new();
    let mut typst_result_index = 1usize;
    let mut write_typst_result = |text: String| -> Result<Value> {
        let filename = if typst_result_index == 1 {
            format!("{}.typ", chunk.label)
        } else {
            format!("{}-{}.typ", chunk.label, typst_result_index)
        };
        typst_result_index += 1;
        let artifact = figures_dir.join(filename);
        if let Some(parent) = artifact.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&artifact, text)?;
        Ok(json!({ "path": artifact_path(&artifact) }))
    };
    for result in engine_results {
        match result {
            EngineResult::Source(_) | EngineResult::Preamble(_) => {}
            EngineResult::Output(text) => {
                if matches!(chunk.display_options.results, ResultsMode::Typst) {
                    let value = write_typst_result(text)?;
                    items.push(rich_text_item(
                        ResultItemType::Display,
                        "text/x-typst",
                        value,
                    ));
                } else {
                    items.push(stream_item(ResultItemName::Stdout, text));
                }
            }
            EngineResult::Warning(text) => {
                items.push(diagnostic_item(DiagnosticLevel::Warning, text))
            }
            EngineResult::Message(text) => {
                items.push(diagnostic_item(DiagnosticLevel::Message, text))
            }
            EngineResult::Error(text) => items.push(error_item(text)),
            EngineResult::Plot(path) => {
                let artifact = normalize_plot_path(&chunk.label, figures_dir, figure, &path)
                    .context("failed to normalize plot artifact path")?;
                let mut data = MimeData::new();
                data.insert(
                    figure.mime_type().to_string(),
                    json!({ "path": artifact_path(&artifact) }),
                );
                items.push(ResultItem {
                    item_type: ResultItemType::Display,
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

fn normalize_plot_path(
    label: &str,
    figures_dir: &Path,
    figure: &FigureSpec,
    path: &Path,
) -> Result<PathBuf> {
    let target = figures_dir.join(normalized_plot_filename(label, figure, path));
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

fn normalized_plot_filename(label: &str, figure: &FigureSpec, path: &Path) -> String {
    let extension = figure.extension();
    let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
        return figure.artifact_filename(label);
    };
    let Some(suffix) = stem.strip_prefix(&format!("{label}-")) else {
        return figure.artifact_filename(label);
    };
    let Ok(index) = suffix.parse::<usize>() else {
        return figure.artifact_filename(label);
    };
    if index <= 1 {
        figure.artifact_filename(label)
    } else {
        format!("{label}-{index}.{extension}")
    }
}

fn stream_item(name: ResultItemName, text: String) -> ResultItem {
    ResultItem {
        item_type: ResultItemType::Stream,
        name: Some(name),
        text: Some(text),
        level: None,
        message: None,
        traceback: None,
        data: None,
        metadata: BTreeMap::new(),
    }
}

fn diagnostic_item(level: DiagnosticLevel, text: String) -> ResultItem {
    ResultItem {
        item_type: ResultItemType::Diagnostic,
        name: None,
        text: Some(text),
        level: Some(level),
        message: None,
        traceback: None,
        data: None,
        metadata: BTreeMap::new(),
    }
}

fn error_item(message: String) -> ResultItem {
    ResultItem {
        item_type: ResultItemType::Error,
        name: Some(ResultItemName::Error),
        text: None,
        level: None,
        message: Some(message),
        traceback: None,
        data: None,
        metadata: BTreeMap::new(),
    }
}

fn rich_text_item(kind: ResultItemType, mime: &str, value: Value) -> ResultItem {
    let mut data = MimeData::new();
    data.insert(mime.to_string(), value);
    ResultItem {
        item_type: kind,
        name: None,
        text: None,
        level: None,
        message: None,
        traceback: None,
        data: Some(data),
        metadata: BTreeMap::new(),
    }
}

fn lines(code: &str) -> Vec<String> {
    code.lines().map(ToOwned::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::model::{DisplayOptions, ExecOptions, ResultsMode, SetupDefaults};

    fn chunk(results: ResultsMode) -> ChunkSpec {
        let defaults = SetupDefaults::default();
        ChunkSpec {
            label: "fig-demo".to_string(),
            engine: EngineName::R,
            code: "x <- 1".to_string(),
            exec_options: ExecOptions {
                eval: true,
                error: false,
                fig_device_format: "svg".to_string(),
                fig_device_dpi: 150,
                fig_device_width: 6.0,
                fig_device_height: None,
                fig_device_aspect: 0.618,
            },
            display_options: DisplayOptions {
                echo: true,
                output: true,
                results,
                warning: true,
                message: true,
                placeholder: true,
                fig_width: None,
                fig_height: None,
                fig_align: None,
                fig_responsive: None,
                fig_link: None,
                fig_caption: None,
                fig_cap_location: None,
                fig_alt_text: None,
                fig_subcaptions: None,
                fig_layout_columns: None,
                fig_layout_rows: None,
                kind: None,
            },
            ordinal: 0,
        }
    }

    fn figure_for(chunk: &ChunkSpec) -> FigureSpec {
        FigureSpec::from_exec_options(chunk.engine.clone(), &chunk.exec_options)
    }

    #[test]
    fn normalizes_verbatim_output_and_diagnostics() {
        let dir = tempfile::tempdir().unwrap();
        let chunk = chunk(ResultsMode::Verbatim);
        let figure = figure_for(&chunk);
        let items = normalize_engine_results(
            &chunk,
            dir.path(),
            &figure,
            vec![
                EngineResult::Source(vec!["x <- 1".to_string()]),
                EngineResult::Output("1".to_string()),
                EngineResult::Warning("careful".to_string()),
                EngineResult::Message("note".to_string()),
            ],
            |_| "unused".to_string(),
        )
        .unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].item_type, ResultItemType::Stream);
        assert_eq!(items[0].text.as_deref(), Some("1"));
        assert_eq!(items[1].level, Some(DiagnosticLevel::Warning));
        assert_eq!(items[2].level, Some(DiagnosticLevel::Message));
    }

    #[test]
    fn typst_results_coerces_stdout_to_typst_mime() {
        let dir = tempfile::tempdir().unwrap();
        let chunk = chunk(ResultsMode::Typst);
        let figure = figure_for(&chunk);
        let items = normalize_engine_results(
            &chunk,
            dir.path(),
            &figure,
            vec![EngineResult::Output("#table()[x]".to_string())],
            |path| path.file_name().unwrap().to_string_lossy().to_string(),
        )
        .unwrap();

        let data = items[0].data.as_ref().unwrap();
        assert_eq!(data["text/x-typst"]["path"], "fig-demo.typ");
        assert!(dir.path().join("fig-demo.typ").exists());
    }

    #[test]
    fn stdout_is_stored_independent_of_results_mode() {
        let dir = tempfile::tempdir().unwrap();
        let chunk = chunk(ResultsMode::Hide);
        let figure = figure_for(&chunk);
        let items = normalize_engine_results(
            &chunk,
            dir.path(),
            &figure,
            vec![EngineResult::Output("visible to runtime".to_string())],
            |_| "unused".to_string(),
        )
        .unwrap();
        assert_eq!(items[0].item_type, ResultItemType::Stream);
        assert_eq!(items[0].text.as_deref(), Some("visible to runtime"));
    }

    #[test]
    fn normalizes_plot_to_label_artifact() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("fig-demo-1.svg");
        std::fs::write(&source, "<svg></svg>").unwrap();
        let chunk = chunk(ResultsMode::Verbatim);
        let figure = figure_for(&chunk);
        let items = normalize_engine_results(
            &chunk,
            dir.path(),
            &figure,
            vec![EngineResult::Plot(source)],
            |path| path.file_name().unwrap().to_string_lossy().to_string(),
        )
        .unwrap();
        let data = items[0].data.as_ref().unwrap();
        assert_eq!(data["image/svg+xml"]["path"], "fig-demo.svg");
        assert!(dir.path().join("fig-demo.svg").exists());
    }

    #[test]
    fn normalizes_additional_plots_to_numbered_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("fig-demo-1.svg");
        let second = dir.path().join("fig-demo-2.svg");
        std::fs::write(&first, "<svg id=\"first\"></svg>").unwrap();
        std::fs::write(&second, "<svg id=\"second\"></svg>").unwrap();
        let chunk = chunk(ResultsMode::Verbatim);
        let figure = figure_for(&chunk);
        let items = normalize_engine_results(
            &chunk,
            dir.path(),
            &figure,
            vec![EngineResult::Plot(first), EngineResult::Plot(second)],
            |path| path.file_name().unwrap().to_string_lossy().to_string(),
        )
        .unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(
            items[0].data.as_ref().unwrap()["image/svg+xml"]["path"],
            "fig-demo.svg"
        );
        assert_eq!(
            items[1].data.as_ref().unwrap()["image/svg+xml"]["path"],
            "fig-demo-2.svg"
        );
        assert!(dir.path().join("fig-demo.svg").exists());
        assert!(dir.path().join("fig-demo-2.svg").exists());
    }

    #[test]
    fn diagram_engines_always_use_svg_figures() {
        assert_eq!(
            FigureSpec::from_exec_options(
                EngineName::parse("mermaid").unwrap(),
                &ExecOptions {
                    fig_device_format: "png".to_string(),
                    ..chunk(ResultsMode::Verbatim).exec_options
                }
            )
            .extension(),
            "svg"
        );
        assert_eq!(
            FigureSpec::from_exec_options(
                EngineName::parse("tikz").unwrap(),
                &ExecOptions {
                    fig_device_format: "pdf".to_string(),
                    ..chunk(ResultsMode::Verbatim).exec_options
                }
            )
            .extension(),
            "svg"
        );
        assert_eq!(
            FigureSpec::from_exec_options(
                EngineName::R,
                &ExecOptions {
                    fig_device_format: "png".to_string(),
                    ..chunk(ResultsMode::Verbatim).exec_options
                }
            )
            .extension(),
            "png"
        );
    }

    #[test]
    fn engine_pool_routes_unknown_engine_to_jupyter_arm() {
        let dir = tempfile::tempdir().unwrap();
        let missing_python = dir.path().join("missing-python3");
        let mut executables = ExecutablePaths::defaults();
        executables.python = missing_python.clone();
        let config = ExecutionConfig {
            cwd: dir.path().to_path_buf(),
            executables,
            timeout: Some(std::time::Duration::from_secs(5)),
        };
        let mut pool = EnginePool::new(config);
        let mut octave_chunk = chunk(ResultsMode::Verbatim);
        octave_chunk.engine = EngineName::Jupyter("octave".to_string());
        octave_chunk.label = "octave-test".to_string();
        octave_chunk.code = "disp(42)".to_string();
        let result = pool.execute_chunk(&octave_chunk, dir.path(), |_| "unused".to_string());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("failed to start Jupyter bridge")
                || err.contains(missing_python.to_string_lossy().as_ref()),
            "expected Jupyter bridge startup error, got: {err}"
        );
    }
}
