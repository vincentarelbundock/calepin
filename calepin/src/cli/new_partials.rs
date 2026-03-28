//! `calepin new partials`: overwrite local partials with built-in templates.

use std::path::PathBuf;

use anyhow::Result;

use crate::render::elements::BUILTIN_PARTIALS;

/// Find all partial directories in the project and overwrite their contents
/// with the latest built-in templates.
pub fn handle_new_partials() -> Result<()> {
    let cwd = std::env::current_dir()?;

    // Collect all partial directories: _calepin/partials/ and {stem}_calepin/partials/
    let mut partial_dirs: Vec<PathBuf> = Vec::new();

    let main_partials = cwd.join("_calepin").join("partials");
    if main_partials.is_dir() {
        partial_dirs.push(main_partials);
    }

    // Find sidecar partial dirs: {stem}_calepin/partials/
    for entry in std::fs::read_dir(&cwd)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.ends_with("_calepin") && name_str != "_calepin" && entry.path().is_dir() {
            let sidecar_partials = entry.path().join("partials");
            if sidecar_partials.is_dir() {
                partial_dirs.push(sidecar_partials);
            }
        }
    }

    if partial_dirs.is_empty() {
        eprintln!("No partial directories found in {}", cwd.display());
        eprintln!("Expected: _calepin/partials/ or {{stem}}_calepin/partials/");
        return Ok(());
    }

    // Count files that will be overwritten
    let mut files_to_write: Vec<(PathBuf, &str)> = Vec::new();
    for dir in &partial_dirs {
        collect_overwrites(dir, &mut files_to_write)?;
    }

    if files_to_write.is_empty() {
        eprintln!("No matching built-in templates found for the local partials.");
        return Ok(());
    }

    eprintln!("This will overwrite {} file(s) in:", files_to_write.len());
    for dir in &partial_dirs {
        let rel = dir.strip_prefix(&cwd).unwrap_or(dir);
        eprintln!("  {}/", rel.display());
    }
    eprint!("Continue? [y/N] ");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if !input.trim().eq_ignore_ascii_case("y") {
        eprintln!("Aborted.");
        return Ok(());
    }

    // Write files
    let mut written = 0;
    for (path, content) in &files_to_write {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        written += 1;
    }

    eprintln!("Updated {} file(s).", written);
    Ok(())
}

/// For a partials directory, find all files that have a matching built-in template
/// and collect them for overwriting.
fn collect_overwrites<'a>(
    partials_dir: &std::path::Path,
    files: &mut Vec<(PathBuf, &'a str)>,
) -> Result<()> {
    // Walk the user's partials directory to discover which engine/target dirs exist
    for entry in walkdir::WalkDir::new(partials_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let rel = entry.path().strip_prefix(partials_dir)
            .unwrap_or(entry.path());
        let rel_str = rel.display().to_string();

        // Check if there's a matching built-in template
        if let Some(file) = BUILTIN_PARTIALS.get_file(&rel_str) {
            if let Some(content) = file.contents_utf8() {
                files.push((entry.path().to_path_buf(), content));
            }
        }
    }
    Ok(())
}
