//! Collection configuration: reads from `_calepin.toml` using the project config schema.
//!
//! The collection builder reads its config from `crate::project::ProjectConfig`.
//! Collection fields (`contents`, `target`, `logo`, etc.) live at the top level.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::project::ProjectConfig;

/// Load the project config, which contains all collection configuration.
/// Looks for `_calepin.toml` in `base_dir`.
pub fn load_config(config_path: Option<&Path>, base_dir: &Path) -> Result<(ProjectConfig, PathBuf)> {
    if let Some(path) = config_path {
        let config = crate::project::load_project_config(path)?;
        return Ok((config, path.to_path_buf()));
    }

    let path = base_dir.join("_calepin.toml");
    if path.exists() {
        let config = crate::project::load_project_config(&path)?;
        return Ok((config, path));
    }

    anyhow::bail!(
        "No config found. Create _calepin.toml in {}",
        base_dir.display()
    )
}

/// Collect all .qmd page paths from [[contents]] (excluding standalone), expanding globs.
pub fn collect_document_paths(meta: &crate::metadata::Metadata, base_dir: &Path) -> Vec<String> {
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
pub fn collect_standalone_paths(meta: &crate::metadata::Metadata, base_dir: &Path) -> Vec<String> {
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
