//! R package documentation extractor.
//!
//! Runs `extract_rdocs.R` to serialize the Rd AST to JSON, then converts
//! each topic to a `.qmd` file via `rdoc::RdRenderer`.

pub mod rdoc;

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

/// R script embedded at compile time (serializes Rd AST to JSON).
const R_EXTRACT_DOCS: &str = include_str!("extract_rdocs.R");

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

    // Discover pkgdown URLs for linked packages (pure Rust, no R dependency).
    let linked = rdoc::collect_linked_packages(&topics, package);
    let urls = if linked.is_empty() {
        std::collections::HashMap::new()
    } else {
        if !quiet {
            eprintln!("Resolving pkgdown URLs for: {}", linked.iter().cloned().collect::<Vec<_>>().join(", "));
        }
        rdoc::discover_pkgdown_urls(&linked)
    };

    if !quiet && !urls.is_empty() {
        eprintln!("Found pkgdown sites for: {}", urls.keys().cloned().collect::<Vec<_>>().join(", "));
    }

    let renderer = rdoc::RdRenderer {
        package,
        urls: &urls,
    };

    fs::create_dir_all(output)?;
    let mut written = 0;
    for topic in &topics {
        let qmd = renderer.render_topic(topic);
        let safe_name = crate::man::safe_name(&topic.topic);
        let outpath = output.join(format!("{}.qmd", safe_name));
        fs::write(&outpath, &qmd)?;
        written += 1;
    }

    if !quiet {
        eprintln!("Wrote {} .qmd files to '{}'", written, output_str);
    }

    Ok(())
}
