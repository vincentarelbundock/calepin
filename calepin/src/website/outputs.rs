use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use super::pagefind::PAGEFIND_DIR;
use super::paths::rel_posix;
use super::{PageInfoMap, PagefindManifest, WebsiteManifest, SKIP_DIRS};

pub(super) const MANIFEST_PATH: &str = ".calepin/website-manifest.json";
const DEFAULT_FAVICON_SVG: &str = include_str!("../assets/default-favicon.svg");

pub(super) struct GeneratedOutputInputs<'a> {
    pub out_dir: &'a Path,
    pub typ_files: &'a [PathBuf],
    pub page_info: &'a PageInfoMap,
    pub sitemap_path: &'a Option<PathBuf>,
    pub robots_path: &'a Option<PathBuf>,
    pub feed_paths: &'a BTreeSet<PathBuf>,
    pub theme_asset_paths: &'a BTreeSet<PathBuf>,
    pub default_favicon_path: Option<&'a Path>,
}

pub(super) fn expected_generated_outputs(inputs: GeneratedOutputInputs<'_>) -> BTreeSet<PathBuf> {
    let mut outputs = BTreeSet::new();
    for input_path in inputs.typ_files {
        if let Some(info) = inputs.page_info.get(input_path) {
            outputs.insert(inputs.out_dir.join(&info.href));
            if let Some(pdf_href) = &info.pdf_href {
                outputs.insert(inputs.out_dir.join(pdf_href));
            }
        }
    }
    if let Some(path) = inputs.sitemap_path {
        outputs.insert(path.clone());
    }
    if let Some(path) = inputs.robots_path {
        outputs.insert(path.clone());
    }
    outputs.extend(inputs.feed_paths.iter().cloned());
    outputs.extend(inputs.theme_asset_paths.iter().cloned());
    if let Some(path) = inputs.default_favicon_path {
        outputs.insert(inputs.out_dir.join(path));
    }
    outputs
}

pub(super) fn write_default_favicon(out_dir: &Path, path: &Path) -> Result<()> {
    let path = out_dir.join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&path, DEFAULT_FAVICON_SVG)
        .with_context(|| format!("failed to write {}", path.display()))
}

pub(super) fn load_manifest(out_dir: &Path) -> Result<WebsiteManifest> {
    let path = out_dir.join(MANIFEST_PATH);
    if !path.is_file() {
        return Ok(WebsiteManifest::default());
    }
    let contents =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))
}

pub(super) fn reconcile_manifest_outputs(
    out_dir: &Path,
    manifest: &WebsiteManifest,
    expected_outputs: &BTreeSet<PathBuf>,
) -> Result<()> {
    for rel in &manifest.outputs {
        let path = out_dir.join(Path::new(rel));
        if expected_outputs.contains(&path) || !path.exists() {
            continue;
        }
        if path.is_file() {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove stale output {}", path.display()))?;
        }
    }
    Ok(())
}

pub(super) fn remove_unexpected_rendered_outputs(
    out_dir: &Path,
    expected_outputs: &BTreeSet<PathBuf>,
) -> Result<()> {
    if !out_dir.is_dir() {
        return Ok(());
    }
    remove_unexpected_rendered_outputs_in(out_dir, out_dir, expected_outputs)
}

fn remove_unexpected_rendered_outputs_in(
    root: &Path,
    dir: &Path,
    expected_outputs: &BTreeSet<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let Some(first) = rel.components().next() else {
            continue;
        };
        if first
            .as_os_str()
            .to_str()
            .is_some_and(|name| SKIP_DIRS.contains(&name))
        {
            continue;
        }
        if path.is_dir() {
            remove_unexpected_rendered_outputs_in(root, &path, expected_outputs)?;
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("html" | "pdf")
        ) && !expected_outputs.contains(&path)
        {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove stale output {}", path.display()))?;
        }
    }
    Ok(())
}

