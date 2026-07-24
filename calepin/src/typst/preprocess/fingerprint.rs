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
    execution: ExecutionFingerprintInputs<'_>,
    theme: &crate::theme::ThemeSelection,
    asset_dir: &Path,
    image_meta_signature: u64,
) -> Result<u64> {
    let payload = PreprocessFingerprint {
        schema: crate::typst::model::RESULT_SCHEMA_VERSION,
        calepin_version: env!("CARGO_PKG_VERSION"),
        input_rel: path_fingerprint(&layout.input_rel),
        figures_dir: path_fingerprint(&layout.figures_dir),
        cwd: path_fingerprint(execution.cwd),
        timeout_secs: execution.timeout.map(|duration| duration.as_secs()),
        executables: ExecutableFingerprint::from(executables),
        chunks: chunks
            .iter()
            .map(ChunkFingerprint::from)
            .collect::<Vec<_>>(),
        store: execution.store.clone(),
        theme: theme_fingerprint(theme),
        asset_dir: path_fingerprint(asset_dir),
        image_meta: format!("{image_meta_signature:016x}"),
    };
    let bytes = serde_json::to_vec(&payload)?;
    Ok(xxh3_64(&bytes))
}

pub(super) struct ExecutionFingerprintInputs<'a> {
    pub cwd: &'a Path,
    pub timeout: Option<Duration>,
    pub store: &'a serde_json::Value,
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
    store: serde_json::Value,
    theme: String,
    asset_dir: String,
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
    let Ok(chain) = crate::theme::resolve_theme_chain(theme) else {
        return match theme {
            crate::theme::ThemeSelection::Default => "default".to_string(),
            crate::theme::ThemeSelection::Typst => "typst".to_string(),
            crate::theme::ThemeSelection::Builtin(name) => format!("builtin:{name}"),
            crate::theme::ThemeSelection::Dir(path) => format!("dir:{}", path.display()),
        };
    };
    let mut parts = Vec::new();
    if chain.terminal_typst {
        parts.push("typst".to_string());
    }
    for layer in chain.layers {
        match layer {
            crate::theme::ThemeLayer::Builtin(name) => parts.push(format!("builtin:{name}")),
            crate::theme::ThemeLayer::Dir(path) => {
                parts.push(format!(
                    "dir:{}:{}",
                    path.display(),
                    theme_dir_fingerprint(&path)
                ));
            }
        }
    }
    parts.join("|")
}

fn theme_dir_fingerprint(path: &Path) -> String {
    let mut files = Vec::new();
    if let Err(error) = collect_theme_files(path, path, &mut files) {
        return format!("error:{error}");
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut payload = Vec::new();
    for (relative, hash) in files {
        payload.extend_from_slice(relative.as_bytes());
        payload.push(0);
        payload.extend_from_slice(&hash.to_le_bytes());
    }
    format!("{:016x}", xxh3_64(&payload))
}

fn collect_theme_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<(String, u64)>,
) -> Result<()> {
    let mut entries = std::fs::read_dir(directory)
        .with_context(|| format!("failed to read theme directory {}", directory.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_theme_files(root, &path, files)?;
        } else if file_type.is_file() {
            let bytes = std::fs::read(&path)
                .with_context(|| format!("failed to read theme file {}", path.display()))?;
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            files.push((relative, xxh3_64(&bytes)));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_fingerprint_tracks_all_local_theme_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("theme.toml"), "extends = \"typst\"\n").unwrap();
        std::fs::create_dir_all(dir.path().join("layouts")).unwrap();
        std::fs::write(dir.path().join("layouts/pdf.typ"), "#let value = 1\n").unwrap();
        let theme = crate::theme::ThemeSelection::Dir(dir.path().to_path_buf());

        let first = theme_fingerprint(&theme);
        std::fs::write(dir.path().join("layouts/pdf.typ"), "#let value = 2\n").unwrap();
        let second = theme_fingerprint(&theme);

        assert_ne!(first, second);
    }
}
