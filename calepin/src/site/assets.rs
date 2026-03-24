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
/// Also copies built-in target-scoped assets as fallback for files not
/// present in the project's assets/ directory.
/// If `static_dirs` is non-empty, also copies those directories into output.
pub fn copy_assets(base_dir: &Path, output_dir: &Path, static_dirs: &[String]) -> Result<()> {
    let assets_dst = output_dir.join("assets");

    // Copy project assets first (user files take priority)
    let assets_src = base_dir.join("_calepin").join("assets");
    if assets_src.is_dir() {
        copy_dir_recursive(&assets_src, &assets_dst)
            .context("Failed to copy assets/ to output directory")?;
    }

    // Copy built-in target-scoped assets as fallback.
    // Resolve the active target name to find assets/{target}/ in BUILTIN_PROJECT.
    if let Some(target_name) = crate::paths::get_active_target() {
        let builtin_path = format!("assets/{}", target_name);
        if let Some(builtin_dir) = crate::render::elements::BUILTIN_PROJECT.get_dir(&builtin_path) {
            fs::create_dir_all(&assets_dst)?;
            for file in builtin_dir.files() {
                let name = file.path().file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                if name.is_empty() { continue; }
                let dst_file = assets_dst.join(name);
                // Only copy if project doesn't have this file
                if !dst_file.exists() {
                    if let Some(content) = file.contents_utf8() {
                        fs::write(&dst_file, content)?;
                    } else {
                        fs::write(&dst_file, file.contents())?;
                    }
                }
            }
        }
    }

    // Copy user-specified static paths (files or directories) into output
    for entry in static_dirs {
        let src = base_dir.join(entry);
        let dst = output_dir.join(entry);
        if src.is_dir() {
            copy_dir_recursive(&src, &dst)
                .with_context(|| format!("Failed to copy static directory '{}' to output", entry))?;
        } else if src.is_file() {
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&src, &dst)
                .with_context(|| format!("Failed to copy static file '{}' to output", entry))?;
        }
    }

    Ok(())
}
