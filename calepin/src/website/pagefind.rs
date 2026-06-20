use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{anyhow, Context, Result};
use xxhash_rust::xxh3::xxh3_64;

use super::outputs::remove_stale_output_file;
use super::paths::{ensure_path_within_root, ensure_relative_path, rel_posix};
use super::{PageInfoMap, WebsiteManifest};

pub(super) const PAGEFIND_DIR: &str = "pagefind";
pub(super) const PAGEFIND_CSS: &str = "pagefind/pagefind-component-ui.css";
pub(super) const PAGEFIND_JS: &str = "pagefind/pagefind-component-ui.js";
const PAGEFIND_ROOT_SELECTOR: &str = "[data-pagefind-body]";

static PAGEFIND_RUNTIME: OnceLock<std::result::Result<tokio::runtime::Runtime, String>> =
    OnceLock::new();

pub(super) fn pagefind_pages(
    out_dir: &Path,
    typ_files: &[PathBuf],
    page_info: &PageInfoMap,
    fallback_files: &[PathBuf],
) -> Vec<(PathBuf, String)> {
    typ_files
        .iter()
        .filter(|path| !fallback_files.contains(path))
        .filter_map(|path| {
            // Pagefind prepends its runtime `baseUrl` to indexed URLs. That base URL is
            // inferred from the loaded bundle path (for example `/calepin/pagefind/` on
            // GitHub Pages), so storing a site `base_url` prefix here would duplicate it
            // in search result links.
            page_info
                .get(path)
                .map(|info| (out_dir.join(&info.href), info.href.clone()))
        })
        .collect()
}

pub(super) fn base_url_path_prefix(base_url: &str) -> Option<String> {
    let after_host = base_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(base_url);
    let path = after_host
        .find('/')
        .map(|index| &after_host[index..])
        .unwrap_or("");
    let path = path
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .trim_end_matches('/');
    (!path.is_empty()).then(|| path.to_string())
}

pub(super) fn pagefind_signature(out_dir: &Path, pages: &[(PathBuf, String)]) -> Result<u64> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"calepin-pagefind-v1\0");
    bytes.extend_from_slice(PAGEFIND_ROOT_SELECTOR.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(b"keep_index_url=true\0");
    let mut pages = pages.to_vec();
    pages.sort_by(|left, right| {
        rel_posix(out_dir, &left.0)
            .cmp(&rel_posix(out_dir, &right.0))
            .then_with(|| left.1.cmp(&right.1))
    });
    for (path, url) in pages {
        let path = ensure_path_within_root(out_dir, &path, "pagefind input path")?;
        bytes.extend_from_slice(rel_posix(out_dir, path).as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(url.as_bytes());
        bytes.push(0);
        let html = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
        bytes.extend_from_slice(&(html.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&html);
        bytes.push(0xff);
    }
    Ok(xxh3_64(&bytes))
}

pub(super) fn manifest_output_paths(
    out_dir: &Path,
    outputs: &[String],
) -> Result<BTreeSet<PathBuf>> {
    outputs
        .iter()
        .map(|path| pagefind_output_path(out_dir, path.as_str()))
        .collect()
}

pub(super) fn cached_pagefind_outputs(
    out_dir: &Path,
    manifest: &WebsiteManifest,
    signature: u64,
) -> Result<Option<BTreeSet<PathBuf>>> {
    let Some(pagefind) = manifest.pagefind.as_ref() else {
        return Ok(None);
    };
    if pagefind.signature != signature {
        return Ok(None);
    }
    let outputs = manifest_output_paths(out_dir, &pagefind.outputs)?;
    if outputs.iter().all(|path| path.is_file()) {
        Ok(Some(outputs))
    } else {
        Ok(None)
    }
}

pub(super) fn remove_stale_pagefind_outputs(
    out_dir: &Path,
    manifest: &WebsiteManifest,
    expected_outputs: &BTreeSet<PathBuf>,
) -> Result<()> {
    let Some(pagefind) = manifest.pagefind.as_ref() else {
        return Ok(());
    };
    for path in manifest_output_paths(out_dir, &pagefind.outputs)? {
        if expected_outputs.contains(&path) || !path.exists() {
            continue;
        }
        remove_stale_output_file(&path, "stale pagefind output")?;
    }
    Ok(())
}

pub(super) fn write_pagefind_index(
    out_dir: &Path,
    pages: &[(PathBuf, String)],
) -> Result<BTreeSet<PathBuf>> {
    if pages.is_empty() {
        return Ok(BTreeSet::new());
    }
    pagefind_runtime()?.block_on(write_pagefind_index_async(
        out_dir.to_path_buf(),
        pages.to_vec(),
    ))
}

fn pagefind_runtime() -> Result<&'static tokio::runtime::Runtime> {
    PAGEFIND_RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|error| anyhow!("failed to start Pagefind runtime: {error}"))
}

async fn write_pagefind_index_async(
    out_dir: PathBuf,
    pages: Vec<(PathBuf, String)>,
) -> Result<BTreeSet<PathBuf>> {
    let options = pagefind::options::PagefindServiceConfig::builder()
        .root_selector(PAGEFIND_ROOT_SELECTOR.to_string())
        .keep_index_url(true)
        .build();
    let mut index = pagefind::api::PagefindIndex::new(Some(options))
        .context("failed to initialize Pagefind")?;

    for (path, url) in pages {
        let html = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("failed to read {}", path.display()))?;
        index
            .add_html_file(Some(path.to_string_lossy().into_owned()), Some(url), html)
            .await
            .with_context(|| format!("failed to index {}", path.display()))?;
    }

    let files = index
        .get_files()
        .await
        .context("failed to build Pagefind index")?;
    let mut outputs = BTreeSet::new();
    for file in files {
        let path = pagefind_output_path(&out_dir, &file.filename)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        tokio::fs::write(&path, file.contents)
            .await
            .with_context(|| format!("failed to write {}", path.display()))?;
        outputs.insert(path);
    }
    Ok(outputs)
}

fn pagefind_output_path<P: AsRef<Path>>(out_dir: &Path, rel: P) -> Result<PathBuf> {
    let rel = rel.as_ref();
    let rel = ensure_relative_path(rel, "Pagefind output path")?;
    let is_scoped_to_pagefind = rel.starts_with(PAGEFIND_DIR);
    Ok(if is_scoped_to_pagefind {
        out_dir.join(rel)
    } else {
        out_dir.join(PAGEFIND_DIR).join(rel)
    })
}
