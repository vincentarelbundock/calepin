//! The `calepin flush` command: clean up cache, generated files, and compilation artifacts.

use std::path::{Path, PathBuf};
use anyhow::Result;

pub fn handle_flush(path: &Path, stem: Option<&str>, skip_confirm: bool, do_cache: bool, do_files: bool, do_compilation: bool) -> Result<()> {
    use std::io::Write;

    let root = if path.is_relative() {
        std::env::current_dir()?.join(path)
    } else {
        path.to_path_buf()
    };

    let mut targets: Vec<PathBuf> = Vec::new();
    let latex_exts = ["aux", "log", "out", "toc", "fls", "fdb_latexmk", "synctex.gz", "xdv"];

    // Find the _calepin/ directory to search in.
    let calepin_dir = if root.file_name().unwrap_or_default().to_string_lossy().ends_with("_calepin") {
        root.clone()
    } else {
        root.join("_calepin")
    };

    if calepin_dir.is_dir() {
        find_targets_in_calepin(&calepin_dir, &mut targets, &latex_exts, do_cache, do_files, do_compilation, stem);
    }

    if targets.is_empty() {
        eprintln!("Nothing to clean.");
        return Ok(());
    }

    // Show what will be deleted
    for t in &targets {
        let display = t.strip_prefix(&root).unwrap_or(t);
        if t.is_dir() {
            eprintln!("  rm -rf {}/", display.display());
        } else {
            eprintln!("  rm {}", display.display());
        }
    }

    // Confirm
    if !skip_confirm {
        eprint!("\nDelete these? [y/N] ");
        std::io::stderr().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            eprintln!("Cancelled.");
            return Ok(());
        }
    }

    // Delete
    for t in &targets {
        if t.is_dir() {
            std::fs::remove_dir_all(t)?;
        } else {
            std::fs::remove_file(t)?;
        }
    }

    // Second pass: remove any empty directories left behind under _calepin/
    if calepin_dir.is_dir() {
        remove_empty_dirs(&calepin_dir);
    }

    eprintln!("Done.");
    Ok(())
}

/// Recursively search inside a _calepin/ directory for flush targets.
/// Looks for {stem}_calepin/cache/, {stem}_calepin/files/, and latex artifacts.
fn find_targets_in_calepin(dir: &Path, targets: &mut Vec<PathBuf>, latex_exts: &[&str],
                           do_cache: bool, do_files: bool, do_compilation: bool,
                           stem_filter: Option<&str>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with("_calepin") {
                // If the dir has no files, target the whole thing
                if !has_files(&p) {
                    targets.push(p.clone());
                } else {
                    if do_cache {
                        let cache = p.join("cache");
                        if cache.is_dir() {
                            if let Some(stem) = stem_filter {
                                find_matching_dirs(&cache, stem, targets);
                            } else {
                                targets.push(cache);
                            }
                        }
                    }
                    if do_files {
                        let files = p.join("files");
                        if files.is_dir() {
                            if let Some(stem) = stem_filter {
                                find_matching_dirs(&files, stem, targets);
                            } else {
                                targets.push(files);
                            }
                        }
                    }
                }
            }
            // Always recurse deeper
            find_targets_in_calepin(&p, targets, latex_exts, do_cache, do_files, do_compilation, stem_filter);
        } else if p.is_file() && do_compilation && stem_filter.is_none() {
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                if latex_exts.contains(&ext) {
                    targets.push(p);
                }
            }
        }
    }
}

/// Check whether a directory tree contains any files.
fn has_files(dir: &Path) -> bool {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() {
            return true;
        }
        if p.is_dir() && has_files(&p) {
            return true;
        }
    }
    false
}

/// Recursively remove empty directories (bottom-up). Returns true if the directory was removed.
fn remove_empty_dirs(dir: &Path) -> bool {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return false,
    };
    let mut has_remaining = false;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if !remove_empty_dirs(&p) {
                has_remaining = true;
            }
        } else {
            has_remaining = true;
        }
    }
    if !has_remaining {
        let _ = std::fs::remove_dir(dir);
        return true;
    }
    false
}

/// Search recursively for directories whose name matches the stem filter.
fn find_matching_dirs(dir: &Path, name: &str, targets: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if entry.file_name().to_string_lossy() == name {
                targets.push(p);
            } else {
                find_matching_dirs(&p, name, targets);
            }
        }
    }
}

