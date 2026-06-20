use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::ExecutablePaths;
use crate::engines::{
    self, jupyter::JupyterBridgeSession, python::PythonSession, r::RSession, EngineContext,
    EngineResult,
};
use crate::typst::io::write_if_changed;
use crate::typst::model::{
    ChunkResultDocument, ChunkSpec, ChunkStatus, DiagnosticLevel, EngineName, FigureSpec,
    ResultItem, ResultItemName, ResultItemType, ResultsMode, DEFAULT_FIG_DEVICE_ASPECT,
    DEFAULT_FIG_DEVICE_DPI, DEFAULT_FIG_DEVICE_WIDTH,
};

const PRELUDE_FIG_WIDTH: f64 = DEFAULT_FIG_DEVICE_WIDTH;
const PRELUDE_FIG_HEIGHT: f64 = DEFAULT_FIG_DEVICE_WIDTH * DEFAULT_FIG_DEVICE_ASPECT;
const PRELUDE_FIG_DPI: f64 = DEFAULT_FIG_DEVICE_DPI as f64;

#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    pub cwd: PathBuf,
    pub executables: ExecutablePaths,
    pub timeout: Option<Duration>,
    /// Document-level parameters, injected once per engine at session startup.
    pub params: Value,
    /// Path to the on-disk `params.json`, exposed to Jupyter kernels via
    /// `CALEPIN_PARAMS_PATH` for kernels Calepin cannot auto-bind.
    pub params_path: Option<PathBuf>,
}

impl ExecutionConfig {
    fn has_params(&self) -> bool {
        self.params
            .as_object()
            .is_some_and(|params| !params.is_empty())
    }
}

