pub mod python;
pub mod r;
pub mod sh;
pub mod subprocess;

use anyhow::Result;
use std::path::Path;

use crate::types::{ChunkOptions, ChunkResult};
use crate::utils::tools;

/// Holds mutable references to active engine sessions.
pub struct EngineContext<'a> {
    pub r: Option<&'a mut r::RSession>,
    pub python: Option<&'a mut python::PythonSession>,
    pub sh: Option<&'a mut sh::ShSession>,
}

/// Execute a Typst chunk and capture its output.
pub fn execute_chunk(
    source: &[String],
    options: &ChunkOptions,
    label: &str,
    fig_dir: &Path,
    fig_ext: &str,
    ctx: &mut EngineContext,
) -> Result<Vec<ChunkResult>> {
    let code = source.join("\n");
    let mut results = Vec::new();

    if !options.eval() {
        results.push(ChunkResult::Source(source.to_vec()));
        return Ok(results);
    }

    let engine = options.engine();
    let interleaved = !matches!(engine.as_str(), "sh");
    if !interleaved {
        results.push(ChunkResult::Source(source.to_vec()));
    }

    let is_table_chunk = label.starts_with("tbl-");
    std::fs::create_dir_all(fig_dir).ok();
    let fig_width = options.fig_width();
    let fig_height = options.fig_height();
    let fig_full_path = fig_dir.join(format!("{}-1.{}", label, fig_ext));
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

    let captured = match engine.as_str() {
        "sh" => {
            let session = ctx
                .sh
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("{}", tools::not_found_message(&tools::SH)))?;
            session.capture(&code)?
        }
        "python" => {
            let session = ctx
                .python
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("{}", tools::not_found_message(&tools::PYTHON)))?;
            let dpi = options
                .get_opt_string("dpi")
                .and_then(|value| value.parse().ok())
                .unwrap_or_else(|| options.metadata.dpi.unwrap_or(150.0));
            session.capture(&code, &fig_full_str, fig_width, fig_height, dpi)?
        }
        "r" => {
            let session = ctx
                .r
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("{}", tools::not_found_message(&tools::RSCRIPT)))?;
            let dev = if options.get_opt_string("dev").is_some() {
                options.dev()
            } else {
                match fig_ext {
                    "pdf" => "cairo_pdf".to_string(),
                    "svg" => "svg".to_string(),
                    _ => "png".to_string(),
                }
            };
            session.capture(&code, &fig_full_str, &dev, fig_width, fig_height)?
        }
        other => return Err(anyhow::anyhow!("unsupported engine `{}`", other)),
    };

    process_results(&captured, &fig_full_path, options, &mut results)?;

    if interleaved && !results.iter().any(|result| matches!(result, ChunkResult::Source(_))) {
        results.insert(0, ChunkResult::Source(source.to_vec()));
    }

    Ok(results)
}

pub fn make_sentinel() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("__CALEPIN_{:x}_{:x}__", std::process::id(), seq)
}

fn process_results(
    raw: &str,
    fig_path: &Path,
    options: &ChunkOptions,
    results: &mut Vec<ChunkResult>,
) -> Result<()> {
    let (sentinel, rest) = raw.split_once('\n').unwrap_or(("", raw));
    let sep = format!("\n{}_SEP\n", sentinel);

    let source_prefix = format!("{}_SOURCE:", sentinel);
    let output_prefix = format!("{}_OUTPUT:", sentinel);
    let asis_prefix = format!("{}_ASIS:", sentinel);
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
                results.push(ChunkResult::Source(text.lines().map(ToOwned::to_owned).collect()));
            }
        } else if let Some(text) = part.strip_prefix(&error_prefix) {
            if !text.is_empty() {
                results.push(ChunkResult::Error(text.to_string()));
            }
        } else if let Some(text) = part.strip_prefix(&asis_prefix) {
            if !text.is_empty() {
                results.push(ChunkResult::Asis(text.to_string()));
            }
        } else if let Some(text) = part.strip_prefix(&output_prefix) {
            if let Some(message) = text.strip_prefix(&error_prefix) {
                results.push(ChunkResult::Error(message.to_string()));
            } else if !text.is_empty() {
                results.push(ChunkResult::Output(text.to_string()));
            }
        } else if let Some(text) = part.strip_prefix(&warning_prefix) {
            if options.warning() && !text.is_empty() {
                results.push(ChunkResult::Warning(text.to_string()));
            }
        } else if let Some(text) = part.strip_prefix(&message_prefix) {
            if options.message() && !text.is_empty() {
                results.push(ChunkResult::Message(text.to_string()));
            }
        } else if part.starts_with(&plot_prefix) {
            if fig_path.exists() {
                results.push(ChunkResult::Plot(fig_path.to_path_buf()));
            }
        } else if let Some(text) = part.strip_prefix(&preamble_prefix) {
            if !text.is_empty() {
                results.push(ChunkResult::Preamble(text.to_string()));
            }
        }
    }

    Ok(())
}
