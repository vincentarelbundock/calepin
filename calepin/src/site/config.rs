//! Site configuration: reads from `_calepin.toml` using the project config schema.
//!
//! The site builder reads its config from `crate::project::ProjectConfig`.
//! Site-specific fields come from `[meta]`, `[site]`, and `[var]`.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::project::ProjectConfig;

/// Load the project config, which contains all site configuration.
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

/// Collect all .qmd page paths from the [site].pages tree, expanding globs.
pub fn collect_page_paths(config: &ProjectConfig, base_dir: &Path) -> Vec<String> {
    let site = match &config.site {
        Some(s) => s,
        None => return Vec::new(),
    };
    let nodes = site.expand_pages(base_dir);
    let mut paths = Vec::new();
    for node in &nodes {
        match node {
            crate::project::PageNode::Page(p) => {
                if p.ends_with(".qmd") {
                    paths.push(p.clone());
                }
            }
            crate::project::PageNode::Section { pages, .. } => {
                for p in pages {
                    if p.ends_with(".qmd") {
                        paths.push(p.clone());
                    }
                }
            }
        }
    }
    paths
}

/// Collect standalone page paths (rendered but not in nav).
pub fn collect_standalone_paths(config: &ProjectConfig, base_dir: &Path) -> Vec<String> {
    use crate::project::SiteSection;
    let site = match &config.site {
        Some(s) => s,
        None => return Vec::new(),
    };
    let prefix = SiteSection::content_prefix(base_dir);
    let mut paths = Vec::new();
    for pattern in &site.content_standalone {
        let pattern = SiteSection::prefixed(pattern, prefix);
        for path in crate::project::expand_glob_pub(&pattern, base_dir) {
            if path.ends_with(".qmd") {
                paths.push(path);
            }
        }
    }
    paths
}
