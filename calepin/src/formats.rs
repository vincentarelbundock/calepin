//! Output format pipeline.
//!
//! A format is defined by a Target configuration that declares an engine
//! (AST emitter), a list of body transform modules, a cross-reference
//! strategy, and an output writer. The `FormatPipeline` reads this config
//! and dispatches to the appropriate modules.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

use crate::types::Element;
use crate::metadata::Metadata;
use crate::render::elements::ElementRenderer;
use crate::project::Target;

// ---------------------------------------------------------------------------
// FormatPipeline
// ---------------------------------------------------------------------------

/// How to write the final output.
pub enum WriterKind {
    /// Write the string directly to the output file.
    File,
    /// Convert via pandoc (e.g., markdown -> docx).
    Pandoc,
}

/// Pipeline executor driven by Target configuration.
///
/// Each pipeline stage dispatches to registered modules declared in the
/// Target's config: body transforms, crossref strategy, page template, writer.
pub struct FormatPipeline {
    pub engine: String,
    pub extension: String,
    pub fig_extension: String,
    pub page_template: Option<String>,
    pub crossref: String,
    pub writer: WriterKind,
    #[allow(dead_code)]
    pub embed_resources: bool,
    /// Module names, resolved via the registry at each pipeline stage.
    module_names: Vec<String>,
    /// The target name (e.g., "revealjs", "html"), used for template resolution.
    pub target_name: String,
    /// Whether to pass headings for TOC generation during page assembly.
    pub toc_headings: bool,
    /// Extra template variables injected during page assembly.
    pub page_vars: HashMap<String, String>,
}

impl FormatPipeline {
    /// Build a pipeline from a resolved Target.
    pub fn from_target(target: &Target, target_name: &str) -> Result<Self> {
        let engine = target.engine.clone();

        // Crossref defaults to engine-appropriate strategy
        let crossref = target.crossref.clone().unwrap_or_else(|| {
            match engine.as_str() {
                "html" => "html",
                "latex" => "latex",
                _ => "plain",
            }.to_string()
        });

        let writer = match target.writer.as_deref() {
            Some("pandoc") => WriterKind::Pandoc,
            _ => WriterKind::File,
        };

        // toc_headings defaults to true for html, false for latex
        let toc_headings = target.toc_headings.unwrap_or_else(|| {
            engine != "latex"
        });

        Ok(FormatPipeline {
            engine,
            extension: target.output_extension().to_string(),
            fig_extension: target.fig_ext().to_string(),
            page_template: target.template.clone(),
            crossref,
            writer,
            embed_resources: target.embed_resources.unwrap_or(true),
            module_names: target.modules.clone(),
            target_name: target_name.to_string(),
            toc_headings,
            page_vars: target.page_vars.clone(),
        })
    }

    /// Build a pipeline from just an engine name, using built-in target defaults.
    pub fn from_engine(engine: &str) -> Result<Self> {
        let target = crate::project::resolve_target(engine, &std::collections::HashMap::new())?;
        Self::from_target(&target, engine)
    }

    pub fn engine(&self) -> &str { &self.engine }
    pub fn extension(&self) -> &str { &self.extension }
    pub fn default_fig_ext(&self) -> &str { &self.fig_extension }

    /// Whether a given body transform is active in this pipeline.
    #[allow(dead_code)]
    pub fn has_transform(&self, name: &str) -> bool {
        self.module_names.iter().any(|n| n == name)
    }

    /// Render a list of elements into the final document body.
    pub fn render(&self, elements: &[Element], renderer: &ElementRenderer) -> Result<String> {
        renderer.collect_footnote_defs(elements);

        let body = elements
            .iter()
            .map(|el| renderer.render(el))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");

        Ok(body)
    }

    /// Run all body transforms in declared order, resolving each by name
    /// from the plugin registry.
    pub fn transform_body(&self, body: &str, renderer: &ElementRenderer, target: &Target) -> String {
        let registry = renderer.registry();
        let mut result = body.to_string();
        for name in &self.module_names {
            if let Some(t) = registry.resolve_body_transform(name) {
                result = t.transform(&result, renderer, target);
            }
        }
        result
    }

