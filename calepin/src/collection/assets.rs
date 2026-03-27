use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::paths::copy_dir_recursive;

/// Copy `_calepin/assets/` directory to `assets/` in the output directory.
/// Also copies built-in target-scoped assets as fallback for files not
/// present in the project's assets/ directory.
/// If `static_dirs` is non-empty, also copies those directories into output.
pub fn copy_assets(base_dir: &Path, output_dir: &Path, static_dirs: &[String]) -> Result<()> {
    let assets_dst = output_dir.join("assets");

    let calepin_assets = crate::paths::assets_dir(&base_dir);
    if calepin_assets.is_dir() {
        copy_dir_recursive(&calepin_assets, &assets_dst)
            .context("Failed to copy _calepin/assets/ to output directory")?;
    }

    // Copy built-in assets as fallback (recursively).
    {
        if let Some(builtin_dir) = crate::render::elements::BUILTIN_ASSETS.get_dir("assets") {
            fs::create_dir_all(&assets_dst)?;
            copy_builtin_dir_recursive(builtin_dir, Path::new("assets"), &assets_dst)?;
        }
    }

    // Copy base page.css as fallback (the website template links both page.css and calepin.css).
    let page_css_dst = assets_dst.join("page.css");
    if !page_css_dst.exists() {
        fs::create_dir_all(&assets_dst)?;
        let css = crate::render::template::load_default_css();
        fs::write(&page_css_dst, css)?;
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

/// Recursively copy files from an embedded `include_dir` directory into `dst`,
/// skipping files that already exist (project overrides take priority).
fn copy_builtin_dir_recursive(
    dir: &include_dir::Dir<'static>,
    strip_prefix: &Path,
    dst: &Path,
) -> Result<()> {
    for file in dir.files() {
        let rel = file.path().strip_prefix(strip_prefix).unwrap_or(file.path());
        let dst_file = dst.join(rel);
        if dst_file.exists() { continue; }
        if let Some(parent) = dst_file.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Some(content) = file.contents_utf8() {
            fs::write(&dst_file, content)?;
        } else {
            fs::write(&dst_file, file.contents())?;
        }
    }
    for subdir in dir.dirs() {
        copy_builtin_dir_recursive(subdir, strip_prefix, dst)?;
    }
    Ok(())
}
