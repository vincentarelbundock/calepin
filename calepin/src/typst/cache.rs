use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use xxhash_rust::xxh3::xxh3_128;

use crate::typst::model::{
    ChunkResultDocument, ChunkSpec, RESULT_SCHEMA_VERSION,
};

#[derive(Debug)]
pub struct CacheState {
    cache_dir: PathBuf,
    upstream_digest: u128,
    enabled: bool,
}

impl CacheState {
    pub fn new(cache_dir: PathBuf, enabled: bool) -> Self {
        Self {
            cache_dir,
            upstream_digest: 0,
            enabled,
        }
    }

    pub fn lookup_or_execute(
        &mut self,
        chunk: &ChunkSpec,
        root: &Path,
        execute: impl FnOnce() -> Result<ChunkResultDocument>,
    ) -> Result<ChunkResultDocument> {
        let key_hash = compute_key(chunk, self.upstream_digest)?;
        self.advance_digest(key_hash);

        if !self.enabled || !chunk.exec_options.cache {
            return execute();
        }

        if let Some(mut cached) = load_entry(&self.cache_dir, root, key_hash)? {
            cached.cached = true;
            return Ok(cached);
        }

        let document = execute()?;
        if let Err(error) = store_entry(&self.cache_dir, root, key_hash, &document) {
            cwarn!("cache write failed for chunk '{}': {}", chunk.label, error);
        }
        Ok(document)
    }

    fn advance_digest(&mut self, chunk_hash: u128) {
        let mut bytes = [0_u8; 32];
        bytes[..16].copy_from_slice(&self.upstream_digest.to_le_bytes());
        bytes[16..].copy_from_slice(&chunk_hash.to_le_bytes());
        self.upstream_digest = xxh3_128(&bytes);
    }
}

#[derive(Serialize)]
struct CacheKey<'a> {
    schema: u8,
    engine: &'a str,
    code: &'a str,
    label: &'a str,
    eval: bool,
    dev: &'a str,
    dpi: u32,
    fig_width: f64,
    fig_height: Option<f64>,
    upstream_digest: String,
}

