//! Collection configuration: loads project config and converts to `Metadata`.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::config::Metadata;

/// Load and validate a project config, returning it as `Metadata`.
/// Looks for `_calepin/config.toml` or `_calepin/config.toml` in `base_dir`.
pub fn load_config(config_path: Option<&Path>, base_dir: &Path) -> Result<(Metadata, PathBuf)> {
    if let Some(path) = config_path {
        let config = crate::config::load_project_metadata(path)?;
        return Ok((config, path.to_path_buf()));
    }

    if let Some(path) = crate::cli::find_project_config(base_dir) {
        let meta = crate::config::load_project_metadata(&path)?;
        return Ok((meta, path));
    }

    anyhow::bail!(
        "No config found. Create _calepin/config.toml in {}",
        base_dir.display()
    )
}

/// Collect .qmd page paths listed in [[contents]], expanding globs.
/// These are the pages that appear in navigation (sidebar, prev/next).
pub fn collect_document_paths(meta: &Metadata, base_dir: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    for section in &meta.contents {
        if let Some(ref idx) = section.index {
            if idx.ends_with(".qmd") {
                paths.push(idx.clone());
            }
        }
        for entry in &section.pages {
            for path in super::contents::expand_glob_pub(entry.path(), base_dir) {
                if path.ends_with(".qmd") {
                    paths.push(path);
                }
            }
        }
    }
    paths
}