    /// Resolve cross-references using the configured strategy.
    pub fn resolve_crossrefs(&self, body: &str, renderer: &ElementRenderer) -> String {
        let thm_nums = renderer.theorem_numbers();
        let walk_meta = renderer.walk_metadata();
        match self.crossref.as_str() {
            "html" => crate::crossref::resolve_html_with_ids(body, &thm_nums, &walk_meta.ids),
            "latex" => crate::crossref::resolve_latex(body, &thm_nums),
            _ => crate::crossref::resolve_plain(body, &thm_nums),
        }
    }

    /// Collect cross-reference data without resolving (for collection mode pass 1).
    pub fn collect_crossref_data(&self, body: &str, renderer: &ElementRenderer) -> Option<crate::crossref::PageRefData> {
        if self.crossref == "html" {
            let thm_nums = renderer.theorem_numbers();
            let walk_meta = renderer.walk_metadata();
            Some(crate::crossref::collect_ids_html(body, &thm_nums, &walk_meta.ids))
        } else {
            None
        }
    }

    /// Run pre-render element transforms from active modules.
    /// Calls `transform_all` on each raw element transform.
    pub fn transform_elements_pre(&self, elements: &mut Vec<Element>, renderer: &ElementRenderer) {
        let registry = renderer.registry();
        for t in registry.resolve_element_raw_transforms(&self.module_names) {
            t.transform_all(elements);
        }
    }

    /// Run page transforms from active modules (during page assembly).
    pub fn transform_page(&self, vars: &mut HashMap<String, String>, renderer: &ElementRenderer, meta: &Metadata) {
        let registry = renderer.registry();
        for t in registry.resolve_page_transforms(&self.module_names) {
            t.transform(vars, renderer, meta);
        }
    }

    /// Run document transforms from active modules (post-assembly).
    pub fn transform_document(&self, document: &str, renderer: &ElementRenderer) -> String {
        let registry = renderer.registry();
        let mut result = document.to_string();
        for t in registry.resolve_document_transforms(&self.module_names) {
            result = t.transform(&result);
        }
        result
    }

    /// Wrap the rendered body in a page template.
    pub fn assemble_page(
        &self,
        body: &str,
        meta: &Metadata,
        renderer: &ElementRenderer,
    ) -> Option<String> {
        self.page_template.as_deref()?;

        let walk_meta = renderer.walk_metadata();
        let headings = if self.toc_headings { &walk_meta.headings[..] } else { &[][..] };

        // Collect page transform vars from modules
        let mut extra_vars = HashMap::new();
        self.transform_page(&mut extra_vars, renderer, meta);

        let page_vars = &self.page_vars;

        let html = crate::render::template::assemble_page(
            body, meta, &self.target_name, headings, renderer.preamble(),
            renderer.target.as_ref(),
            |vars| {
                // Apply module-provided page vars
                for (k, v) in &extra_vars {
                    vars.insert(k.clone(), v.clone());
                }

                // Apply page_vars from target config (overrides everything)
                for (k, v) in page_vars {
                    vars.insert(k.clone(), v.clone());
                }
            },
        );

        Some(html)
    }

    /// Resolve citations in the element list via hayagriva.
    pub fn resolve_bibliography(
        &self,
        elements: &mut Vec<Element>,
        meta: &Metadata,
        project_root: &Path,
    ) -> Result<()> {
        crate::bibliography::process_citations(elements, meta, project_root)
    }

    /// Write the final rendered content to the output file.
    pub fn write_output(&self, content: &str, output_path: &Path) -> Result<()> {
        match self.writer {
            WriterKind::File => {
                std::fs::write(output_path, content)
                    .with_context(|| format!("Failed to write output file: {}", output_path.display()))?;
                Ok(())
            }
            WriterKind::Pandoc => {
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
                    .map_err(|e| crate::tools::check_spawn_error(e, &crate::tools::PANDOC))?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!("pandoc failed: {}", stderr);
                }

                Ok(())
            }
        }
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
