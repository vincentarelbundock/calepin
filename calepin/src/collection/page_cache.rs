// Page-level cache for collection builds.
//
// Stores a hash per page that captures all inputs affecting its rendered output:
// source content, config file mtime, target name, and overrides. On subsequent
// builds, pages whose hash matches are skipped entirely (no parse, no evaluate,
// no render). The cache file lives at `{output}/.page_cache.json`.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use xxhash_rust::xxh3::xxh3_128;

/// Compute a cache key for a single page.
///
/// Inputs mixed into the hash:
/// - source file content (the .qmd body)
/// - config file content (the _calepin.toml -- changes to nav, targets, etc.)
/// - target name (html vs latex produce different output)
/// - overrides (embed-resources, highlight style, etc.)
pub fn page_hash(
    source_content: &[u8],
    config_content: &[u8],
    target_name: &str,
    overrides: &[String],
) -> u64 {
    let mut buf = Vec::with_capacity(source_content.len() + config_content.len() + 128);
    buf.extend_from_slice(source_content);
    buf.push(0xFF);
    buf.extend_from_slice(config_content);
    buf.push(0xFF);
    buf.extend_from_slice(target_name.as_bytes());
    for o in overrides {
        buf.push(0xFF);
        buf.extend_from_slice(o.as_bytes());
    }
    // Truncate to u64 -- collisions are harmless (just a redundant re-render)
    xxh3_128(&buf) as u64
}

/// Load the page cache from disk. Returns empty map on any error.
pub fn load(output_dir: &Path) -> HashMap<String, u64> {
    let path = output_dir.join(".page_cache.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Save the page cache to disk.
pub fn save(output_dir: &Path, cache: &HashMap<String, u64>) {
    let path = output_dir.join(".page_cache.json");
    if let Ok(json) = serde_json::to_string(cache) {
        let _ = fs::write(&path, json);
    }
}
