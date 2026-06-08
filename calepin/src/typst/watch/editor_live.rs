use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::cli::WatchArgs;
use crate::config::ExecutablePaths;
use crate::typst::compile::{compile_with_typst, resolve_output_path, CompileOptions};
use crate::typst::model::LayoutPaths;
use crate::typst::paths::resolve_layout;
use crate::typst::preprocess::{preprocess, PreprocessOptions, PreprocessOutput, SourceInput};
use crate::typst::source_rewrite::write_staged_source_text;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
enum EditorLiveRequest {
    #[serde(rename = "snapshot")]
    Snapshot {
        version: u64,
        path: PathBuf,
        text: String,
        #[serde(default)]
        exec: bool,
    },
    #[serde(rename = "shutdown")]
    Shutdown,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
enum EditorLiveResponse {
    #[serde(rename = "ready")]
    Ready { input: String, output: String },
    #[serde(rename = "compiling")]
    Compiling { version: u64 },
    #[serde(rename = "compiled")]
    Compiled {
        version: u64,
        output: String,
        format: String,
        diagnostics: Vec<ProtocolDiagnostic>,
    },
    #[serde(rename = "error")]
    Error {
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<u64>,
        message: String,
        diagnostics: Vec<ProtocolDiagnostic>,
    },
}

#[derive(Debug, Serialize)]
struct ProtocolDiagnostic {
    path: String,
    line: u32,
    column: u32,
    #[serde(rename = "endLine")]
    end_line: u32,
    #[serde(rename = "endColumn")]
    end_column: u32,
    severity: &'static str,
    message: String,
    source: &'static str,
}

struct EditorLiveState {
    logical_path: PathBuf,
    layout: LayoutPaths,
    executables: ExecutablePaths,
    themes_dir: PathBuf,
}

pub fn run_editor_live(args: WatchArgs) -> Result<()> {
    let format = args
        .format
        .map(|format| format.as_str().to_string())
        .unwrap_or_else(|| "pdf".to_string());
    let initial_output = args
        .output
        .clone()
        .unwrap_or_else(|| args.input.with_extension(&format));

    emit(&EditorLiveResponse::Ready {
        input: args.input.display().to_string(),
        output: initial_output.display().to_string(),
    })?;

    let stdin = io::stdin();
    let mut state: Option<EditorLiveState> = None;
    for line in stdin.lock().lines() {
        let line = line.context("failed to read editor-live request")?;
        if line.trim().is_empty() {
            continue;
        }

        let request = match serde_json::from_str::<EditorLiveRequest>(&line) {
            Ok(request) => request,
            Err(error) => {
                emit(&EditorLiveResponse::Error {
                    version: None,
                    message: format!("Invalid editor-live request: {error}"),
                    diagnostics: Vec::new(),
                })?;
                continue;
            }
        };

        match request {
            EditorLiveRequest::Snapshot {
                version,
                path,
                text,
                exec,
            } => {
                emit(&EditorLiveResponse::Compiling { version })?;
                let output = args.output.clone();
                let no_exec = args.no_exec && !exec;
                match compile_snapshot(
                    &args, &format, version, path, text, output, no_exec, &mut state,
                ) {
                    Ok(output) => emit(&EditorLiveResponse::Compiled {
                        version,
                        output: output.display().to_string(),
                        format: format.clone(),
                        diagnostics: Vec::new(),
                    })?,
                    Err(error) => {
                        let message = error.to_string();
                        emit(&EditorLiveResponse::Error {
                            version: Some(version),
                            message: message.clone(),
                            diagnostics: vec![diagnostic_for(&args.input, message)],
                        })?;
                    }
                }
            }
            EditorLiveRequest::Shutdown => break,
        }
    }

    Ok(())
}

fn compile_snapshot(
    args: &WatchArgs,
    format: &str,
    version: u64,
    path: PathBuf,
    text: String,
    output: Option<PathBuf>,
    no_exec: bool,
    state: &mut Option<EditorLiveState>,
) -> Result<PathBuf> {
    if !args.common.quiet {
        eprintln!("calepin editor-live compiling snapshot {version}");
    }
    if no_exec {
        if let Some(state) = state.as_ref().filter(|state| state.logical_path == path) {
            return compile_snapshot_fast(args, format, state, &text, output);
        }
    }

    let preserved_results = if no_exec {
        Some(preserve_results(&path)?)
    } else {
        None
    };
    let result = compile_snapshot_inner(args, format, path, text, output, no_exec);
    if let Some(preserved_results) = preserved_results {
        preserved_results.restore()?;
    }
    match result {
        Ok((output_path, next_state)) => {
            *state = Some(next_state);
            Ok(output_path)
        }
        Err(error) => Err(error),
    }
}

fn compile_snapshot_inner(
    args: &WatchArgs,
    format: &str,
    path: PathBuf,
    text: String,
    output: Option<PathBuf>,
    no_exec: bool,
) -> Result<(PathBuf, EditorLiveState)> {
    let preprocessed = preprocess(PreprocessOptions {
        input: path.clone(),
        source: Some(SourceInput {
            logical_path: path.clone(),
            text,
        }),
        config: args.common.config.clone(),
        quiet: args.common.quiet,
        timeout: args.common.timeout,
        sync_pages: false,
        no_exec,
        param_overrides: args.common.params.clone(),
    })?;
    let output_path = resolve_output_path(&preprocessed.layout, output.as_deref(), Some(format));
    compile_preprocessed(
        args,
        format,
        &preprocessed.layout,
        &preprocessed.executables,
        &preprocessed.themes_dir,
        &output_path,
    )?;
    Ok((
        output_path,
        EditorLiveState::from_preprocess(path, preprocessed),
    ))
}

fn compile_snapshot_fast(
    args: &WatchArgs,
    format: &str,
    state: &EditorLiveState,
    text: &str,
    output: Option<PathBuf>,
) -> Result<PathBuf> {
    write_staged_source_text(&state.layout, text)?;
    let output_path = resolve_output_path(&state.layout, output.as_deref(), Some(format));
    compile_preprocessed(
        args,
        format,
        &state.layout,
        &state.executables,
        &state.themes_dir,
        &output_path,
    )?;
    Ok(output_path)
}

fn compile_preprocessed(
    args: &WatchArgs,
    format: &str,
    layout: &LayoutPaths,
    executables: &ExecutablePaths,
    themes_dir: &Path,
    output_path: &Path,
) -> Result<()> {
    compile_with_typst(
        &executables.typst,
        layout,
        CompileOptions {
            output: Some(output_path.to_path_buf()),
            format: Some(format),
            typst_args: &args.typst_args,
            template_theme: None,
            themes_dir,
        },
    )
}

fn emit(response: &EditorLiveResponse) -> Result<()> {
    let mut stdout = io::stdout().lock();
    serde_json::to_writer(&mut stdout, response)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

fn diagnostic_for(path: &std::path::Path, message: String) -> ProtocolDiagnostic {
    ProtocolDiagnostic {
        path: path.display().to_string(),
        line: 1,
        column: 1,
        end_line: 1,
        end_column: 2,
        severity: "error",
        message,
        source: "calepin",
    }
}

struct PreservedResults {
    path: PathBuf,
    contents: Option<Vec<u8>>,
}

fn preserve_results(input: &Path) -> Result<PreservedResults> {
    let layout = resolve_layout(input, None)?;
    let path = layout.results_path;
    let contents = match fs::read(&path) {
        Ok(contents) => Some(contents),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()))
        }
    };
    Ok(PreservedResults { path, contents })
}

impl PreservedResults {
    fn restore(self) -> Result<()> {
        match self.contents {
            Some(contents) => {
                if let Some(parent) = self.path.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("failed to create {}", parent.display()))?;
                }
                fs::write(&self.path, contents)
                    .with_context(|| format!("failed to restore {}", self.path.display()))?;
            }
            None => match fs::remove_file(&self.path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to remove {}", self.path.display()))
                }
            },
        }
        Ok(())
    }
}

impl EditorLiveState {
    fn from_preprocess(logical_path: PathBuf, preprocessed: PreprocessOutput) -> Self {
        Self {
            logical_path,
            layout: preprocessed.layout,
            executables: preprocessed.executables,
            themes_dir: preprocessed.themes_dir,
        }
    }
}
