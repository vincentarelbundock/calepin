//! `calepin new website` -- scaffold a website project.

use std::path::Path;

use anyhow::{bail, Context, Result};
use include_dir::{include_dir, Dir};

static SCAFFOLD: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/scaffold/website");

pub fn handle_new_website(dir: &Path) -> Result<()> {
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

    // Add built-in partials + website assets to _calepin/
    crate::paths::write_builtin_partials(&calepin_dir.join("partials"));
    crate::paths::write_builtin_assets("website", &calepin_dir.join("assets"));

    // Print tree
    // List scaffolded files (excluding _calepin internals)
    let mut files: Vec<String> = Vec::new();
    for file in SCAFFOLD.files() {
        let p = file.path().display().to_string();
        if !p.starts_with("_calepin/") {
            files.push(p);
        }
    }
    fn collect_files(dir: &include_dir::Dir<'static>, files: &mut Vec<String>) {
        for file in dir.files() {
            let p = file.path().display().to_string();
            if !p.starts_with("_calepin/") {
                files.push(p);
            }
        }
        for sub in dir.dirs() { collect_files(sub, files); }
    }
    files.clear();
    collect_files(&SCAFFOLD, &mut files);
    files.sort();

    eprintln!("Created website project in {}/", dir.display());
    eprintln!();
    eprintln!("  {}/", dir.display());
    eprintln!("  |-- _calepin/config.toml");
    for f in &files {
        eprintln!("  |-- {}", f);
    }
    eprintln!();
    eprintln!("To preview:  cd {} && calepin preview", dir.display());

    // Warn if pagefind is not installed
    if std::process::Command::new("pagefind").arg("--version").output().is_err() {
        eprintln!();
        eprintln!("\x1b[33mNote:\x1b[0m pagefind not found. Install it for search support:");
        eprintln!("  https://pagefind.app");
    }

    Ok(())
}
