//! `calepin man python` -- extract Python package documentation as .qmd files.

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

/// Python script embedded at compile time (serializes docstrings to JSON).
const PY_EXTRACT_DOCS: &str = include_str!("../extract_pydocs.py");

pub fn handle_man_python(package: &str, output: &Path, quiet: bool) -> Result<()> {
    let output_str = output.display().to_string();

    let tmp_dir = tempfile::tempdir()
        .context("Failed to create temporary directory")?;
    let script_path = tmp_dir.path().join("extract_pydocs.py");
    fs::write(&script_path, PY_EXTRACT_DOCS)
        .context("Failed to write temporary Python script")?;

    if !quiet {
        eprintln!("Extracting Python docs for '{}' -> {}", package, output_str);
    }

    let result = Command::new("python3")
        .args([
            script_path.to_str().unwrap(),
            package,
            &output_str,
        ])
        .output()
        .map_err(|_| anyhow::anyhow!("{}", crate::utils::tools::not_found_message(&crate::utils::tools::PYTHON)))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        anyhow::bail!("python3 failed:\n{}", stderr.trim());
    }

    if !quiet {
        let stdout = String::from_utf8_lossy(&result.stdout);
        eprint!("{}", stdout);
    }

    Ok(())
}
