use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::paths::copy_dir_recursive;

/// Copy `assets/` directory to `assets/` in the output directory.
/// Also copies built-in target-scoped assets as fallback for files not
/// present in the project's assets/ directory.
/// If `static_dirs` is non-empty, also copies those directories into output.
pub fn copy_assets(base_dir: &Path, output_dir: &Path, static_dirs: &[String]) -> Result<()> {
    let assets_dst = output_dir.join("assets");

    // Copy project assets: assets/ at project root first, then _calepin/assets/
    // overrides (so _calepin/assets/ takes priority).
    let root_assets = base_dir.join("assets");
    if root_assets.is_dir() {
        copy_dir_recursive(&root_assets, &assets_dst)
            .context("Failed to copy assets/ to output directory")?;
    }
    let calepin_assets = crate::paths::assets_dir(&base_dir);
    if calepin_assets.is_dir() {
        copy_dir_recursive(&calepin_assets, &assets_dst)
            .context("Failed to copy _calepin/assets/ to output directory")?;
    }

    // Copy built-in assets as fallback.
    {
        if let Some(builtin_dir) = crate::render::elements::BUILTIN_ASSETS.get_dir("assets") {
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
