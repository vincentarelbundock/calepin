// ISC License
//
// Copyright (c) 2021, Timothee Mazzucotelli
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

//! Pure-Rust Python package documentation extractor.
//!
//! Replaces the Python-based `extract_pydocs.py` + griffe dependency with
//! a self-contained implementation that uses the ruff Python parser for AST
//! analysis and supports Google, NumPy, Sphinx, and markdown docstring styles.
//!
//! # Pipeline
//!
//! 1. Locate the installed package on disk via `python3 -c "..."`.
//! 2. Walk `.py` files and parse each with `ruff_python_parser`.
//! 3. Extract function/class definitions with docstrings.
//! 4. Parse docstrings into structured sections (or pass through as markdown).
//! 5. Filter to public API if `--all` is set.
//! 6. Render each object as a `.qmd` file in a nested directory structure.

pub mod extract;
pub mod google;
pub mod numpy;
pub mod render;
pub mod sphinx;
pub mod types;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use types::{DocstringStyle, PyObject};

/// Options for `handle_man_python`.
pub struct ManPythonOptions<'a> {
    /// Docstring style override (None = auto).
    pub style: Option<&'a str>,
    /// Only include names listed in `__all__`.
    pub exports_only: bool,
    /// Also include names re-exported via `__init__.py` imports.
    pub include_imports: bool,
    /// Include test directories and files.
    pub include_tests: bool,
    /// Include `_`-prefixed directories (internal modules).
    pub include_private: bool,
}

/// Extract Python package documentation and write `.qmd` files.
pub fn handle_man_python(
    package: &str,
    output: &Path,
    quiet: bool,
    opts: ManPythonOptions,
) -> Result<()> {
    let output_str = output.display().to_string();
    let doc_style = opts
        .style
        .map(DocstringStyle::from_str)
        .unwrap_or(DocstringStyle::Auto);

    if !quiet {
        eprintln!("Extracting Python docs for '{}' -> {}", package, output_str);
    }

    // Locate the package on disk
    let pkg_path = find_package_path(package)?;
    if !quiet {
        eprintln!("Found package at: {}", pkg_path.display());
    }

    // Discover .py files (respecting test/private filters)
    let py_files = discover_py_files(&pkg_path, opts.include_tests, opts.include_private);
    if !quiet {
        eprintln!("Found {} Python files", py_files.len());
    }

    // Collect __all__ and __init__.py imports if filtering is requested
    let allowed_names = if opts.exports_only {
        Some(collect_public_names(
            &pkg_path,
            package,
            opts.include_imports,
        ))
    } else {
        None
    };

    if !quiet {
        if let Some(ref names) = allowed_names {
            eprintln!("Public API: {} exported names", names.len());
        }
    }

    // Parse all files and extract objects
    let mut all_objects: Vec<PyObject> = Vec::new();
    for path in &py_files {
        let module_path = file_to_module_path(&pkg_path, path, package);
        let source = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let objects = extract::extract_objects(&source, &module_path);
        all_objects.extend(objects);
    }

    // Filter to allowed names if --all is set
    if let Some(ref names) = allowed_names {
        all_objects.retain(|obj| names.contains(&obj.name));
    }

    // Deduplicate: prefer shortest path for each short name
    let mut seen: HashMap<String, PyObject> = HashMap::new();
    for obj in all_objects {
        let short = obj.path.rsplit('.').next().unwrap_or(&obj.name).to_string();
        if let Some(existing) = seen.get(&short) {
            if obj.path.len() < existing.path.len() {
                seen.insert(short, obj);
            }
        } else {
            seen.insert(short, obj);
        }
    }

    let mut public: Vec<PyObject> = seen.into_values().collect();
    public.sort_by(|a, b| a.path.cmp(&b.path));

    if !quiet {
        eprintln!(
            "Extracting {} documented objects from '{}'",
            public.len(),
            package
        );
    }

    fs::create_dir_all(output)?;
    let mut written = 0;

    for obj in &public {
        let docstring = match &obj.docstring {
            Some(d) => d,
            None => continue,
        };

        let qmd = if doc_style == DocstringStyle::Markdown {
            render::obj_to_qmd_markdown(obj, docstring)
        } else {
            let sections = parse_docstring(docstring, doc_style);
            match render::obj_to_qmd(obj, &sections) {
                Some(q) => q,
                None => continue,
            }
        };

        // Build nested path: pkg.sub.func -> sub/func.qmd
        let rel_path = obj
            .path
            .strip_prefix(package)
            .unwrap_or(&obj.path)
            .trim_start_matches('.');
        let outpath = if rel_path.contains('.') {
            let parts: Vec<&str> = rel_path.split('.').collect();
            let mut p = output.to_path_buf();
            for dir in &parts[..parts.len() - 1] {
                p.push(render::safe_name(dir));
            }
            fs::create_dir_all(&p)?;
            p.push(format!("{}.qmd", render::safe_name(parts[parts.len() - 1])));
            p
        } else {
            output.join(format!("{}.qmd", render::safe_name(rel_path)))
        };
        fs::write(&outpath, &qmd)?;
        written += 1;
    }

    if !quiet {
        eprintln!("Wrote {} .qmd files to '{}'", written, output_str);
    }

    Ok(())
}

