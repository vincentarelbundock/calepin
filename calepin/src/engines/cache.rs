// Code chunk caching with content-addressed storage.
//
// ## Digest chain
//
// Each render recomputes the full digest chain from scratch. This is cheap —
// just hashing small fixed-size buffers, not re-executing chunks. The workflow:
//
// 1. Walk chunks in order, building a running "upstream digest"
// 2. For each chunk, compute its cache key from source + options + upstream digest
// 3. Look up that key in the cache — hit or miss falls out naturally
//
// If chunk 3 changes, chunks 1 and 2 still produce the same digest (cache hits),
// but chunk 3's key changes, which cascades: chunk 4's upstream digest is now
// different too, so its key changes, and so on. All chunks after the change
// automatically get cache misses, which is correct since they may depend on
// the changed state. You never need to ask "where did things change?" — the
// digest chain makes downstream invalidation automatic.
//
// ## Cache key invalidation
//
// A chunk's cache key includes all options except known display-only ones
// (echo, include, results, warning, message, comment, fig.cap, fig.alt, cache).
// Display-only options are applied after cache lookup in evaluate_block, so they
// don't affect execution output. Any unknown option invalidates by default —
// new options are safe without updating this module.
//
// ## Functions
//
// - CacheState::new()              — Initialize cache state for a document.
// - CacheState::advance_digest()   — Mix a chunk's key hash into the running upstream digest.
// - CacheState::advance_digest_inline() — Mix inline code expressions into the upstream digest.
// - execute_chunk_cached()         — Execute a chunk with cache lookup: compute key from source,
//                                    options, and upstream digest; load from cache on hit; execute
//                                    and store on miss. Always advances the upstream digest chain.
// - compute_key()                  — xxh3-128 hash of source, options, and upstream digest.
// - load_cache() / store_cache()   — Read/write cached results (bincode) and metadata (JSON).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use xxhash_rust::xxh3::xxh3_128;

use crate::types::{ChunkOptions, ChunkResult};

/// Tracks the running upstream digest and cache directory across the evaluate loop.
pub struct CacheState {
    pub cache_dir: PathBuf,
    pub upstream_digest: u128,
    pub enabled: bool,
}

impl CacheState {
    pub fn new(input_path: &Path, enabled: bool) -> Self {
        let stem = input_path.file_stem().unwrap_or_default().to_string_lossy();
        let cache_dir = input_path.with_file_name(format!("{}_cache", stem));
        Self {
            cache_dir,
            upstream_digest: 0,
            enabled,
        }
    }

    /// Create a CacheState with an explicit cache directory.
    pub fn new_with_dir(input_path: &Path, cache_dir: &Path, enabled: bool) -> Self {
        let _ = input_path; // input_path reserved for future hash seeding
        Self {
            cache_dir: cache_dir.to_path_buf(),
            upstream_digest: 0,
            enabled,
        }
    }

    /// Update the upstream digest by mixing in a chunk's key hash.
    pub fn advance_digest(&mut self, chunk_hash: u128) {
        let mut buf = [0u8; 32];
        buf[..16].copy_from_slice(&self.upstream_digest.to_le_bytes());
        buf[16..].copy_from_slice(&chunk_hash.to_le_bytes());
        self.upstream_digest = xxh3_128(&buf);
    }

    /// Update the upstream digest with inline code expressions (they mutate session state).
    pub fn advance_digest_inline(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let text_hash = xxh3_128(text.as_bytes());
        let mut buf = [0u8; 32];
        buf[..16].copy_from_slice(&self.upstream_digest.to_le_bytes());
        buf[16..].copy_from_slice(&text_hash.to_le_bytes());
        self.upstream_digest = xxh3_128(&buf);
    }
}

/// Options that only affect display, not execution. These are applied after
/// cache lookup in `evaluate_block`, so changing them should not invalidate.
const DISPLAY_ONLY_OPTIONS: &[&str] = &[
    "echo", "include", "warning", "message", "comment", "results",
    "fig.cap", "fig.alt", "fig.cap.location", "tbl.cap", "cache",
];

