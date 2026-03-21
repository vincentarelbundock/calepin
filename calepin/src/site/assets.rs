use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use walkdir::WalkDir;

// Built-in CSS and JS embedded at compile time
const SITE_CSS: &str = include_str!("built_in/css/site.css");
const SEARCH_JS: &str = include_str!("built_in/js/search.js");
const THEME_JS: &str = include_str!("built_in/js/theme.js");

/// Copy resource directories listed in project config to the output directory.
pub fn copy_resources(resources: &[String], base_dir: &Path, output_dir: &Path) -> Result<()> {
    for resource in resources {
        let src = base_dir.join(resource);
        if !src.exists() {
            eprintln!("Warning: resource not found: {}", src.display());
            continue;
        }

        if src.is_dir() {
            copy_dir_recursive(&src, &output_dir.join(resource))?;
        } else {
            let dest = output_dir.join(resource);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&src, &dest)
                .with_context(|| format!("Failed to copy {}", src.display()))?;
        }
    }
    Ok(())
}

/// Copy a directory tree recursively.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    for entry in WalkDir::new(src) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src)?;
        let target = dst.join(rel);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Write built-in CSS and JS assets to the output directory.
pub fn write_builtin_assets(output_dir: &Path) -> Result<()> {
    let assets_dir = output_dir.join("_assets");
    fs::create_dir_all(&assets_dir)?;

    fs::write(assets_dir.join("site.css"), SITE_CSS)?;
    fs::write(assets_dir.join("search.js"), SEARCH_JS)?;
    fs::write(assets_dir.join("theme.js"), THEME_JS)?;

    Ok(())
}
