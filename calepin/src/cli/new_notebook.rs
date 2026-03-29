//! `calepin init notebook` -- scaffold a .qmd file with its sidecar directory.

use std::path::Path;

use anyhow::{bail, Result};
use include_dir::{include_dir, Dir};


static SCAFFOLD: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/scaffold/notebook");

pub fn handle_new_notebook(path: &Path, target_name: Option<&str>) -> Result<()> {
    // Ensure .qmd extension
    let path = if path.extension().is_some() {
        path.to_path_buf()
    } else {
        path.with_extension("qmd")
    };

    if path.exists() {
        bail!("{} already exists", path.display());
    }

    // Check sidecar directory (compute path without resolve_sidecar_dir,
    // which auto-creates the directory as a side effect)
    let stem = path.file_stem().unwrap().to_string_lossy();
    let sidecar = path.parent().unwrap_or(Path::new(".")).join(format!("{}_calepin", stem));
    if sidecar.exists() {
        bail!("Sidecar directory {} already exists", sidecar.display());
    }

    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    // Write the .qmd file from scaffold template
    let template = SCAFFOLD.get_file("notebook.qmd")
        .and_then(|f| f.contents_utf8())
        .unwrap_or("");
    std::fs::write(&path, template)?;

    // Apply theme assets if specified
    if let Some(_name) = target_name {
        let kind = crate::paths::ProjectKind::Document {
            qmd: path.clone(),
            sidecar: sidecar.clone(),
        };
        crate::themes::copy_builtin_assets(&kind)?;
    }

    eprintln!("Created {}", path.display());

    Ok(())
}