/// Compute the cache key hash for a chunk.
fn compute_key(
    source: &[String],
    options: &ChunkOptions,
    upstream_digest: u128,
) -> u128 {
    // Build a single buffer with all key material, then hash once
    let mut buf = Vec::new();

    // Chunk source code
    for line in source {
        buf.extend_from_slice(line.as_bytes());
        buf.push(b'\n');
    }

    // All options except display-only ones, sorted for determinism
    let mut keys: Vec<&String> = options.inner.keys()
        .filter(|k| !DISPLAY_ONLY_OPTIONS.contains(&k.as_str()))
        .collect();
    keys.sort();
    for key in keys {
        buf.extend_from_slice(key.as_bytes());
        buf.push(b':');
        match &options.inner[key] {
            crate::types::OptionValue::Bool(b) => buf.push(if *b { 1 } else { 0 }),
            crate::types::OptionValue::String(s) => buf.extend_from_slice(s.as_bytes()),
            crate::types::OptionValue::Number(n) => buf.extend_from_slice(&n.to_bits().to_le_bytes()),
            crate::types::OptionValue::Null => buf.push(0xFF),
        }
        buf.push(b'\n');
    }

    // Upstream digest (cumulative hash of all prior chunks)
    buf.extend_from_slice(b"upstream:");
    buf.extend_from_slice(&upstream_digest.to_le_bytes());

    xxh3_128(&buf)
}

/// Hex-encode a 128-bit hash.
fn hex(hash: u128) -> String {
    format!("{:032x}", hash)
}

/// Cache metadata stored alongside serialized results.
#[derive(serde::Serialize, serde::Deserialize)]
struct CacheMeta {
    hash: String,
    plot_files: Vec<String>,
}

/// Try to load cached results for a chunk. Returns None on miss.
fn load_cache(
    cache_dir: &Path,
    label: &str,
    key_hash: u128,
) -> Option<(Vec<ChunkResult>, Vec<String>)> {
    let chunk_dir = cache_dir.join(label);
    let meta_path = chunk_dir.join("meta.json");
    let results_path = chunk_dir.join("results.bincode");

    let meta_bytes = fs::read_to_string(&meta_path).ok()?;
    let meta: CacheMeta = serde_json::from_str(&meta_bytes).ok()?;

    if meta.hash != hex(key_hash) {
        return None;
    }

    let results_bytes = fs::read(&results_path).ok()?;
    let config = bincode::config::standard();
    let (results, _): (Vec<ChunkResult>, _) =
        bincode::serde::decode_from_slice(&results_bytes, config).ok()?;

    Some((results, meta.plot_files))
}

/// Store results in the cache.
fn store_cache(
    cache_dir: &Path,
    label: &str,
    key_hash: u128,
    results: &[ChunkResult],
    _fig_dir: &Path,
) -> Result<()> {
    let chunk_dir = cache_dir.join(label);
    fs::create_dir_all(&chunk_dir)?;

    // Find plot files in results and copy them to cache
    let mut plot_files = Vec::new();
    for result in results {
        if let ChunkResult::Plot(path) = result {
            if path.exists() {
                if let Some(filename) = path.file_name() {
                    let dest = chunk_dir.join(filename);
                    fs::copy(path, &dest)?;
                    plot_files.push(filename.to_string_lossy().to_string());
                }
            }
        }
    }

    // Write meta.json
    let meta = CacheMeta {
        hash: hex(key_hash),
        plot_files,
    };
    fs::write(
        chunk_dir.join("meta.json"),
        serde_json::to_string(&meta)?,
    )?;

    // Write results.bincode
    let config = bincode::config::standard();
    let encoded = bincode::serde::encode_to_vec(results, config)?;
    fs::write(chunk_dir.join("results.bincode"), encoded)?;

    Ok(())
}