pub(super) fn write_manifest(
    out_dir: &Path,
    expected_outputs: &BTreeSet<PathBuf>,
    pagefind: Option<PagefindManifest>,
) -> Result<()> {
    let path = out_dir.join(MANIFEST_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let manifest = WebsiteManifest {
        outputs: expected_outputs
            .iter()
            .map(|path| rel_posix(out_dir, path))
            .collect(),
        pagefind,
    };
    let contents = serde_json::to_string_pretty(&manifest)?;
    fs::write(&path, contents).with_context(|| format!("failed to write {}", path.display()))
}

pub(super) fn clear_previous_outputs(
    src_dir: &Path,
    out_dir: &Path,
    preserve_pagefind: bool,
) -> Result<()> {
    if out_dir == src_dir {
        return Ok(());
    } else if out_dir.exists() {
        ensure_safe_clean_target(out_dir)?;
        for entry in fs::read_dir(out_dir)
            .with_context(|| format!("failed to read {}", out_dir.display()))?
        {
            let path = entry?.path();
            let name = path.file_name().and_then(|name| name.to_str());
            if path.is_dir() {
                if name.is_some_and(|name| {
                    SKIP_DIRS.contains(&name) || (preserve_pagefind && name == PAGEFIND_DIR)
                }) {
                    continue;
                }
                fs::remove_dir_all(&path)
                    .with_context(|| format!("failed to remove {}", path.display()))?;
            } else if name != Some(".gitkeep") {
                fs::remove_file(&path)
                    .with_context(|| format!("failed to remove {}", path.display()))?;
            }
        }
    }
    Ok(())
}

fn ensure_safe_clean_target(out_dir: &Path) -> Result<()> {
    if out_dir.join(MANIFEST_PATH).is_file() || output_dir_is_effectively_empty(out_dir)? {
        return Ok(());
    }
    bail!(
        "refusing to clean non-empty output directory {} because it does not contain {}; choose an empty/dedicated output directory or remove it manually",
        out_dir.display(),
        MANIFEST_PATH
    )
}

fn output_dir_is_effectively_empty(out_dir: &Path) -> Result<bool> {
    for entry in
        fs::read_dir(out_dir).with_context(|| format!("failed to read {}", out_dir.display()))?
    {
        let path = entry?.path();
        let name = path.file_name().and_then(|name| name.to_str());
        if name == Some(".gitkeep")
            || path.is_dir() && name.is_some_and(|name| SKIP_DIRS.contains(&name))
        {
            continue;
        }
        return Ok(false);
    }
    Ok(true)
}

pub(super) fn copy_typ_sources(
    src_dir: &Path,
    out_dir: &Path,
    typ_files: &[PathBuf],
) -> Result<()> {
    for input_path in typ_files {
        let rel = input_path.strip_prefix(src_dir).unwrap_or(input_path);
        let target = out_dir.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::copy(input_path, &target).with_context(|| {
            format!(
                "failed to copy {} to {}",
                input_path.display(),
                target.display()
            )
        })?;
    }
    Ok(())
}

pub(super) fn static_output_paths(
    src_dir: &Path,
    out_dir: &Path,
    static_files: &[PathBuf],
) -> BTreeSet<PathBuf> {
    static_files
        .iter()
        .map(|path| {
            let rel = path.strip_prefix(src_dir).unwrap_or(path);
            out_dir.join(rel)
        })
        .collect()
}

pub(super) fn copy_static_files(
    src_dir: &Path,
    out_dir: &Path,
    static_files: &[PathBuf],
) -> Result<()> {
    for input_path in static_files {
        let rel = input_path.strip_prefix(src_dir).unwrap_or(input_path);
        let target = out_dir.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        if target.is_dir() {
            fs::remove_dir_all(&target)
                .with_context(|| format!("failed to remove {}", target.display()))?;
        }
        fs::copy(input_path, &target).with_context(|| {
            format!(
                "failed to copy static file {} to {}",
                input_path.display(),
                target.display()
            )
        })?;
    }
    Ok(())
}