/// Collect names that constitute the public API of a package.
///
/// Walks `__init__.py` files to find `__all__` declarations, and optionally
/// includes names imported at the package level.
fn collect_public_names(
    pkg_path: &Path,
    package: &str,
    include_imports: bool,
) -> HashSet<String> {
    let mut names = HashSet::new();

    // Read the top-level __init__.py
    let init_path = pkg_path.join("__init__.py");
    if let Ok(source) = fs::read_to_string(&init_path) {
        let all_names = extract::extract_all_names(&source);
        names.extend(all_names);

        if include_imports {
            let import_names = extract::extract_imports(&source);
            names.extend(import_names);
        }
    }

    // Also check for __all__ in subpackage __init__.py files, but only
    // if the parent package re-exports them. For simplicity, we walk all
    // __init__.py and collect their __all__ entries too.
    walk_init_files(pkg_path, package, include_imports, &mut names);

    names
}

/// Recursively walk __init__.py files and collect __all__ names.
fn walk_init_files(
    dir: &Path,
    _package: &str,
    include_imports: bool,
    names: &mut HashSet<String>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let dirname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if dirname.starts_with('.') || dirname.starts_with("__") {
                continue;
            }
            if is_test_dir(dirname) {
                continue;
            }
            let sub_init = path.join("__init__.py");
            if let Ok(source) = fs::read_to_string(&sub_init) {
                names.extend(extract::extract_all_names(&source));
                if include_imports {
                    names.extend(extract::extract_imports(&source));
                }
            }
            walk_init_files(&path, _package, include_imports, names);
        }
    }
}

/// Parse a docstring using the specified style (or auto-detect).
fn parse_docstring(
    docstring: &str,
    style: DocstringStyle,
) -> Vec<types::DocSection> {
    match style {
        DocstringStyle::Google => google::parse_google(docstring),
        DocstringStyle::Numpy => numpy::parse_numpy(docstring),
        DocstringStyle::Sphinx => sphinx::parse_sphinx(docstring),
        DocstringStyle::Markdown => unreachable!("markdown style handled in caller"),
        DocstringStyle::Auto => {
            if sphinx::is_sphinx_style(docstring) {
                sphinx::parse_sphinx(docstring)
            } else if numpy::is_numpy_style(docstring) {
                numpy::parse_numpy(docstring)
            } else if google::is_google_style(docstring) {
                google::parse_google(docstring)
            } else {
                let text = docstring.trim().to_string();
                if text.is_empty() {
                    Vec::new()
                } else {
                    vec![types::DocSection {
                        kind: types::SectionKind::Text,
                        content: types::SectionContent::Text(text),
                    }]
                }
            }
        }
    }
}

