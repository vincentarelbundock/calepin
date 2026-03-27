//! `calepin man` -- extract package documentation as .qmd files.
//!
//! Subcommands:
//!   - `calepin man r <package>` -- R package docs via Rd AST
//!   - `calepin man python <package>` -- Python package docs via inspect

mod rdoc;

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

// ---------------------------------------------------------------------------
// R
// ---------------------------------------------------------------------------

/// R script embedded at compile time (serializes Rd AST to JSON).
const R_EXTRACT_DOCS: &str = include_str!("../extract_rdocs.R");

pub fn handle_man_r(package: &str, output: &Path, quiet: bool) -> Result<()> {
    let output_str = output.display().to_string();

    let tmp_dir = tempfile::tempdir()
        .context("Failed to create temporary directory")?;
    let script_path = tmp_dir.path().join("extract_rdocs.R");
    fs::write(&script_path, R_EXTRACT_DOCS)
        .context("Failed to write temporary R script")?;

    if !quiet {
        eprintln!("Extracting R docs for '{}' -> {}", package, output_str);
    }

    let result = Command::new("Rscript")
        .args([
            script_path.to_str().unwrap(),
            package,
            &output_str,
        ])
        .output()
        .map_err(|_| anyhow::anyhow!("{}", crate::utils::tools::not_found_message(&crate::utils::tools::RSCRIPT)))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        anyhow::bail!("Rscript failed:\n{}", stderr.trim());
    }

    let json = String::from_utf8(result.stdout)
        .context("Rscript output is not valid UTF-8")?;

    let topics: Vec<rdoc::RdTopic> = serde_json::from_str(&json)
        .context("Failed to parse Rd JSON from Rscript")?;

    if !quiet {
        eprintln!("Converting {} help topics to .qmd", topics.len());
    }

    fs::create_dir_all(output)?;
    let mut written = 0;
    for topic in &topics {
        let qmd = rdoc::rd_to_qmd(topic);
        let safe_name = topic.topic.replace(|c: char| !c.is_alphanumeric() && c != '.' && c != '_' && c != '-', "_");
        let outpath = output.join(format!("{}.qmd", safe_name));
        fs::write(&outpath, &qmd)?;
        written += 1;
    }

    if !quiet {
        eprintln!("Wrote {} .qmd files to '{}'", written, output_str);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

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
