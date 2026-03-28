//! `calepin init notebook` -- scaffold a .qmd file with its sidecar directory.

use std::path::Path;

use anyhow::{bail, Result};
use include_dir::{include_dir, Dir};

use crate::paths;

static SCAFFOLD: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/scaffold/notebook");

pub fn handle_new_notebook(path: &Path, theme_name: Option<&str>) -> Result<()> {
    if path.exists() {
        bail!("File already exists: {}", path.display());
    }

    // Ensure .qmd extension
    let path = if path.extension().is_some() {
        path.to_path_buf()
    } else {
        path.with_extension("qmd")
    };

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

    // Create the sidecar directory with default config
    let sidecar = paths::resolve_sidecar_dir(&path)
        .unwrap_or_else(|| {
            let stem = path.file_stem().unwrap().to_string_lossy();
            path.parent().unwrap_or(Path::new(".")).join(format!("{}_calepin", stem))
        });

    // Apply theme if specified
    if let Some(name) = theme_name {
        let theme = crate::themes::Theme::resolve(name)?;
        let kind = crate::paths::ProjectKind::Document {
            qmd: path.clone(),
            sidecar: sidecar.clone(),
        };
        theme.apply(&kind)?;
    }

    eprintln!("Created {}", path.display());
    eprintln!("Created {}/config.toml", sidecar.display());

    Ok(())
}
