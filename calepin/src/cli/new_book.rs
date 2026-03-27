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
    let calepin_dir = crate::paths::calepin_dir(dir, &[]);
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

    // Apply default theme (partials + assets)
    let theme = crate::themes::Theme::builtin_default();
    let kind = crate::config::paths::ProjectKind::Collection {
        root: dir.to_path_buf(),
        config: calepin_dir.join("config.toml"),
    };
    theme.apply_quiet(&kind)?;

    eprintln!("Created book project in {}/", dir.display());
    eprintln!();
    print_tree(dir, 2);
    eprintln!();
    eprintln!("To preview:  calepin preview {}", dir.display());

    Ok(())
}

/// Print a directory tree to stderr with a maximum depth.
fn print_tree(root: &Path, max_depth: usize) {
    eprintln!("  {}/", root.display());
    print_tree_dir(root, "", max_depth, 0);
}

fn print_tree_dir(dir: &Path, prefix: &str, max_depth: usize, depth: usize) {
    let mut entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return,
    };
    entries.sort_by_key(|e| e.file_name());

    let count = entries.len();
    for (i, entry) in entries.iter().enumerate() {
        let is_last = i + 1 == count;
        let connector = if is_last { "`-- " } else { "|-- " };
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if entry.path().is_dir() {
            eprintln!("  {}{}{}/", prefix, connector, name);
            if depth + 1 < max_depth {
                let child_prefix = if is_last {
                    format!("{}    ", prefix)
                } else {
                    format!("{}|   ", prefix)
                };
                print_tree_dir(&entry.path(), &child_prefix, max_depth, depth + 1);
            }
        } else {
            eprintln!("  {}{}{}", prefix, connector, name);
        }
    }
}
