//! `calepin new book` -- scaffold a book project.

use std::path::Path;

use anyhow::{bail, Context, Result};
use include_dir::{include_dir, Dir};

static SCAFFOLD: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/scaffold/book");

pub fn handle_new_book(dir: &Path) -> Result<()> {
    if dir.exists() {
        bail!("Directory already exists: {}", dir.display());
    }
    std::fs::create_dir_all(dir)
        .with_context(|| format!("Failed to create directory: {}", dir.display()))?;

    // Copy scaffold files
    crate::paths::write_embedded_dir(&SCAFFOLD, dir);

    // Compose config.toml: scaffold project config + shared defaults + collection defaults
    let calepin_dir = dir.join("_calepin");
    let project_config = SCAFFOLD.get_file("_calepin/config.toml")
        .and_then(|f| f.contents_utf8())
        .unwrap_or("");
    let composed = format!(
        "{}\n\n# === Built-in defaults ===\n\n{}\n{}",
        project_config.trim(),
        crate::config::SHARED_TOML,
        crate::config::COLLECTION_TOML,
    );
    std::fs::write(calepin_dir.join("config.toml"), composed)?;

    // Add built-in partials to _calepin/
    crate::paths::write_builtin_partials(&calepin_dir.join("partials"));

    // Print tree
    eprintln!("Created book project in {}/", dir.display());
    eprintln!();
    eprintln!("  {}/", dir.display());
    eprintln!("  |-- _calepin/");
    eprintln!("  |   `-- config.toml");
    eprintln!("  |-- references.bib");
    eprintln!("  `-- chapters/");
    eprintln!("      |-- index.qmd");
    eprintln!("      |-- 01-introduction.qmd");
    eprintln!("      |-- 02-methods.qmd");
    eprintln!("      `-- 03-results.qmd");
    eprintln!();
    eprintln!("To preview:  cd {} && calepin preview", dir.display());

    Ok(())
}
