use std::path::Path;

use anyhow::{Context, Result};

use crate::formats::OutputRenderer;
use crate::types::Metadata;
use crate::render::elements::ElementRenderer;

pub struct WordRenderer;

impl OutputRenderer for WordRenderer {
    fn format(&self) -> &str { "word" }
    fn extension(&self) -> &str { "docx" }
    fn base_format(&self) -> &str { "markdown" }

    fn apply_template(
        &self,
        _body: &str,
        _meta: &Metadata,
        _renderer: &ElementRenderer,
    ) -> Option<String> {
        None
    }

    fn write_output(&self, content: &str, output_path: &Path) -> Result<()> {
        let tmp_dir = tempfile::tempdir()
            .context("Failed to create temporary directory")?;
        let md_path = tmp_dir.path().join("input.md");
        std::fs::write(&md_path, content)
            .context("Failed to write temporary markdown file")?;

        let output = std::process::Command::new("pandoc")
            .args([
                &md_path.to_string_lossy() as &str,
                "-o",
                &output_path.to_string_lossy() as &str,
            ])
            .output()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => anyhow::anyhow!(
                    "pandoc not found. The Word format requires pandoc.\n\
                     Install it from https://pandoc.org/installing.html"
                ),
                _ => anyhow::anyhow!("Failed to run pandoc: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("pandoc failed: {}", stderr);
        }

        Ok(())
    }
}
