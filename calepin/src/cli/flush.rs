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

    // Collect directories and files to delete
    let mut targets: Vec<PathBuf> = Vec::new();
    let latex_exts = ["aux", "log", "out", "toc", "fls", "fdb_latexmk", "synctex.gz", "xdv"];

    // Search recursively for directories whose name matches the stem filter.
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

    // Walk recursively to find matching artefacts
    fn find_targets(dir: &Path, targets: &mut Vec<PathBuf>, latex_exts: &[&str],
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
                if name == "_calepin" {
                    // Check for cache/ and files/ inside _calepin/
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
                } else if name != "." && name != ".." && name != ".git" && name != "node_modules" {
                    find_targets(&p, targets, latex_exts, do_cache, do_files, do_compilation, stem_filter);
                }
            } else if do_compilation && p.is_file() && stem_filter.is_none() {
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if latex_exts.contains(&ext) {
                        targets.push(p);
                    }
                }
            }
        }
    }
    find_targets(&root, &mut targets, &latex_exts, do_cache, do_files, do_compilation, stem);

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

    eprintln!("Done.");
    Ok(())
}
