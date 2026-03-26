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

/// Extensions for auxiliary files whose changes should invalidate the page cache.
const AUX_EXTENSIONS: &[&str] = &["css", "js", "html", "tex", "md", "typ"];

/// Collect content of all auxiliary files (.css, .js, .html, .tex, .md, .typ)
/// in `base_dir` and its `_calepin/` subdirectory into a byte buffer.
/// Append this to `config_bytes` so that partial/stylesheet/template changes
/// invalidate the cache for every page.
pub fn collect_auxiliary_bytes(base_dir: &Path) -> Vec<u8> {
    let mut buf = Vec::new();
    collect_from_dir(&mut buf, base_dir);
    let calepin_dir = base_dir.join("_calepin");
    if calepin_dir.is_dir() {
        collect_from_dir(&mut buf, &calepin_dir);
    }
    buf
}

fn collect_from_dir(buf: &mut Vec<u8>, dir: &Path) {
    let mut paths = Vec::new();
    collect_paths_recursive(dir, &mut paths);
    paths.sort();
    for path in paths {
        if let Ok(content) = fs::read(&path) {
            buf.extend_from_slice(path.to_string_lossy().as_bytes());
            buf.push(0xFF);
            buf.extend_from_slice(&content);
            buf.push(0xFF);
        }
    }
}

fn collect_paths_recursive(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_paths_recursive(&path, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if AUX_EXTENSIONS.contains(&ext) {
                out.push(path);
            }
        }
    }
}

/// Compute a cache key for a single page.
///
/// Inputs mixed into the hash:
/// - source file content (the .qmd body)
/// - config content (config file + auxiliary file bytes)
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