/// Execute a chunk with caching support.
/// If cache is enabled for this chunk, tries to load from cache first.
/// Always updates the upstream digest regardless of cache hit/miss.
pub fn execute_chunk_cached(
    source: &[String],
    options: &ChunkOptions,
    label: &str,
    fig_dir: &Path,
    fig_ext: &str,
    ctx: &mut super::EngineContext,
    cache: &mut CacheState,
) -> Result<Vec<ChunkResult>> {
    // Compute cache key for this chunk (always, even if cache is off for this chunk).
    // This ensures every chunk contributes to the upstream digest chain.
    let key_hash = compute_key(source, options, cache.upstream_digest);

    // Always advance the upstream digest, even for eval=false or non-cached chunks
    cache.advance_digest(key_hash);

    // eval=false: don't execute, just return source (digest already advanced above)
    if !options.eval() {
        return super::execute_chunk(source, options, label, fig_dir, fig_ext, ctx);
    }

    // If caching disabled globally or for this chunk, execute normally
    if !cache.enabled || !options.cache() {
        return super::execute_chunk(source, options, label, fig_dir, fig_ext, ctx);
    }

    // Use the hash as the cache directory name (not the label, which may be positional)
    let cache_id = &hex(key_hash)[..16];

    // Try cache hit
    if let Some((results, plot_files)) = load_cache(&cache.cache_dir, cache_id, key_hash) {
        // Restore plot files to fig_dir
        let chunk_dir = cache.cache_dir.join(cache_id);
        for filename in &plot_files {
            let src = chunk_dir.join(filename);
            if src.exists() {
                fs::create_dir_all(fig_dir).ok();
                let dest = fig_dir.join(filename);
                fs::copy(&src, &dest)?;
            }
        }
        return Ok(results);
    }

    // Cache miss: execute and store
    let results = super::execute_chunk(source, options, label, fig_dir, fig_ext, ctx)?;

    if let Err(e) = store_cache(&cache.cache_dir, cache_id, key_hash, &results, fig_dir) {
        cwarn!("cache write failed for chunk '{}': {}", label, e);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ChunkOptions;

    #[test]
    fn test_cache_key_changes_with_source() {
        let opts = ChunkOptions::default();
        let k1 = compute_key(&["x <- 1".into()], &opts, 0);
        let k2 = compute_key(&["x <- 2".into()], &opts, 0);
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_cache_key_changes_with_upstream() {
        let opts = ChunkOptions::default();
        let source = vec!["x <- 1".into()];
        let k1 = compute_key(&source, &opts, 0);
        let k2 = compute_key(&source, &opts, 12345);
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_cache_key_stable() {
        let opts = ChunkOptions::default();
        let source = vec!["x <- 1".into()];
        let k1 = compute_key(&source, &opts, 42);
        let k2 = compute_key(&source, &opts, 42);
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_cache_state_advance() {
        let mut state = CacheState {
            cache_dir: PathBuf::from("/tmp/test__cache"),
            upstream_digest: 0,
            enabled: true,
        };
        let initial = state.upstream_digest;
        state.advance_digest(123);
        assert_ne!(state.upstream_digest, initial);
    }

    #[test]
    fn test_cache_miss_on_nonexistent() {
        let result = load_cache(Path::new("/nonexistent/__cache"), "chunk1", 0);
        assert!(result.is_none());
    }

    #[test]
    fn test_store_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let cache_dir = dir.path().join("test__cache");
        let fig_dir = dir.path().join("figs");

        let results = vec![
            ChunkResult::Source(vec!["x <- 1".into()]),
            ChunkResult::Output("1".into()),
        ];
        let key_hash: u128 = 42;

        store_cache(&cache_dir, "my-chunk", key_hash, &results, &fig_dir).unwrap();

        let (loaded, plots) = load_cache(&cache_dir, "my-chunk", key_hash).unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(plots.is_empty());

        // Wrong key should miss
        assert!(load_cache(&cache_dir, "my-chunk", 99).is_none());
    }
}