/// Find the filesystem path of an installed Python package.
fn find_package_path(package: &str) -> Result<PathBuf> {
    let script = format!(
        r#"
import importlib, importlib.util, sys, pathlib
try:
    spec = importlib.util.find_spec("{pkg}")
except (ModuleNotFoundError, ValueError):
    spec = None
if spec is None:
    print("NOT_FOUND", file=sys.stderr)
    sys.exit(1)
if spec.submodule_search_locations:
    print(spec.submodule_search_locations[0])
elif spec.origin:
    print(str(pathlib.Path(spec.origin).parent))
else:
    print("NOT_FOUND", file=sys.stderr)
    sys.exit(1)
"#,
        pkg = package
    );

    let result = Command::new("python3")
        .args(["-c", &script])
        .output()
        .map_err(|_| {
            anyhow::anyhow!(
                "{}",
                crate::utils::tools::not_found_message(&crate::utils::tools::PYTHON)
            )
        })?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        anyhow::bail!(
            "Could not find Python package '{}': {}",
            package,
            stderr.trim()
        );
    }

    let path_str = String::from_utf8(result.stdout)
        .context("python3 output is not valid UTF-8")?;
    let path = PathBuf::from(path_str.trim());

    if !path.exists() {
        anyhow::bail!("Package path does not exist: {}", path.display());
    }

    Ok(path)
}

/// Discover all `.py` files in a package directory (recursively).
fn discover_py_files(root: &Path, include_tests: bool, include_private: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    walk_dir(root, include_tests, include_private, &mut files);
    files.sort();
    files
}

fn walk_dir(dir: &Path, include_tests: bool, include_private: bool, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // Always skip __pycache__, hidden dirs
            if name.starts_with('.') || name == "__pycache__" {
                continue;
            }
            // Skip test directories unless requested
            if !include_tests && is_test_dir(name) {
                continue;
            }
            // Skip _-prefixed directories (internal modules) unless requested
            if !include_private && name.starts_with('_') && name != "__init__" {
                continue;
            }
            walk_dir(&path, include_tests, include_private, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("py") {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // Skip test files unless requested
            if !include_tests && is_test_file(name) {
                continue;
            }
            // Include __init__.py (for exports) and all non-_-prefixed files
            if !name.starts_with('_') || name == "__init__.py" {
                out.push(path);
            }
        }
    }
}

/// Check if a directory name indicates a test directory.
fn is_test_dir(name: &str) -> bool {
    name == "tests" || name == "test" || name == "testing" || name.starts_with("test_")
}

/// Check if a file name indicates a test file.
fn is_test_file(name: &str) -> bool {
    name.starts_with("test_") || name == "conftest.py"
}

/// Convert a file path to a Python module path.
fn file_to_module_path(root: &Path, file: &Path, package: &str) -> String {
    let rel = file.strip_prefix(root).unwrap_or(file);
    let stem = rel
        .with_extension("")
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, ".");

    let stem = stem.strip_suffix(".__init__").unwrap_or(&stem);

    if stem.is_empty() || stem == "__init__" {
        package.to_string()
    } else {
        format!("{}.{}", package, stem)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_to_module_path() {
        let root = Path::new("/site-packages/pkg");
        let file = Path::new("/site-packages/pkg/sub/mod.py");
        assert_eq!(file_to_module_path(root, file, "pkg"), "pkg.sub.mod");

        let init = Path::new("/site-packages/pkg/__init__.py");
        assert_eq!(file_to_module_path(root, init, "pkg"), "pkg");

        let sub_init = Path::new("/site-packages/pkg/sub/__init__.py");
        assert_eq!(file_to_module_path(root, sub_init, "pkg"), "pkg.sub");
    }

    #[test]
    fn test_parse_docstring_auto() {
        let google = "Summary.\n\nArgs:\n    x (int): Value.\n";
        let sections = parse_docstring(google, DocstringStyle::Auto);
        assert!(sections.len() >= 2);

        let numpy = "Summary.\n\nParameters\n----------\nx : int\n    Value.\n";
        let sections = parse_docstring(numpy, DocstringStyle::Auto);
        assert!(sections.len() >= 2);
    }

    #[test]
    fn test_is_test_dir() {
        assert!(is_test_dir("tests"));
        assert!(is_test_dir("test"));
        assert!(is_test_dir("testing"));
        assert!(is_test_dir("test_utils"));
        assert!(!is_test_dir("utils"));
        assert!(!is_test_dir("attestation"));
    }
}
