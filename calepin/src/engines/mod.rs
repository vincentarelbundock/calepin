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

/// The `-N` suffix rule for naming the Nth figure a single chunk emits: the
/// first plot keeps the base path, later ones get `-N` inserted before the
/// extension (stripping any existing `-1` on the base first, so it isn't
/// doubled). Both the Python engine bootstrap and the Jupyter bridge
/// bootstrap are Python scripts, so this one definition is spliced into both
/// (via the `__CALEPIN_PLOT_PATH_HELPER__` marker) instead of being
/// hand-copied twice. R needs its own R-language implementation (see
/// `.calepin_plot_path` in r.rs) since it cannot share Python source.
pub(crate) const PY_PLOT_INDEX_HELPER: &str = r#"def _calepin_plot_path(base, index):
    if index <= 1:
        return base
    root, ext = os.path.splitext(base)
    if root.endswith("-1"):
        root = root[:-2]
    return f"{root}-{index}{ext}""#;

/// Marker substituted for [`PY_PLOT_INDEX_HELPER`] inside a bootstrap script
/// template. Must sit at column 0 (module scope) in the template, since the
/// helper it's replaced with is not indented.
pub(crate) const PLOT_PATH_HELPER_MARKER: &str = "__CALEPIN_PLOT_PATH_HELPER__";

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
    Unavailable(String),
    Plot(PathBuf),
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

pub(crate) fn process_results(
    raw: &str,
    fig_path: &Path,
    results: &mut Vec<EngineResult>,
) -> Result<()> {
    let (sentinel, rest) = raw.split_once('\n').unwrap_or(("", raw));
    let sep_marker = format!("{}_SEP", sentinel);

    let source_prefix = format!("{}_SOURCE:", sentinel);
    let output_prefix = format!("{}_OUTPUT:", sentinel);
    let error_prefix = format!("{}_ERROR:", sentinel);
    let warning_prefix = format!("{}_WARNING:", sentinel);
    let message_prefix = format!("{}_MESSAGE:", sentinel);
    let unavailable_prefix = format!("{}_UNAVAILABLE:", sentinel);
    let plot_prefix = format!("{}_PLOT:", sentinel);

    let prefixes = [
        source_prefix.as_str(),
        error_prefix.as_str(),
        output_prefix.as_str(),
        warning_prefix.as_str(),
        message_prefix.as_str(),
        unavailable_prefix.as_str(),
        plot_prefix.as_str(),
    ];

    for part in split_result_parts(rest, &sep_marker) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        // A well-formed part starts with one of the tagged prefixes above. If
        // it doesn't, text was written directly to the subprocess's stdout
        // file descriptor (e.g. `subprocess.run(...)`, `system()`, a C
        // extension's `printf`) and landed ahead of the tagged frame instead
        // of going through the language-level capture. Report that stray
        // text as engine output rather than silently dropping the whole
        // part, then keep parsing from the first recognized prefix onward.
        let tag_start = prefixes.iter().filter_map(|prefix| part.find(prefix)).min();
        let (stray, part) = match tag_start {
            Some(0) => ("", part),
            Some(index) => (part[..index].trim(), &part[index..]),
            None => (part, ""),
        };
        if !stray.is_empty() {
            results.push(EngineResult::Output(stray.to_string()));
        }
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
        } else if let Some(text) = part.strip_prefix(&unavailable_prefix) {
            if !text.is_empty() {
                results.push(EngineResult::Unavailable(text.to_string()));
            }
        } else if let Some(text) = part.strip_prefix(&plot_prefix) {
            let path = if text.is_empty() {
                fig_path.to_path_buf()
            } else {
                PathBuf::from(text)
            };
            results.push(EngineResult::Plot(path));
        }
    }

    Ok(())
}

fn split_result_parts(rest: &str, sep_marker: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();

    for line in rest.split_inclusive('\n') {
        let trimmed_line = line.trim_end_matches('\n').trim_end_matches('\r');
        if trimmed_line == sep_marker {
            parts.push(std::mem::take(&mut current));
        } else {
            current.push_str(line);
        }
    }

    parts.push(current);
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_results_preserves_missing_plot_paths_for_later_validation() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing-plot.svg");
        let raw = format!("__TEST__\n__TEST___PLOT:{}", missing.display());
        let mut results = Vec::new();

        process_results(&raw, &dir.path().join("fallback.svg"), &mut results).unwrap();

        assert_eq!(results.len(), 1);
        assert!(matches!(&results[0], EngineResult::Plot(path) if path == &missing));
    }

    #[test]
    fn process_results_parses_unavailable_engine_marker() {
        let raw = "__TEST__\n__TEST___UNAVAILABLE:no kernel named sh";
        let mut results = Vec::new();

        process_results(raw, Path::new("unused.svg"), &mut results).unwrap();

        assert_eq!(results.len(), 1);
        assert!(
            matches!(&results[0], EngineResult::Unavailable(message) if message == "no kernel named sh")
        );
    }

    #[test]
    fn process_results_parses_crlf_separated_output_records() {
        let raw =
            "__TEST__\n__TEST___SOURCE:x = 1\r\nprint(x)\r\n__TEST___SEP\r\n__TEST___OUTPUT:1\r";
        let mut results = Vec::new();

        process_results(raw, Path::new("unused.svg"), &mut results).unwrap();

        assert_eq!(results.len(), 2);
        assert!(matches!(
            &results[0],
            EngineResult::Source(lines) if lines == &vec!["x = 1".to_string(), "print(x)".to_string()]
        ));
        assert!(matches!(&results[1], EngineResult::Output(text) if text == "1"));
    }

    #[test]
    fn process_results_reports_stray_fd_level_text_as_output() {
        // Text written straight to fd 1 (e.g. `subprocess.run([...])`,
        // `system()`, a C extension's `printf`) bypasses the language-level
        // capture and lands ahead of the first tagged part instead of behind
        // a `_SEP` marker. It must surface as output, not be dropped.
        let raw = "__TEST__\nhi\n__TEST___SOURCE:x = 1";
        let mut results = Vec::new();

        process_results(raw, Path::new("unused.svg"), &mut results).unwrap();

        assert_eq!(results.len(), 2);
        assert!(matches!(&results[0], EngineResult::Output(text) if text == "hi"));
        assert!(matches!(
            &results[1],
            EngineResult::Source(lines) if lines == &vec!["x = 1".to_string()]
        ));
    }

    #[test]
    fn process_results_reports_untagged_part_entirely_as_output() {
        let raw = "__TEST__\njust some stray text with no tag at all";
        let mut results = Vec::new();

        process_results(raw, Path::new("unused.svg"), &mut results).unwrap();

        assert_eq!(results.len(), 1);
        assert!(matches!(
            &results[0],
            EngineResult::Output(text) if text == "just some stray text with no tag at all"
        ));
    }

    #[test]
    fn process_results_does_not_leak_crlf_markers_into_warning_records() {
        let raw = "__TEST__\n__TEST___WARNING:3\r\n__TEST___SEP\r\n__TEST___SOURCE:import sys\r\nprint(3, file=sys.stderr)\r";
        let mut results = Vec::new();

        process_results(raw, Path::new("unused.svg"), &mut results).unwrap();

        assert_eq!(results.len(), 2);
        assert!(matches!(&results[0], EngineResult::Warning(text) if text == "3"));
        assert!(matches!(
            &results[1],
            EngineResult::Source(lines) if lines == &vec![
                "import sys".to_string(),
                "print(3, file=sys.stderr)".to_string()
            ]
        ));
    }
}
