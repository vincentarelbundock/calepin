//! Output format backends.
//!
//! Built-in formats: html, latex, typst, markdown, revealjs, word.

pub mod html;
pub mod latex;
pub mod markdown;
pub mod revealjs;
pub mod typst;
pub mod word;

use std::path::Path;

use anyhow::{Context, Result};

use crate::types::{Element, Metadata};
use crate::render::elements::ElementRenderer;

/// Trait for pluggable output formats.
pub trait OutputRenderer {
    /// Canonical format name (e.g., "html", "latex", "typst", "markdown").
    fn format(&self) -> &str;

    /// File extension for output files (e.g., "html", "tex", "typ", "md").
    fn extension(&self) -> &str;

    /// Base format name for element template lookup.
    /// For built-in formats, same as `format()`. For custom formats,
    /// returns the base format (e.g., "html" for a "blog" format).
    fn engine(&self) -> &str {
        self.format()
    }

    /// Default figure file extension, derived from the built-in config.
    fn default_fig_ext(&self) -> &str {
        crate::project::builtin_config()
            .targets.get(self.engine())
            .and_then(|t| t.fig_extension.as_deref())
            .unwrap_or("png")
    }

    /// Render a list of elements into the final document body.
    fn render(&self, elements: &[Element], renderer: &ElementRenderer) -> Result<String> {
        // Pre-collect footnote definitions across all Text elements so that
        // references in one block can resolve against definitions in another.
        renderer.collect_footnote_defs(elements);

        let body = elements
            .iter()
            .map(|el| renderer.render(el))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");

        Ok(body)
    }

    /// Format-specific transformation of the rendered body string.
    /// Called after render(), before cross-ref resolution.
    fn transform_body(&self, body: &str, _renderer: &ElementRenderer) -> String {
        body.to_string()
    }

    /// Resolve cross-references in the rendered body.
    /// Default dispatches to the appropriate crossref function based on engine.
    fn resolve_crossrefs(&self, body: &str, renderer: &ElementRenderer) -> String {
        let thm_nums = renderer.theorem_numbers();
        let walk_meta = renderer.walk_metadata();
        match self.engine() {
            "html" => crate::crossref::resolve_html_with_ids(body, &thm_nums, &walk_meta.ids),
            "latex" => crate::crossref::resolve_latex(body, &thm_nums),
            _ => crate::crossref::resolve_plain(body, &thm_nums),
        }
    }

    /// Collect cross-reference data without resolving (for collection mode pass 1).
    /// Returns None for non-HTML formats.
    fn collect_crossref_data(&self, body: &str, renderer: &ElementRenderer) -> Option<crate::crossref::PageRefData> {
        if self.engine() == "html" {
            let thm_nums = renderer.theorem_numbers();
            let walk_meta = renderer.walk_metadata();
            Some(crate::crossref::collect_ids_html(body, &thm_nums, &walk_meta.ids))
        } else {
            None
        }
    }

    /// Wrap the rendered body in a page template. Return None to skip.
    fn assemble_page(&self, body: &str, meta: &Metadata, renderer: &ElementRenderer)
        -> Option<String>;

    /// Format-specific transformation of the complete document (after page template).
    fn transform_document(&self, document: &str, _renderer: &ElementRenderer) -> String {
        document.to_string()
    }

    /// Resolve citations in the element list via hayagriva.
    fn resolve_bibliography(
        &self,
        elements: &mut Vec<Element>,
        meta: &Metadata,
        project_root: &Path,
    ) -> Result<()> {
        crate::bibliography::process_citations(elements, meta, project_root)
    }

    /// Write the final rendered content to the output file.
    /// The default implementation writes the string directly. Formats that
    /// need external conversion (e.g., Word via pandoc) override this.
    fn write_output(&self, content: &str, output_path: &Path) -> Result<()> {
        std::fs::write(output_path, content)
            .with_context(|| format!("Failed to write output file: {}", output_path.display()))?;
        Ok(())
    }
}

/// Create a renderer from a format name string.
/// Checks built-in formats first, then custom format configs.
pub fn create_renderer(format: &str) -> Result<Box<dyn OutputRenderer>> {
    match format {
        "html" => Ok(Box::new(html::HtmlRenderer)),
        "latex" => Ok(Box::new(latex::LatexRenderer)),
        "markdown" => Ok(Box::new(markdown::MarkdownRenderer)),
        "typst" => Ok(Box::new(typst::TypstRenderer)),
        "word" => Ok(Box::new(word::WordRenderer)),
        "revealjs" => Ok(Box::new(revealjs::RevealJsRenderer)),
        other => anyhow::bail!("Unknown format: '{}'", other),
    }
}

/// Map a file extension to a canonical format name.
pub fn resolve_format_from_extension(ext: &str) -> &str {
    match ext {
        "tex" => "latex",
        "pdf" => "latex",
        "typ" => "typst",
        "md" => "markdown",
        "docx" => "word",
        "html" => "html",
        other => other,
    }
}