/// A prelude that errored is a Calepin bug (we generate the literal), so surface
/// the raw engine output rather than letting later chunks fail mysteriously.
///
/// The error tag is matched with its per-run sentinel prefix so a parameter
/// string value that happens to contain `_ERROR:` cannot trigger a false alarm
/// (the engine echoes the prelude source back as a tagged line).
fn check_prelude_output(raw: &str, engine: &str) -> Result<()> {
    let sentinel = raw.split_once('\n').map_or("", |(first, _)| first);
    if !sentinel.is_empty() && raw.contains(&format!("{sentinel}_ERROR:")) {
        return Err(anyhow!(
            "failed to inject document parameters into the {engine} engine: {}",
            raw.trim()
        ));
    }
    Ok(())
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
            return Ok(chunk_result_document(
                chunk,
                ChunkStatus::Skipped,
                Vec::new(),
            ));
        }

        let engine = chunk.engine.clone();
        let source = lines(&chunk.code);
        let figure = FigureSpec::from_exec_options(&engine, &chunk.exec_options)
            .map_err(|err| anyhow!("{}: {err}", execution_context(chunk)))?;
        let engine_results =
            self.run_engine_results(chunk, figures_dir, engine, &source, &figure)?;
        let items =
            normalize_engine_results(chunk, figures_dir, &figure, engine_results, artifact_path)
                .map_err(|err| {
                    anyhow!(
                        "failed to normalize results for chunk `{}`: {err}",
                        chunk.label
                    )
                })?;
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
        Ok(chunk_result_document(
            chunk,
            if has_error {
                ChunkStatus::Error
            } else {
                ChunkStatus::Ok
            },
            items,
        ))
    }

    fn run_engine_results(
        &mut self,
        chunk: &ChunkSpec,
        figures_dir: &Path,
        engine: EngineName,
        source: &[String],
        figure: &FigureSpec,
    ) -> Result<Vec<EngineResult>> {
        let results = if engine.is_diagram() {
            let fig_path = figures_dir.join(format!("{}-1.svg", chunk.label));
            engines::diagram::execute_diagram(
                &chunk.code,
                engine.clone(),
                &fig_path,
                source,
                &self.config.executables,
            )
            .map_err(|err| anyhow!("{}: {err}", execution_context(chunk)))
        } else {
            let mut ctx = self
                .context_for(engine.clone())
                .map_err(|err| anyhow!("{}: {err}", execution_context(chunk)))?;
            engines::execute_chunk(
                source,
                engine.clone(),
                &chunk.label,
                figures_dir,
                figure,
                &mut ctx,
            )
            .map_err(|err| anyhow!("{}: {err}", execution_context(chunk)))
        };
        results
    }

    fn ensure_r_session(&mut self) -> Result<()> {
        if self.r.is_none() {
            let mut session = RSession::init_with_program(
                &self.config.executables.rscript,
                "typst",
                Some(&self.config.cwd),
                self.config.timeout,
            )?;
            if self.config.has_params() {
                inject_r_params(&mut session, &self.config.params)?;
            }
            self.r = Some(session);
        }
        Ok(())
    }

    fn ensure_python_session(&mut self) -> Result<()> {
        if self.python.is_none() {
            let mut session = PythonSession::init_with_program(
                &self.config.executables.python,
                Some(&self.config.cwd),
                self.config.timeout,
            )?;
            if self.config.has_params() {
                inject_python_params(&mut session, &self.config.params)?;
            }
            self.python = Some(session);
        }
        Ok(())
    }

    fn ensure_jupyter_session(&mut self) -> Result<()> {
        if self.jupyter.is_none() {
            self.jupyter = Some(JupyterBridgeSession::init_with_program(
                &self.config.executables.python,
                Some(&self.config.cwd),
                self.config.timeout,
                self.config.params_path.as_deref(),
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

fn execution_context(chunk: &ChunkSpec) -> String {
    format!(
        "failed to execute chunk `{}` with engine `{}`",
        chunk.label, chunk.engine
    )
}

fn inject_r_params(session: &mut RSession, params: &Value) -> Result<()> {
    let code = engines::prelude::r_prelude("params", params);
    let raw = session.capture(
        &code,
        "",
        "svg",
        PRELUDE_FIG_WIDTH,
        PRELUDE_FIG_HEIGHT,
        PRELUDE_FIG_DPI,
    )?;
    check_prelude_output(&raw, "R")
}

fn inject_python_params(session: &mut PythonSession, params: &Value) -> Result<()> {
    let code = engines::prelude::python_prelude("params", params);
    let raw = session.capture(
        &code,
        "",
        PRELUDE_FIG_WIDTH,
        PRELUDE_FIG_HEIGHT,
        PRELUDE_FIG_DPI,
    )?;
    check_prelude_output(&raw, "Python")
}

fn chunk_result_document(
    chunk: &ChunkSpec,
    status: ChunkStatus,
    items: Vec<ResultItem>,
) -> ChunkResultDocument {
    ChunkResultDocument {
        label: chunk.label.clone(),
        engine: chunk.engine.clone(),
        status,
        display_options: chunk.display_options.clone(),
        items,
        crossref_labels: chunk.crossref_labels.clone(),
    }
}

fn normalize_engine_results(
    chunk: &ChunkSpec,
    figures_dir: &Path,
    figure: &FigureSpec,
    engine_results: Vec<EngineResult>,
    artifact_path: impl Fn(&Path) -> String,
) -> Result<Vec<ResultItem>> {
    ResultNormalizer {
        chunk,
        figures_dir,
        figure,
        artifact_path,
        items: Vec::new(),
        typst_result_index: 1,
        plot_index: 1,
    }
    .normalize(engine_results)
}

struct ResultNormalizer<'a, F>
where
    F: Fn(&Path) -> String,
{
    chunk: &'a ChunkSpec,
    figures_dir: &'a Path,
    figure: &'a FigureSpec,
    artifact_path: F,
    items: Vec<ResultItem>,
    typst_result_index: usize,
    plot_index: usize,
}

impl<'a, F> ResultNormalizer<'a, F>
where
    F: Fn(&Path) -> String,
{
    fn normalize(mut self, engine_results: Vec<EngineResult>) -> Result<Vec<ResultItem>> {
        for result in engine_results {
            match result {
                EngineResult::Source(_) | EngineResult::Preamble(_) => {}
                EngineResult::Output(text) => self.push_output(text)?,
                EngineResult::Warning(text) => {
                    self.items
                        .push(ResultItem::diagnostic(DiagnosticLevel::Warning, text));
                }
                EngineResult::Message(text) => {
                    self.items
                        .push(ResultItem::diagnostic(DiagnosticLevel::Message, text));
                }
                EngineResult::Error(text) => self.items.push(ResultItem::error(text)),
                EngineResult::Plot(path) => self.push_plot(path)?,
            }
        }
        Ok(self.items)
    }

    fn push_output(&mut self, text: String) -> Result<()> {
        if matches!(self.chunk.display_options.results, ResultsMode::Typst) {
            let value = self.write_typst_result(text)?;
            self.items.push(ResultItem::rich_data(
                ResultItemType::Display,
                "text/x-typst",
                value,
            ));
        } else {
            self.items
                .push(ResultItem::stream(ResultItemName::Stdout, text));
        }
        Ok(())
    }

    fn write_typst_result(&mut self, text: String) -> Result<Value> {
        let filename = if self.typst_result_index == 1 {
            format!("{}.typ", self.chunk.label)
        } else {
            format!("{}-{}.typ", self.chunk.label, self.typst_result_index)
        };
        self.typst_result_index += 1;
        let artifact = self.figures_dir.join(filename);
        write_if_changed(&artifact, text)
            .with_context(|| format!("failed to write Typst result {}", artifact.display()))?;
        Ok(json!({ "path": (self.artifact_path)(&artifact) }))
    }

    fn push_plot(&mut self, path: PathBuf) -> Result<()> {
        let artifact = normalize_plot_path(
            &self.chunk.label,
            self.figures_dir,
            self.figure,
            &path,
            self.plot_index,
        )
        .map_err(|err| {
            anyhow!(
                "failed to normalize plot artifact path for chunk `{}`: {err}",
                self.chunk.label
            )
        })?;
        self.plot_index += 1;
        self.items.push(ResultItem::rich_data(
            ResultItemType::Display,
            self.figure.mime_type().to_string(),
            json!({ "path": (self.artifact_path)(&artifact) }),
        ));
        Ok(())
    }
}

fn normalize_plot_path(
    label: &str,
    figures_dir: &Path,
    figure: &FigureSpec,
    path: &Path,
    index: usize,
) -> Result<PathBuf> {
    let target = figures_dir.join(plot_artifact_filename(label, figure, index));
    if !path.exists() {
        return Err(anyhow!("plot artifact `{}` does not exist", path.display()));
    }
    validate_plot_artifact_format(path, figure, label)?;
    if plot_paths_equivalent(path, &target)? {
        return Ok(target);
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::copy(path, &target).with_context(|| {
        format!(
            "failed to copy plot artifact `{}` to `{}`",
            path.display(),
            target.display()
        )
    })?;
    std::fs::remove_file(path)
        .with_context(|| format!("failed to remove source plot artifact `{}`", path.display()))?;
    Ok(target)
}

fn plot_paths_equivalent(source: &Path, target: &Path) -> Result<bool> {
    if source == target {
        return Ok(true);
    }
    if !target.exists() {
        return Ok(false);
    }

    let source = std::fs::canonicalize(source)
        .with_context(|| format!("failed to resolve plot artifact `{}`", source.display()))?;
    let target = std::fs::canonicalize(target)
        .with_context(|| format!("failed to resolve plot artifact `{}`", target.display()))?;
    Ok(source == target)
}

fn plot_artifact_filename(label: &str, figure: &FigureSpec, index: usize) -> String {
    if index <= 1 {
        figure.artifact_filename(label)
    } else {
        format!("{label}-{index}.{}", figure.extension())
    }
}

fn validate_plot_artifact_format(path: &Path, figure: &FigureSpec, label: &str) -> Result<()> {
    let Some(actual) = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
    else {
        return Err(anyhow!(
            "plot artifact format mismatch for chunk `{label}`: expected .{}, got no extension from `{}`",
            figure.extension(),
            path.display()
        ));
    };
    if plot_extensions_match(&actual, figure.extension()) {
        return Ok(());
    }
    Err(anyhow!(
        "plot artifact format mismatch for chunk `{label}`: expected .{}, got .{} from `{}`",
        figure.extension(),
        actual,
        path.display()
    ))
}

fn plot_extensions_match(actual: &str, expected: &str) -> bool {
    if actual == expected {
        return true;
    }
    matches!((actual, expected), ("jpeg", "jpg") | ("jpg", "jpeg"))
}

fn lines(code: &str) -> Vec<String> {
    code.lines().map(ToOwned::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::model::{ExecOptions, ResultsMode};
    use crate::typst::testfixtures;
    use crate::utils::testutil::command_available;

    fn chunk(results: ResultsMode) -> ChunkSpec {
        let mut chunk = testfixtures::chunk("fig-demo", "x <- 1", results);
        chunk.engine = EngineName::R;
        chunk
    }

    fn figure_for(chunk: &ChunkSpec) -> FigureSpec {
        FigureSpec::from_exec_options(&chunk.engine, &chunk.exec_options).unwrap()
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
    fn typst_results_number_multiple_stdout_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let chunk = chunk(ResultsMode::Typst);
        let figure = figure_for(&chunk);
        let items = normalize_engine_results(
            &chunk,
            dir.path(),
            &figure,
            vec![
                EngineResult::Output("#table()[first]".to_string()),
                EngineResult::Output("#table()[second]".to_string()),
            ],
            |path| path.file_name().unwrap().to_string_lossy().to_string(),
        )
        .unwrap();

        assert_eq!(
            items[0].data.as_ref().unwrap()["text/x-typst"]["path"],
            "fig-demo.typ"
        );
        assert_eq!(
            items[1].data.as_ref().unwrap()["text/x-typst"]["path"],
            "fig-demo-2.typ"
        );
        assert!(std::fs::read_to_string(dir.path().join("fig-demo.typ"))
            .unwrap()
            .contains("first"));
        assert!(std::fs::read_to_string(dir.path().join("fig-demo-2.typ"))
            .unwrap()
            .contains("second"));
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
    fn normalizes_arbitrary_plot_names_by_result_order() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("plot-alpha.svg");
        let second = dir.path().join("plot-beta.svg");
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

        assert_eq!(
            items[0].data.as_ref().unwrap()["image/svg+xml"]["path"],
            "fig-demo.svg"
        );
        assert_eq!(
            items[1].data.as_ref().unwrap()["image/svg+xml"]["path"],
            "fig-demo-2.svg"
        );
        assert!(std::fs::read_to_string(dir.path().join("fig-demo.svg"))
            .unwrap()
            .contains("first"));
        assert!(std::fs::read_to_string(dir.path().join("fig-demo-2.svg"))
            .unwrap()
            .contains("second"));
    }

    #[test]
    fn rejects_plot_artifacts_with_mismatched_extension() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("fig-demo-1.png");
        std::fs::write(&source, "png bytes").unwrap();
        let chunk = chunk(ResultsMode::Verbatim);
        let figure = figure_for(&chunk);

        let err = normalize_engine_results(
            &chunk,
            dir.path(),
            &figure,
            vec![EngineResult::Plot(source)],
            |_| "unused".to_string(),
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("plot artifact format"), "{err}");
        assert!(err.contains("fig-demo"), "{err}");
    }

    #[test]
    fn rejects_plot_artifacts_without_extension() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("fig-demo-1");
        std::fs::write(&source, "svg bytes").unwrap();
        let chunk = chunk(ResultsMode::Verbatim);
        let figure = figure_for(&chunk);

        let err = normalize_engine_results(
            &chunk,
            dir.path(),
            &figure,
            vec![EngineResult::Plot(source)],
            |_| "unused".to_string(),
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("plot artifact format"), "{err}");
        assert!(err.contains("expected .svg"), "{err}");
        assert!(err.contains("no extension"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn accepts_canonically_equivalent_plot_source_and_target_paths() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("fig-demo.svg");
        let source = dir.path().join("alias.svg");
        std::fs::write(&target, "<svg id=\"kept\"></svg>").unwrap();
        symlink(&target, &source).unwrap();
        let chunk = chunk(ResultsMode::Verbatim);
        let figure = figure_for(&chunk);

        let items = normalize_engine_results(
            &chunk,
            &dir.path().join("."),
            &figure,
            vec![EngineResult::Plot(source)],
            |path| path.file_name().unwrap().to_string_lossy().to_string(),
        )
        .unwrap();

        assert_eq!(
            items[0].data.as_ref().unwrap()["image/svg+xml"]["path"],
            "fig-demo.svg"
        );
        assert!(
            std::fs::read_to_string(&target).unwrap().contains("kept"),
            "source and target aliases should not be copied over themselves"
        );
    }

    #[test]
    fn missing_plot_artifact_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("fig-demo-1.svg");
        let chunk = chunk(ResultsMode::Verbatim);
        let figure = figure_for(&chunk);

        let err = normalize_engine_results(
            &chunk,
            dir.path(),
            &figure,
            vec![EngineResult::Plot(missing)],
            |_| "unused".to_string(),
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("plot artifact"), "{err}");
        assert!(err.contains("fig-demo"), "{err}");
    }

    #[test]
    fn diagram_engines_always_use_svg_figures() {
        assert_eq!(
            FigureSpec::from_exec_options(
                &EngineName::from_name("mermaid"),
                &ExecOptions {
                    fig_device_format: "png".to_string(),
                    ..chunk(ResultsMode::Verbatim).exec_options
                }
            )
            .unwrap()
            .extension(),
            "svg"
        );
        assert_eq!(
            FigureSpec::from_exec_options(
                &EngineName::from_name("tikz"),
                &ExecOptions {
                    fig_device_format: "pdf".to_string(),
                    ..chunk(ResultsMode::Verbatim).exec_options
                }
            )
            .unwrap()
            .extension(),
            "svg"
        );
        assert_eq!(
            FigureSpec::from_exec_options(
                &EngineName::R,
                &ExecOptions {
                    fig_device_format: "png".to_string(),
                    ..chunk(ResultsMode::Verbatim).exec_options
                }
            )
            .unwrap()
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
            params: Value::Object(serde_json::Map::new()),
            params_path: None,
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

    fn pool_with_params(dir: &Path, params: Value) -> EnginePool {
        EnginePool::new(ExecutionConfig {
            cwd: dir.to_path_buf(),
            executables: ExecutablePaths::defaults(),
            timeout: Some(std::time::Duration::from_secs(20)),
            params,
            params_path: None,
        })
    }

    fn run_chunk(pool: &mut EnginePool, dir: &Path, engine: EngineName, code: &str) -> String {
        let mut chunk = chunk(ResultsMode::Verbatim);
        chunk.engine = engine;
        chunk.label = "params-chunk".to_string();
        chunk.code = code.to_string();
        let result = pool
            .execute_chunk(&chunk, dir, |_| "unused".to_string())
            .unwrap();
        result
            .items
            .iter()
            .filter_map(|item| item.text.clone())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn r_engine_reads_injected_params() {
        if !command_available("Rscript") {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let mut pool = pool_with_params(
            dir.path(),
            serde_json::json!({"label": "baseline", "alpha": 0.1, "n": 3, "flag": true}),
        );
        let out = run_chunk(
            &mut pool,
            dir.path(),
            EngineName::R,
            "cat(params$label, params$alpha, params$n, params$flag)",
        );
        assert!(out.contains("baseline"), "{out:?}");
        assert!(out.contains("0.1"), "{out:?}");
        assert!(out.contains('3'), "{out:?}");
        assert!(out.contains("TRUE"), "{out:?}");
    }

    #[test]
    fn r_params_with_quotes_do_not_break_injection() {
        if !command_available("Rscript") {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let mut pool = pool_with_params(dir.path(), serde_json::json!({"q": "a\"b"}));
        let out = run_chunk(&mut pool, dir.path(), EngineName::R, "cat(params$q)");
        assert!(out.contains("a\"b"), "{out:?}");
    }

    #[test]
    fn python_engine_reads_injected_params() {
        if !command_available("python3") {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let mut pool = pool_with_params(
            dir.path(),
            serde_json::json!({"label": "baseline", "years": [2020, 2021], "active": true}),
        );
        let out = run_chunk(
            &mut pool,
            dir.path(),
            EngineName::Python,
            "print(params['label'], params['years'][1], params['active'])",
        );
        assert!(out.contains("baseline"), "{out:?}");
        assert!(out.contains("2021"), "{out:?}");
        assert!(out.contains("True"), "{out:?}");
    }

    #[test]
    fn empty_params_inject_nothing() {
        if !command_available("python3") {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        // No params: a chunk referencing `params` should raise a NameError,
        // proving nothing was injected.
        let mut pool = pool_with_params(dir.path(), serde_json::json!({}));
        let mut chunk = chunk(ResultsMode::Verbatim);
        chunk.engine = EngineName::Python;
        chunk.label = "no-params".to_string();
        chunk.code = "print(params)".to_string();
        let err = pool
            .execute_chunk(&chunk, dir.path(), |_| "unused".to_string())
            .unwrap_err()
            .to_string();
        assert!(err.contains("NameError") || err.contains("params"), "{err}");
    }
}
