//! Collection configuration: loads `_calepin.toml` and converts to `Metadata`.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::metadata::Metadata;

/// Load and validate a project config, returning it as `Metadata`.
/// Looks for `_calepin.toml` at the given path or in `base_dir`.
pub fn load_config(config_path: Option<&Path>, base_dir: &Path) -> Result<(Metadata, PathBuf)> {
    if let Some(path) = config_path {
        let config = crate::project::load_project_metadata(path)?;
        return Ok((config, path.to_path_buf()));
    }

    let path = base_dir.join("_calepin.toml");
    if path.exists() {
        let meta = crate::project::load_project_metadata(&path)?;
        return Ok((meta, path));
    }

    anyhow::bail!(
        "No config found. Create _calepin.toml in {}",
        base_dir.display()
    )
}

/// Collect all .qmd page paths from [[contents]] (excluding standalone), expanding globs.
pub fn collect_document_paths(meta: &Metadata, base_dir: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    for section in &meta.contents {
        if section.standalone {
            continue;
        }
        if let Some(ref idx) = section.index {
            if idx.ends_with(".qmd") {
                paths.push(idx.clone());
            }
        }
        for entry in &section.pages {
            for path in crate::project::expand_glob_pub(entry.path(), base_dir) {
                if path.ends_with(".qmd") {
                    paths.push(path);
                }
            }
        }
    }
    paths
}

/// Collect standalone page paths (rendered but not in nav).
pub fn collect_standalone_paths(meta: &Metadata, base_dir: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    for section in &meta.contents {
        if !section.standalone {
            continue;
        }
        for entry in &section.pages {
            for path in crate::project::expand_glob_pub(entry.path(), base_dir) {
                if path.ends_with(".qmd") {
                    paths.push(path);
                }
            }
        }
    }
    paths
}
