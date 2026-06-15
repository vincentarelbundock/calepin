pub mod diagram;
pub mod jupyter;
pub mod prelude;
pub mod python;
pub mod r;
pub mod subprocess;

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

use crate::engines::jupyter::JupyterCapture;
use crate::typst::model::{EngineName, FigureSpec};

pub(crate) const META_PREFIX: &str = "META:";

pub(crate) fn build_payload(meta: Value, code: &str) -> Result<String> {
    Ok(format!("{}\n{}", format_meta_payload(meta)?, code))
}

fn format_meta_payload(meta: Value) -> Result<String> {
    let encoded = serde_json::to_string(&meta).context("serialize engine meta payload")?;
    Ok(format!("{META_PREFIX}{encoded}"))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum EngineResult {
    Source(Vec<String>),
    Output(String),
    Warning(String),
    Message(String),
    Error(String),
    Plot(PathBuf),
    Preamble(String),
}

/// Holds mutable references to active engine sessions.
pub struct EngineContext<'a> {
    pub r: Option<&'a mut r::RSession>,
    pub python: Option<&'a mut python::PythonSession>,
    pub jupyter: Option<&'a mut jupyter::JupyterBridgeSession>,
}

/// Execute a Typst chunk and capture its output.
pub fn execute_chunk(
    source: &[String],
    engine: EngineName,
    label: &str,
    fig_dir: &Path,
    figure: &FigureSpec,
    ctx: &mut EngineContext,
) -> Result<Vec<EngineResult>> {
    let code = source.join("\n");
    let mut results = Vec::new();

    let is_table_chunk = label.starts_with("tbl-");
    std::fs::create_dir_all(fig_dir)?;
    let fig_full_path = fig_dir.join(figure.numbered_filename(label));
    let fig_abs = if fig_full_path.is_relative() {
        std::env::current_dir()?.join(&fig_full_path)
    } else {
        fig_full_path.clone()
    };
    let fig_full_str = if is_table_chunk {
        String::new()
    } else {
        fig_abs.to_string_lossy().replace('\\', "/")
    };

    let captured = match engine {
        EngineName::Python => {
            let session = ctx
                .python
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("Python engine session was not initialized"))?;
            session.capture(
                &code,
                &fig_full_str,
                figure.width,
                figure.height,
                f64::from(figure.dpi),
            )?
        }
        EngineName::R => {
            let session = ctx
                .r
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("R engine session was not initialized"))?;
            session.capture(
                &code,
                &fig_full_str,
                figure.r_device(),
                figure.width,
                figure.height,
                f64::from(figure.dpi),
            )?
        }
        EngineName::Jupyter(ref kernel) => {
            let session = ctx
                .jupyter
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("Jupyter engine session was not initialized"))?;
            session.capture(JupyterCapture {
                kernel,
                code: &code,
                fig_path: &fig_full_str,
                fig_format: &figure.format,
                width: figure.width,
                height: figure.height,
                dpi: f64::from(figure.dpi),
            })?
        }
        other => return Err(anyhow::anyhow!("unsupported engine `{}`", other)),
    };

    process_results(&captured, &fig_full_path, &mut results)?;

    if !results
        .iter()
        .any(|result| matches!(result, EngineResult::Source(_)))
    {
        results.insert(0, EngineResult::Source(source.to_vec()));
    }

    Ok(results)
}

pub fn make_sentinel() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("__CALEPIN_{:x}_{:x}__", std::process::id(), seq)
}

fn process_results(raw: &str, fig_path: &Path, results: &mut Vec<EngineResult>) -> Result<()> {
    let (sentinel, rest) = raw.split_once('\n').unwrap_or(("", raw));
    let sep = format!("\n{}_SEP\n", sentinel);

    let source_prefix = format!("{}_SOURCE:", sentinel);
    let output_prefix = format!("{}_OUTPUT:", sentinel);
    let error_prefix = format!("{}_ERROR:", sentinel);
    let warning_prefix = format!("{}_WARNING:", sentinel);
    let message_prefix = format!("{}_MESSAGE:", sentinel);
    let plot_prefix = format!("{}_PLOT:", sentinel);
    let preamble_prefix = format!("{}_PREAMBLE:", sentinel);

    for part in rest.split(&sep) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(text) = part.strip_prefix(&source_prefix) {
            if !text.is_empty() {
                results.push(EngineResult::Source(
                    text.lines().map(ToOwned::to_owned).collect(),
                ));
            }
        } else if let Some(text) = part.strip_prefix(&error_prefix) {
            if !text.is_empty() {
                results.push(EngineResult::Error(text.to_string()));
            }
        } else if let Some(text) = part.strip_prefix(&output_prefix) {
            if let Some(message) = text.strip_prefix(&error_prefix) {
                results.push(EngineResult::Error(message.to_string()));
            } else if !text.is_empty() {
                results.push(EngineResult::Output(text.to_string()));
            }
        } else if let Some(text) = part.strip_prefix(&warning_prefix) {
            if !text.is_empty() {
                results.push(EngineResult::Warning(text.to_string()));
            }
        } else if let Some(text) = part.strip_prefix(&message_prefix) {
            if !text.is_empty() {
                results.push(EngineResult::Message(text.to_string()));
            }
        } else if let Some(text) = part.strip_prefix(&plot_prefix) {
            let path = if text.is_empty() {
                fig_path.to_path_buf()
            } else {
                PathBuf::from(text)
            };
            if path.exists() {
                results.push(EngineResult::Plot(path));
            }
        } else if let Some(text) = part.strip_prefix(&preamble_prefix) {
            if !text.is_empty() {
                results.push(EngineResult::Preamble(text.to_string()));
            }
        }
    }

    Ok(())
}
