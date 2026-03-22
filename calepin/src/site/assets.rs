use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use walkdir::WalkDir;

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

/// Copy `assets/` directory to `assets/` in the output directory.
pub fn copy_assets(base_dir: &Path, output_dir: &Path) -> Result<()> {
    let assets_src = base_dir.join("assets");
    if !assets_src.is_dir() {
        return Ok(());
    }
    let assets_dst = output_dir.join("assets");
    copy_dir_recursive(&assets_src, &assets_dst)
        .context("Failed to copy assets/ to output directory")
}
