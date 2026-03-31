use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::paths::copy_dir_recursive;

/// Copy sidecar `assets/` directory to `assets/` in the output directory.
/// CSS and JS files from `assets/css/` and `assets/js/` are concatenated
/// (alphabetically by filename) into `assets/calepin.css` and `assets/calepin.js`.
/// If `static_dirs` is non-empty, also copies those directories into output.
pub fn copy_assets(base_dir: &Path, output_dir: &Path, static_dirs: &[String]) -> Result<()> {
    let assets_dst = output_dir.join("assets");
    fs::create_dir_all(&assets_dst)?;

    // Copy sidecar assets (always populated with built-in assets)
    let calepin_assets = crate::paths::assets_dir(&base_dir);
    if calepin_assets.is_dir() {
        copy_dir_recursive(&calepin_assets, &assets_dst)
            .context("Failed to copy sidecar assets/ to output directory")?;
    }

    // Concatenate css/*.css -> calepin.css and js/*.js -> calepin.js in output,
    // then remove the source subdirectories (only the concatenated files are served).
    concatenate_and_write(&assets_dst, "css", "css", "calepin.css")?;
    concatenate_and_write(&assets_dst, "js", "js", "calepin.js")?;
    let _ = fs::remove_dir_all(assets_dst.join("css"));
    let _ = fs::remove_dir_all(assets_dst.join("js"));

    // Copy extension assets to assets/{ext_name}/
    // Each extension's assets/ directory is copied wholesale under its name,
    // so paths are predictable and collision-free.
    {
        let target_name = crate::paths::get_active_target().unwrap_or_default();
        let chain: Vec<(String, std::path::PathBuf)> =
            crate::config::extension::walk_chain(base_dir, &target_name, |name, ext_dir, _manifest| {
                let assets_dir = ext_dir.join("assets");
                if assets_dir.is_dir() {
                    Some((name.to_string(), assets_dir))
                } else {
                    None
                }
            });
        // Parent-first order so child extensions override parent files
        for (ext_name, src_assets) in chain.iter().rev() {
            let ext_dst = assets_dst.join(ext_name);
            copy_dir_recursive(&src_assets, &ext_dst)
                .with_context(|| format!("Failed to copy extension '{}' assets", ext_name))?;
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

/// Concatenate all files with the given extension from a subdirectory,
/// sorted alphabetically by filename, and write to a single output file.
fn concatenate_and_write(
    assets_dir: &Path,
    subdir: &str,
    ext: &str,
    output_name: &str,
) -> Result<()> {
    let dir = assets_dir.join(subdir);
    if !dir.is_dir() {
        return Ok(());
    }
    let mut files: Vec<_> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == ext))
        .collect();
    files.sort_by_key(|e| e.file_name());
    let content: String = files
        .iter()
        .filter_map(|e| fs::read_to_string(e.path()).ok())
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(assets_dir.join(output_name), content)?;
    Ok(())
}
