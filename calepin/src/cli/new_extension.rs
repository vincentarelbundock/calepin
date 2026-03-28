//! `calepin init extension` -- scaffold a new extension directory.

use std::path::Path;

use anyhow::{bail, Result};

/// Scaffold a new extension directory.
pub fn handle_new_extension(name: &str, inherits: &str) -> Result<()> {
    let dir = Path::new(name);
    if dir.exists() {
        bail!("Directory already exists: {}", dir.display());
    }

    // Validate inherits target
    let valid_parents = ["html", "latex", "typst", "markdown", "revealjs", "website", "book"];
    if !valid_parents.contains(&inherits) {
        // Allow any name -- it could be a user-installed extension
        eprintln!(
            "Note: '{}' is not a built-in target. Make sure it exists as an installed extension.",
            inherits
        );
    }

    // Determine writer from parent for partial directory naming
    let writer = match inherits {
        "html" | "revealjs" | "website" | "book" => "html",
        "latex" => "latex",
        "typst" => "typst",
        "markdown" => "markdown",
        _ => "html", // default fallback
    };

    // Create directory structure
    std::fs::create_dir_all(dir)?;
    let partials_dir = dir.join("partials").join(writer);
    std::fs::create_dir_all(&partials_dir)?;
    std::fs::write(partials_dir.join(".gitkeep"), "")?;
    let assets_dir = dir.join("assets");
    std::fs::create_dir_all(&assets_dir)?;
    std::fs::write(assets_dir.join(".gitkeep"), "")?;

    // Write extension.toml
    let manifest = format!(
        r#"name = "{name}"
description = ""
version = "0.1.0"
inherits = "{inherits}"

[target]
# modules = ["highlight", "append_footnotes", "embed_images"]

[assets]
# css = []
# js = []

# Declare [[modules]] only if you need structural transforms,
# match rules beyond class names, auto-numbering, or external code.
# For simple div styles, just add a partial.
"#,
        name = name,
        inherits = inherits,
    );
    std::fs::write(dir.join("extension.toml"), manifest)?;

    eprintln!("Created extension in {}/", dir.display());
    eprintln!();
    eprintln!("  {}/", name);
    eprintln!("  |-- extension.toml");
    eprintln!("  |-- partials/");
    eprintln!("  |   `-- {}/", writer);
    eprintln!("  `-- assets/");
    eprintln!();
    eprintln!("To install: copy this directory to your project's _calepin/extensions/");

    Ok(())
}