pub fn compute_key(chunk: &ChunkSpec, upstream_digest: u128) -> Result<u128> {
    let key = CacheKey {
        schema: RESULT_SCHEMA_VERSION,
        engine: chunk.engine.as_str(),
        code: &chunk.code,
        label: &chunk.label,
        eval: chunk.exec_options.eval,
        dev: &chunk.exec_options.dev,
        dpi: chunk.exec_options.dpi,
        fig_width: chunk.exec_options.fig_width,
        fig_height: chunk.exec_options.fig_height,
        upstream_digest: hex(upstream_digest),
    };
    Ok(xxh3_128(&serde_json::to_vec(&key)?))
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheMeta {
    schema: u8,
    hash: String,
    artifacts: Vec<CachedArtifact>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CachedArtifact {
    result_path: String,
    cache_file: String,
}

fn store_entry(cache_dir: &Path, root: &Path, key_hash: u128, document: &ChunkResultDocument) -> Result<()> {
    let entry = entry_dir(cache_dir, key_hash);
    let artifacts_dir = entry.join("artifacts");
    std::fs::create_dir_all(&artifacts_dir)
        .with_context(|| format!("failed to create {}", artifacts_dir.display()))?;

    let mut artifacts = Vec::new();
    for (index, result_path) in artifact_refs(document).into_iter().enumerate() {
        let src = absolute_artifact_path(root, &result_path);
        if !src.exists() {
            continue;
        }
        let filename = src
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "artifact".to_string());
        let cache_file = format!("{}-{}", index, filename);
        std::fs::copy(&src, artifacts_dir.join(&cache_file))
            .with_context(|| format!("failed to cache {}", src.display()))?;
        artifacts.push(CachedArtifact {
            result_path,
            cache_file,
        });
    }

    let meta = CacheMeta {
        schema: RESULT_SCHEMA_VERSION,
        hash: hex(key_hash),
        artifacts,
    };
    std::fs::write(entry.join("meta.json"), serde_json::to_string_pretty(&meta)?)
        .with_context(|| format!("failed to write cache metadata in {}", entry.display()))?;
    std::fs::write(entry.join("result.json"), serde_json::to_string(document)?)
        .with_context(|| format!("failed to write cache result in {}", entry.display()))?;
    Ok(())
}

fn load_entry(cache_dir: &Path, root: &Path, key_hash: u128) -> Result<Option<ChunkResultDocument>> {
    let entry = entry_dir(cache_dir, key_hash);
    let meta_path = entry.join("meta.json");
    let result_path = entry.join("result.json");
    if !meta_path.exists() || !result_path.exists() {
        return Ok(None);
    }

    let meta: CacheMeta = serde_json::from_str(&std::fs::read_to_string(&meta_path)?)
        .with_context(|| format!("failed to read {}", meta_path.display()))?;
    if meta.schema != RESULT_SCHEMA_VERSION || meta.hash != hex(key_hash) {
        return Ok(None);
    }

    let document: ChunkResultDocument = serde_json::from_str(&std::fs::read_to_string(&result_path)?)
        .with_context(|| format!("failed to read {}", result_path.display()))?;
    for artifact in meta.artifacts {
        let src = entry.join("artifacts").join(&artifact.cache_file);
        let dest = absolute_artifact_path(root, &artifact.result_path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        if src.exists() {
            std::fs::copy(&src, &dest)
                .with_context(|| format!("failed to restore {}", dest.display()))?;
        }
    }
    Ok(Some(document))
}

fn artifact_refs(document: &ChunkResultDocument) -> Vec<String> {
    let mut refs = Vec::new();
    for item in &document.items {
        let Some(data) = &item.data else {
            continue;
        };
        for value in data.values() {
            if let Some(path) = value.get("path").and_then(|path| path.as_str()) {
                refs.push(path.to_string());
            }
        }
    }
    refs
}

fn absolute_artifact_path(root: &Path, result_path: &str) -> PathBuf {
    if let Some(root_relative) = result_path.strip_prefix('/') {
        return root.join(root_relative);
    }
    let path = Path::new(result_path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn entry_dir(cache_dir: &Path, hash: u128) -> PathBuf {
    cache_dir.join(&hex(hash)[..16])
}

fn hex(hash: u128) -> String {
    format!("{hash:032x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::model::{
        ChunkStatus, DisplayOptions, EngineName, ExecOptions, ItemSelector, MimeData, ResultItem,
        ResultsMode, SetupDefaults,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    fn chunk() -> ChunkSpec {
        let defaults = SetupDefaults::default();
        ChunkSpec {
            label: "demo".to_string(),
            engine: EngineName::Python,
            code: "print('x')".to_string(),
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
                results: ResultsMode::Verbatim,
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

    fn document(path: &str) -> ChunkResultDocument {
        let mut data = MimeData::new();
        data.insert("image/svg+xml".to_string(), json!({ "path": path }));
        ChunkResultDocument {
            label: "demo".to_string(),
            engine: EngineName::Python,
            status: ChunkStatus::Ok,
            cached: false,
            items: vec![ResultItem {
                item_type: "display".to_string(),
                name: None,
                text: None,
                level: None,
                message: None,
                traceback: None,
                data: Some(data),
                metadata: BTreeMap::new(),
            }],
        }
    }

    #[test]
    fn key_changes_with_source_and_upstream() {
        let original = chunk();
        let mut changed = original.clone();
        changed.code = "print('y')".to_string();

        assert_ne!(compute_key(&original, 0).unwrap(), compute_key(&changed, 0).unwrap());
        assert_ne!(compute_key(&original, 0).unwrap(), compute_key(&original, 42).unwrap());
    }

    #[test]
    fn key_excludes_display_only_options() {
        let original = chunk();
        let mut changed = original.clone();
        changed.display_options.echo = false;
        changed.display_options.results = ResultsMode::Hide;
        changed.display_options.fig_cap = Some("Caption".to_string());
        changed.exec_options.error = true;

        assert_eq!(compute_key(&original, 0).unwrap(), compute_key(&changed, 0).unwrap());
    }

    #[test]
    fn key_changes_with_label() {
        let original = chunk();
        let mut changed = original.clone();
        changed.label = "other".to_string();

        assert_ne!(compute_key(&original, 0).unwrap(), compute_key(&changed, 0).unwrap());
    }

    #[test]
    fn lookup_restores_cached_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let cache_dir = root.join(".calepin/paper/cache");
        let artifact = root.join(".calepin/paper/figures/demo.svg");
        std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        std::fs::write(&artifact, "<svg>first</svg>").unwrap();

        let mut state = CacheState::new(cache_dir.clone(), true);
        let first = state
            .lookup_or_execute(&chunk(), root, || Ok(document(".calepin/paper/figures/demo.svg")))
            .unwrap();
        assert!(!first.cached);
        std::fs::remove_file(&artifact).unwrap();

        let mut state = CacheState::new(cache_dir, true);
        let second = state
            .lookup_or_execute(&chunk(), root, || panic!("cache should hit"))
            .unwrap();

        assert!(second.cached);
        assert_eq!(std::fs::read_to_string(&artifact).unwrap(), "<svg>first</svg>");
    }
}
