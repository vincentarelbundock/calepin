use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use walkdir::WalkDir;

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

/// Copy `_calepin/assets/` directory to `_assets/` in the output directory.
pub fn copy_assets(base_dir: &Path, output_dir: &Path) -> Result<()> {
    let assets_src = base_dir.join("_calepin/assets");
    if !assets_src.is_dir() {
        return Ok(());
    }
    let assets_dst = output_dir.join("_assets");
    copy_dir_recursive(&assets_src, &assets_dst)
        .context("Failed to copy _assets/ to output directory")
}
