use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Serialize;
use xxhash_rust::xxh3::xxh3_64;

use crate::config::ExecutablePaths;
use crate::typst::io::write_if_changed;
use crate::typst::model::{ChunkSpec, EngineName, ExecOptions, LayoutPaths};

const PREPROCESS_FINGERPRINT_FILE: &str = "fingerprint.xxh3";

pub(super) fn preprocess_fingerprint(
    layout: &LayoutPaths,
    executables: &ExecutablePaths,
    chunks: &[ChunkSpec],
    cwd: &Path,
    timeout: Option<Duration>,
    params: &serde_json::Value,
    theme: &crate::theme::ThemeSelection,
    runtime_dir: &Path,
    image_meta_signature: u64,
) -> Result<u64> {
    let payload = PreprocessFingerprint {
        schema: crate::typst::model::RESULT_SCHEMA_VERSION,
        calepin_version: env!("CARGO_PKG_VERSION"),
        input_rel: path_fingerprint(&layout.input_rel),
        figures_dir: path_fingerprint(&layout.figures_dir),
        cwd: path_fingerprint(cwd),
        timeout_secs: timeout.map(|duration| duration.as_secs()),
        executables: ExecutableFingerprint::from(executables),
        chunks: chunks
            .iter()
            .map(ChunkFingerprint::from)
            .collect::<Vec<_>>(),
        params: params.clone(),
        theme: theme_fingerprint(theme),
        runtime_dir: path_fingerprint(runtime_dir),
        image_meta: format!("{image_meta_signature:016x}"),
    };
    let bytes = serde_json::to_vec(&payload)?;
    Ok(xxh3_64(&bytes))
}

pub(super) fn preprocess_cache_hit(layout: &LayoutPaths, fingerprint: u64) -> Result<bool> {
    if !layout.results_path.is_file() {
        return Ok(false);
    }
    Ok(read_preprocess_fingerprint(layout)? == Some(fingerprint))
}

fn preprocess_fingerprint_path(layout: &LayoutPaths) -> PathBuf {
    layout.artifact_path(PREPROCESS_FINGERPRINT_FILE)
}

fn read_preprocess_fingerprint(layout: &LayoutPaths) -> Result<Option<u64>> {
    let path = preprocess_fingerprint_path(layout);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()))
        }
    };
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    Ok(u64::from_str_radix(text, 16).ok())
}

pub(super) fn write_preprocess_fingerprint(layout: &LayoutPaths, fingerprint: u64) -> Result<()> {
    let path = preprocess_fingerprint_path(layout);
    let text = format!("{fingerprint:016x}\n");
    write_if_changed(&path, text)
}

#[derive(Serialize)]
struct PreprocessFingerprint {
    schema: u8,
    calepin_version: &'static str,
    input_rel: String,
    figures_dir: String,
    cwd: String,
    timeout_secs: Option<u64>,
    executables: ExecutableFingerprint,
    chunks: Vec<ChunkFingerprint>,
    params: serde_json::Value,
    theme: String,
    runtime_dir: String,
    image_meta: String,
}

#[derive(Serialize)]
struct ChunkFingerprint {
    label: String,
    ordinal: usize,
    engine: EngineName,
    code: String,
    exec_options: ExecOptions,
}

impl From<&ChunkSpec> for ChunkFingerprint {
    fn from(chunk: &ChunkSpec) -> Self {
        Self {
            label: chunk.label.clone(),
            ordinal: chunk.ordinal,
            engine: chunk.engine.clone(),
            code: chunk.code.clone(),
            exec_options: chunk.exec_options.clone(),
        }
    }
}

#[derive(Serialize)]
struct ExecutableFingerprint {
    typst: String,
    rscript: String,
    python: String,
    mmdc: String,
    dot: String,
    tectonic: String,
    dvisvgm: String,
    pdf2svg: String,
    d2: String,
    chrome: Option<String>,
}

impl From<&ExecutablePaths> for ExecutableFingerprint {
    fn from(paths: &ExecutablePaths) -> Self {
        Self {
            typst: path_fingerprint(&paths.typst),
            rscript: path_fingerprint(&paths.rscript),
            python: path_fingerprint(&paths.python),
            mmdc: path_fingerprint(&paths.mmdc),
            dot: path_fingerprint(&paths.dot),
            tectonic: path_fingerprint(&paths.tectonic),
            dvisvgm: path_fingerprint(&paths.dvisvgm),
            pdf2svg: path_fingerprint(&paths.pdf2svg),
            d2: path_fingerprint(&paths.d2),
            chrome: paths.chrome.as_deref().map(path_fingerprint),
        }
    }
}

fn path_fingerprint(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn theme_fingerprint(theme: &crate::theme::ThemeSelection) -> String {
    match theme {
        crate::theme::ThemeSelection::Default => "default".to_string(),
        crate::theme::ThemeSelection::Typst => "typst".to_string(),
        crate::theme::ThemeSelection::Builtin(name) => format!("builtin:{name}"),
        crate::theme::ThemeSelection::Dir(path) => format!("dir:{}", path.display()),
    }
}
