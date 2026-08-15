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
    artifact_label_stem, ChunkResultDocument, ChunkSpec, ChunkStatus, DiagnosticLevel, EngineName,
    FigureSpec, ResultItem, ResultItemName, ResultItemType, ResultsMode, DEFAULT_FIG_DEVICE_ASPECT,
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
    pub store: serde_json::Map<String, Value>,
}

/// A prelude that errored is a Calepin bug (we generate the literal), so surface
/// the raw engine output rather than letting later chunks fail mysteriously.
///
/// The error tag is matched with its per-run sentinel prefix so a variable
/// string value that happens to contain `_ERROR:` cannot trigger a false alarm
/// (the engine echoes the prelude source back as a tagged line).
fn check_prelude_output(raw: &str, engine: &str) -> Result<()> {
    let sentinel = raw.split_once('\n').map_or("", |(first, _)| first);
    if !sentinel.is_empty() && raw.contains(&format!("{sentinel}_ERROR:")) {
        return Err(anyhow!(
            "failed to inject document variables into the {engine} engine: {}",
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
    store: serde_json::Map<String, Value>,
}

impl EnginePool {
    pub fn new(config: ExecutionConfig) -> Self {
        let store = config.store.clone();
        Self {
            r: None,
            python: None,
            jupyter: None,
            config,
            store,
        }
    }

    pub fn store(&self) -> &serde_json::Map<String, Value> {
        &self.store
    }

    pub fn execute_chunk(
        &mut self,
        chunk: &ChunkSpec,
        figures_dir: &Path,
        artifact_path: impl Fn(&Path) -> Result<String>,
    ) -> Result<ChunkResultDocument> {
        if !chunk.exec_options.eval {
            return Ok(chunk_result_document(
                chunk,
                ChunkStatus::Skipped,
                Vec::new(),
            ));
        }
        self.validate_store_options(chunk)?;
        self.inject_store_values(chunk)?;

        let engine = chunk.engine.clone();
        let source = lines(&chunk.code);
        let figure = FigureSpec::from_exec_options(&engine, &chunk.exec_options)
            .map_err(|err| anyhow!("{}: {err}", execution_context(chunk)))?;
        let engine_results =
            self.run_engine_results(chunk, figures_dir, engine, &source, &figure)?;
        if engine_results_unavailable(&engine_results) {
            return Ok(unavailable_chunk_result_document(chunk));
        }
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
        if has_error && (!chunk.exec_options.error || !chunk.exec_options.store_set.is_empty()) {
            let message = items
                .iter()
                .find(|item| item.item_type == ResultItemType::Error)
                .and_then(|item| item.message.as_deref())
                .unwrap_or("execution failed");
            return Err(anyhow!("chunk `{}` failed: {}", chunk.label, message));
        }
        if !has_error {
            self.capture_store_values(chunk)?;
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
            let fig_path = figures_dir.join(figure.numbered_filename(&chunk.label));
            engines::diagram::execute_diagram(
                &chunk.code,
                engine.clone(),
                &fig_path,
                source,
                &self.config.executables,
            )
            .map_err(|err| anyhow!("{}: {err}", execution_context(chunk)))
        } else {
            let mut ctx = match self.context_for(engine.clone()) {
                Ok(ctx) => ctx,
                Err(err) if is_unavailable_engine_error(&err) => {
                    return Ok(vec![EngineResult::Unavailable(err.to_string())]);
                }
                Err(err) => return Err(anyhow!("{}: {err}", execution_context(chunk))),
            };
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
            let session = RSession::init_with_program(
                &self.config.executables.rscript,
                "typst",
                Some(&self.config.cwd),
                self.config.timeout,
            )?;
            self.r = Some(session);
        }
        Ok(())
    }

    fn ensure_python_session(&mut self) -> Result<()> {
        if self.python.is_none() {
            let session = PythonSession::init_with_program(
                &self.config.executables.python,
                Some(&self.config.cwd),
                self.config.timeout,
            )?;
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

    fn validate_store_options(&self, chunk: &ChunkSpec) -> Result<()> {
        if chunk.exec_options.store_get.is_empty() && chunk.exec_options.store_set.is_empty() {
            return Ok(());
        }
        if !matches!(chunk.engine, EngineName::R | EngineName::Python) {
            return Err(anyhow!(
                "chunk `{}` declares a document store option, but engine `{}` does not support the Calepin document store",
                chunk.label,
                chunk.engine
            ));
        }
        for key in &chunk.exec_options.store_get {
            if !self.store.contains_key(key) {
                return Err(anyhow!(
                    "chunk `{}` requests store key `{key}`, but no earlier writer has committed that key",
                    chunk.label
                ));
            }
        }
        for key in &chunk.exec_options.store_set {
            if self.store.contains_key(key) {
                return Err(anyhow!(
                    "chunk `{}` cannot set store key `{key}` because it is already initialized or committed",
                    chunk.label
                ));
            }
        }
        Ok(())
    }

    fn inject_store_values(&mut self, chunk: &ChunkSpec) -> Result<()> {
        if chunk.exec_options.store_get.is_empty() {
            return Ok(());
        }
        let values = Value::Object(
            chunk
                .exec_options
                .store_get
                .iter()
                .map(|key| (key.clone(), self.store[key].clone()))
                .collect(),
        );
        match chunk.engine {
            EngineName::R => {
                self.ensure_r_session()?;
                let session = self.r.as_mut().expect("R session initialized");
                for (key, value) in values.as_object().expect("store values object") {
                    inject_r_binding(session, key, value)?;
                }
            }
            EngineName::Python => {
                self.ensure_python_session()?;
                let session = self.python.as_mut().expect("Python session initialized");
                for (key, value) in values.as_object().expect("store values object") {
                    inject_python_binding(session, key, value)?;
                }
            }
            _ => unreachable!("store engine validated"),
        }
        Ok(())
    }

    fn capture_store_values(&mut self, chunk: &ChunkSpec) -> Result<()> {
        if chunk.exec_options.store_set.is_empty() {
            return Ok(());
        }
        let captured = match chunk.engine {
            EngineName::Python => {
                self.ensure_python_session()?;
                capture_python_store(
                    self.python.as_mut().expect("Python session initialized"),
                    &chunk.exec_options.store_set,
                )
            }
            EngineName::R => {
                self.ensure_r_session()?;
                capture_r_store(
                    self.r.as_mut().expect("R session initialized"),
                    &chunk.exec_options.store_set,
                )
            }
            _ => unreachable!("store engine validated"),
        }
        .map_err(|error| {
            anyhow!(
                "cannot set store values from chunk `{}`: {error}",
                chunk.label
            )
        })?;
        for key in &chunk.exec_options.store_set {
            if !captured.contains_key(key) {
                return Err(anyhow!(
                    "chunk `{}` declares store-set `{key}`, but the {} session has no object named `{key}` after the chunk completed",
                    chunk.label,
                    chunk.engine
                ));
            }
        }
        crate::typst::store::validate_writer_values(&captured)?;
        let mut committed = self.store.clone();
        committed.extend(captured);
        crate::typst::store::validate_store(&committed)?;
        self.store = committed;
        Ok(())
    }
}

fn execution_context(chunk: &ChunkSpec) -> String {
    format!(
        "failed to execute chunk `{}` with engine `{}`",
        chunk.label, chunk.engine
    )
}

fn engine_results_unavailable(results: &[EngineResult]) -> bool {
    results
        .iter()
        .any(|result| matches!(result, EngineResult::Unavailable(_)))
}

/// The engines Calepin documents as supported, whatever carries them. `julia`
/// and `sh`/`bash` have no named `EngineName` variant because they ride the
/// Jupyter bridge like any third-party kernel, so they have to be named here.
///
/// A fence may pin a version (`julia-1`, `julia-1.11`), so the kernel name is
/// compared on the part before the first `-`.
fn is_documented_engine(engine: &EngineName) -> bool {
    match engine {
        EngineName::R | EngineName::Python | EngineName::Diagram(_) => true,
        EngineName::Jupyter(kernel) => {
            let base = kernel
                .split_once('-')
                .map_or(kernel.as_str(), |(base, _)| base);
            matches!(base, "julia" | "sh" | "bash")
        }
    }
}

fn unavailable_chunk_result_document(chunk: &ChunkSpec) -> ChunkResultDocument {
    let mut display_options = chunk.display_options.clone();
    display_options.echo = true;
    // A missing engine means two different things, and the difference is what
    // the author intended, not how Calepin routes the language internally.
    //
    // A fence in one of the documented engines was written to run: the tool is
    // merely absent, so the block stays a chunk and echoes its source rather
    // than disappearing into prose. Whether the engine is built in or reached
    // through the Jupyter bridge is an implementation detail that must not
    // change what the page looks like.
    //
    // Any other tag reached us because Calepin looks up unrecognised fence
    // languages as kernel names. With no such kernel installed, ```rust or
    // ```json is code the author only meant to display, so tell the runtime it
    // is not a chunk at all (issue #108).
    //
    // The gap this leaves is deliberate: a ```ruby fence on a machine that lost
    // its kernel is indistinguishable from prose without probing
    // `jupyter kernelspec list` on every build, which is not a cost worth
    // paying here.
    let status = if is_documented_engine(&chunk.engine) {
        ChunkStatus::Skipped
    } else {
        ChunkStatus::Unavailable
    };
    ChunkResultDocument {
        label: chunk.label.clone(),
        engine: chunk.engine.clone(),
        source: chunk.code.clone(),
        status,
        display_options,
        items: Vec::new(),
        crossref_labels: chunk.crossref_labels.clone(),
    }
}

fn is_unavailable_engine_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        let message = cause.to_string();
        message.contains("executable not found")
            || message.contains("not found on PATH")
            || message.contains("jupyter_client Python package not found")
    })
}

fn inject_r_binding(session: &mut RSession, name: &str, value: &Value) -> Result<()> {
    let code = format!(
        "assign({}, {}, envir=globalenv(), inherits=FALSE)",
        engines::prelude::r_value(&Value::String(name.to_string())),
        engines::prelude::r_store_value(value)?
    );
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

fn inject_python_binding(session: &mut PythonSession, name: &str, value: &Value) -> Result<()> {
    let code = format!(
        "globals()[{}] = {}",
        engines::prelude::python_value(&Value::String(name.to_string())),
        engines::prelude::python_value(value)
    );
    let raw = session.capture(
        &code,
        "",
        PRELUDE_FIG_WIDTH,
        PRELUDE_FIG_HEIGHT,
        PRELUDE_FIG_DPI,
    )?;
    check_prelude_output(&raw, "Python")
}

fn capture_outputs(raw: &str) -> Result<Vec<String>> {
    let mut results = Vec::new();
    engines::process_results(raw, Path::new(""), &mut results)?;
    let mut outputs = Vec::new();
    for result in results {
        match result {
            EngineResult::Output(text) => outputs.push(text),
            EngineResult::Error(message) => return Err(anyhow!(message)),
            _ => {}
        }
    }
    Ok(outputs)
}

fn capture_python_store(
    session: &mut PythonSession,
    keys: &[String],
) -> Result<serde_json::Map<String, Value>> {
    let keys = serde_json::to_string(keys)?;
    let code = format!(
        r#"import json as _calepin_json, math as _calepin_math
def _calepin_store_value(v, seen=None):
    if seen is None: seen = set()
    if v is None or type(v) in (bool, str): return v
    if type(v) is int:
        if not -(2**63) <= v < 2**63: raise TypeError("integer outside signed 64-bit range")
        return v
    if type(v) is float:
        if not _calepin_math.isfinite(v): raise TypeError("non-finite number")
        return v
    if type(v) in (list, dict):
        if id(v) in seen: raise TypeError("serialization cycle")
        seen.add(id(v))
        if type(v) is list: out = [_calepin_store_value(x, seen) for x in v]
        else:
            if any(type(k) is not str for k in v): raise TypeError("mapping keys must be strings")
            out = {{k: _calepin_store_value(x, seen) for k, x in v.items()}}
        seen.remove(id(v))
        return out
    raise TypeError("the value is outside the supported Python store value model")
_calepin_keys = {keys}
_calepin_missing = [k for k in _calepin_keys if k not in globals()]
if _calepin_missing: raise NameError("missing store variables: " + ", ".join(_calepin_missing))
print(_calepin_json.dumps({{k: _calepin_store_value(globals()[k]) for k in _calepin_keys}}, separators=(",", ":")))"#
    );
    let raw = session.capture(
        &code,
        "",
        PRELUDE_FIG_WIDTH,
        PRELUDE_FIG_HEIGHT,
        PRELUDE_FIG_DPI,
    )?;
    let text = capture_outputs(&raw)?
        .pop()
        .ok_or_else(|| anyhow!("Python store adapter returned no value"))?;
    serde_json::from_str(text.trim()).context("Python store adapter returned invalid JSON")
}

fn capture_r_store(
    session: &mut RSession,
    keys: &[String],
) -> Result<serde_json::Map<String, Value>> {
    let keys = engines::prelude::r_value(&Value::Array(
        keys.iter().cloned().map(Value::String).collect(),
    ));
    let code = format!(
        r#".calepin_json <- function(x) {{
  q <- function(s) encodeString(s, quote="\"", na.encode=FALSE)
  if (is.null(x)) return("null")
  if (is.logical(x) && length(x)==1L && !is.na(x)) return(if (x) "true" else "false")
  if (is.integer(x) && length(x)==1L && !is.na(x)) return(as.character(x))
  if (is.double(x) && length(x)==1L && is.finite(x)) {{
    out <- sprintf("%.17g", x)
    if (!grepl("[.eE]", out)) out <- paste0(out, ".0")
    return(out)
  }}
  if (is.character(x) && !anyNA(x) && (length(x)==0L || length(x)>1L))
    return(paste0("[", paste(vapply(x, q, ""), collapse=","), "]"))
  if ((is.integer(x) || is.double(x)) && !anyNA(x) && (length(x)==0L || length(x)>1L)) {{
    if (is.double(x) && any(!is.finite(x))) stop("non-finite number")
    return(paste0("[", paste(vapply(as.list(x), .calepin_json, ""), collapse=","), "]"))
  }}
  if (is.character(x) && length(x)==1L && !is.na(x)) return(q(x))
  if (is.list(x) && length(x)>0L && !is.null(names(x)) && all(nzchar(names(x))) && !anyDuplicated(names(x)))
    return(paste0("{{", paste(vapply(seq_along(x), function(i) paste0(q(names(x)[i]), ":", .calepin_json(x[[i]])), ""), collapse=","), "}}"))
  stop("the value is outside the supported R store value model")
}}
.calepin_keys <- {keys}
.calepin_missing <- .calepin_keys[!vapply(.calepin_keys, exists, FALSE, envir=globalenv(), inherits=FALSE)]
if (length(.calepin_missing)) stop(paste("missing store variables:", paste(.calepin_missing, collapse=", ")))
cat(paste0("{{", paste(vapply(.calepin_keys, function(k) paste0(encodeString(k, quote="\""), ":", .calepin_json(get(k, envir=globalenv(), inherits=FALSE))), ""), collapse=","), "}}"))"#
    );
    let raw = session.capture(
        &code,
        "",
        "svg",
        PRELUDE_FIG_WIDTH,
        PRELUDE_FIG_HEIGHT,
        PRELUDE_FIG_DPI,
    )?;
    let text = capture_outputs(&raw)?
        .pop()
        .ok_or_else(|| anyhow!("R store adapter returned no value"))?;
    serde_json::from_str(text.trim()).context("R store adapter returned invalid JSON")
}

fn chunk_result_document(
    chunk: &ChunkSpec,
    status: ChunkStatus,
    items: Vec<ResultItem>,
) -> ChunkResultDocument {
    ChunkResultDocument {
        label: chunk.label.clone(),
        engine: chunk.engine.clone(),
        source: chunk.code.clone(),
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
    artifact_path: impl Fn(&Path) -> Result<String>,
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
    F: Fn(&Path) -> Result<String>,
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
    F: Fn(&Path) -> Result<String>,
{
    fn normalize(mut self, engine_results: Vec<EngineResult>) -> Result<Vec<ResultItem>> {
        for result in engine_results {
            match result {
                EngineResult::Source(_)
                | EngineResult::Unavailable(_)
                | EngineResult::Preamble(_) => {}
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
        let label_stem = artifact_label_stem(&self.chunk.label);
        let filename = if self.typst_result_index == 1 {
            format!("{label_stem}.typ")
        } else {
            format!("{label_stem}-{}.typ", self.typst_result_index)
        };
        self.typst_result_index += 1;
        let artifact = self.figures_dir.join(filename);
        write_if_changed(&artifact, text)
            .with_context(|| format!("failed to write Typst result {}", artifact.display()))?;
        Ok(json!({ "path": (self.artifact_path)(&artifact)? }))
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
            json!({ "path": (self.artifact_path)(&artifact)? }),
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
        format!(
            "{}-{index}.{}",
            artifact_label_stem(label),
            figure.extension()
        )
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

    fn unused_artifact_path(_: &Path) -> Result<String> {
        Ok("unused".to_string())
    }

    fn artifact_file_name(path: &Path) -> Result<String> {
        Ok(path.file_name().unwrap().to_string_lossy().to_string())
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
            unused_artifact_path,
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
            artifact_file_name,
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
            artifact_file_name,
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
    fn path_like_labels_stay_inside_the_figures_directory() {
        let dir = tempfile::tempdir().unwrap();
        let mut chunk = chunk(ResultsMode::Typst);
        chunk.label = "../outside/%2F".to_string();
        let figure = figure_for(&chunk);
        let items = normalize_engine_results(
            &chunk,
            dir.path(),
            &figure,
            vec![EngineResult::Output("#table()[safe]".to_string())],
            artifact_file_name,
        )
        .unwrap();

        let filename = "..%2Foutside%2F%252F.typ";
        assert_eq!(
            items[0].data.as_ref().unwrap()["text/x-typst"]["path"],
            filename
        );
        assert!(dir.path().join(filename).is_file());
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
            unused_artifact_path,
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
            artifact_file_name,
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
            artifact_file_name,
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
    fn path_like_labels_produce_safe_plot_artifact_names() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.svg");
        std::fs::write(&source, "<svg></svg>").unwrap();
        let mut chunk = chunk(ResultsMode::Verbatim);
        chunk.label = "../outside/%2F".to_string();
        let figure = figure_for(&chunk);

        let items = normalize_engine_results(
            &chunk,
            dir.path(),
            &figure,
            vec![EngineResult::Plot(source)],
            artifact_file_name,
        )
        .unwrap();

        let filename = "..%2Foutside%2F%252F.svg";
        assert_eq!(
            items[0].data.as_ref().unwrap()["image/svg+xml"]["path"],
            filename
        );
        assert!(dir.path().join(filename).is_file());
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
            artifact_file_name,
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
            unused_artifact_path,
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
            unused_artifact_path,
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
            artifact_file_name,
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
            unused_artifact_path,
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
    fn engine_pool_falls_back_when_jupyter_bridge_is_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let missing_python = dir.path().join("missing-python3");
        let mut executables = ExecutablePaths::defaults();
        executables.python = missing_python.clone();
        let config = ExecutionConfig {
            cwd: dir.path().to_path_buf(),
            executables,
            timeout: Some(std::time::Duration::from_secs(5)),
            store: serde_json::Map::new(),
        };
        let mut pool = EnginePool::new(config);
        let mut octave_chunk = chunk(ResultsMode::Verbatim);
        octave_chunk.engine = EngineName::Jupyter("octave".to_string());
        octave_chunk.label = "octave-test".to_string();
        octave_chunk.code = "disp(42)".to_string();
        let result = pool.execute_chunk(&octave_chunk, dir.path(), unused_artifact_path);
        let result = result.unwrap();
        assert_eq!(result.status, ChunkStatus::Unavailable);
        assert!(result.display_options.echo);
        assert!(result.items.is_empty());
    }

    /// A missing engine keeps the block a chunk when the document plainly meant
    /// it to run, and demotes it to prose when the language only reached us by
    /// being looked up as a kernel name. Which side a language falls on must not
    /// depend on whether it happens to ride the Jupyter bridge.
    #[test]
    fn missing_engine_demotes_only_undocumented_languages_to_prose() {
        for name in [
            "r",
            "python",
            "mermaid",
            "julia",
            "julia-1.11",
            "sh",
            "bash",
        ] {
            assert!(
                is_documented_engine(&EngineName::from_name(name)),
                "`{name}` is a documented engine and should stay a chunk"
            );
        }
        for name in ["rust", "json", "text", "ruby"] {
            assert!(
                !is_documented_engine(&EngineName::from_name(name)),
                "`{name}` is a bare fence tag and should render as prose"
            );
        }
    }

    #[test]
    fn engine_pool_falls_back_when_builtin_engine_is_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let missing_python = dir.path().join("missing-python3");
        let mut executables = ExecutablePaths::defaults();
        executables.python = missing_python;
        let config = ExecutionConfig {
            cwd: dir.path().to_path_buf(),
            executables,
            timeout: Some(std::time::Duration::from_secs(5)),
            store: serde_json::Map::new(),
        };
        let mut pool = EnginePool::new(config);
        let mut python_chunk = chunk(ResultsMode::Verbatim);
        python_chunk.engine = EngineName::Python;
        python_chunk.label = "python-test".to_string();
        python_chunk.code = "print(42)".to_string();

        let result = pool
            .execute_chunk(&python_chunk, dir.path(), unused_artifact_path)
            .unwrap();

        assert_eq!(result.status, ChunkStatus::Skipped);
        assert!(result.display_options.echo);
        assert!(result.items.is_empty());
    }

    #[test]
    fn engine_pool_preserves_available_engine_code_errors() {
        if !command_available("python3") {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let mut executables = ExecutablePaths::defaults();
        executables.python = PathBuf::from("python3");
        let config = ExecutionConfig {
            cwd: dir.path().to_path_buf(),
            executables,
            timeout: Some(std::time::Duration::from_secs(5)),
            store: serde_json::Map::new(),
        };
        let mut pool = EnginePool::new(config);
        let mut python_chunk = chunk(ResultsMode::Verbatim);
        python_chunk.engine = EngineName::Python;
        python_chunk.label = "python-error-test".to_string();
        python_chunk.code = "raise RuntimeError('boom')".to_string();

        let err = pool
            .execute_chunk(&python_chunk, dir.path(), unused_artifact_path)
            .unwrap_err()
            .to_string();

        assert!(err.contains("chunk `python-error-test` failed"), "{err}");
        assert!(err.contains("boom"), "{err}");
    }

    #[test]
    fn writer_error_commits_nothing_even_when_errors_are_displayable() {
        if !command_available("python3") {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let mut executables = ExecutablePaths::defaults();
        executables.python = PathBuf::from("python3");
        let mut pool = EnginePool::new(ExecutionConfig {
            cwd: dir.path().to_path_buf(),
            executables,
            timeout: Some(std::time::Duration::from_secs(5)),
            store: serde_json::Map::new(),
        });
        let mut writer = chunk(ResultsMode::Verbatim);
        writer.engine = EngineName::Python;
        writer.label = "failing-writer".to_string();
        writer.code = "answer = 42\nraise RuntimeError('boom')".to_string();
        writer.exec_options.error = true;
        writer.exec_options.store_set = vec!["answer".to_string()];

        let error = pool
            .execute_chunk(&writer, dir.path(), unused_artifact_path)
            .unwrap_err()
            .to_string();

        assert!(error.contains("chunk `failing-writer` failed"), "{error}");
        assert!(!pool.store().contains_key("answer"));
    }

    fn pool_with_vars(dir: &Path, vars: Value) -> EnginePool {
        EnginePool::new(ExecutionConfig {
            cwd: dir.to_path_buf(),
            executables: ExecutablePaths::defaults(),
            timeout: Some(std::time::Duration::from_secs(20)),
            store: vars.as_object().cloned().unwrap_or_default(),
        })
    }

    fn run_chunk(pool: &mut EnginePool, dir: &Path, engine: EngineName, code: &str) -> String {
        let mut chunk = chunk(ResultsMode::Verbatim);
        chunk.engine = engine;
        chunk.label = "vars-chunk".to_string();
        chunk.code = code.to_string();
        chunk.exec_options.store_get = pool.store().keys().cloned().collect();
        let result = pool
            .execute_chunk(&chunk, dir, unused_artifact_path)
            .unwrap();
        result
            .items
            .iter()
            .filter_map(|item| item.text.clone())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn r_engine_reads_injected_vars() {
        if !command_available("Rscript") {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let mut pool = pool_with_vars(
            dir.path(),
            serde_json::json!({"label": "baseline", "alpha": 0.1, "n": 3, "flag": true}),
        );
        let out = run_chunk(
            &mut pool,
            dir.path(),
            EngineName::R,
            "cat(label, alpha, n, flag)",
        );
        assert!(out.contains("baseline"), "{out:?}");
        assert!(out.contains("0.1"), "{out:?}");
        assert!(out.contains('3'), "{out:?}");
        assert!(out.contains("TRUE"), "{out:?}");
    }

    #[test]
    fn r_vars_with_quotes_do_not_break_injection() {
        if !command_available("Rscript") {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let mut pool = pool_with_vars(dir.path(), serde_json::json!({"q": "a\"b"}));
        let out = run_chunk(&mut pool, dir.path(), EngineName::R, "cat(q)");
        assert!(out.contains("a\"b"), "{out:?}");
    }

    #[test]
    fn python_engine_reads_injected_vars() {
        if !command_available("python3") {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let mut pool = pool_with_vars(
            dir.path(),
            serde_json::json!({"label": "baseline", "years": [2020, 2021], "active": true}),
        );
        let out = run_chunk(
            &mut pool,
            dir.path(),
            EngineName::Python,
            "print(label, years[1], active)",
        );
        assert!(out.contains("baseline"), "{out:?}");
        assert!(out.contains("2021"), "{out:?}");
        assert!(out.contains("True"), "{out:?}");
    }

    #[test]
    fn empty_vars_inject_nothing() {
        if !command_available("python3") {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        // No vars: Python should still see its built-in `vars` function,
        // proving Calepin did not inject a document mapping.
        let mut pool = pool_with_vars(dir.path(), serde_json::json!({}));
        let mut chunk = chunk(ResultsMode::Verbatim);
        chunk.engine = EngineName::Python;
        chunk.label = "no-vars".to_string();
        chunk.code = "print(vars)".to_string();
        let result = pool
            .execute_chunk(&chunk, dir.path(), unused_artifact_path)
            .unwrap();
        let out = result
            .items
            .iter()
            .filter_map(|item| item.text.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(out.contains("<built-in function vars>"), "{out:?}");
    }
}
