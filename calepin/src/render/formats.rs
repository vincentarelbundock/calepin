//! Output format pipeline.
//!
//! A format is defined by a Target configuration that declares a writer
//! (AST emitter), a list of body transform modules, and a cross-reference
//! strategy. The `FormatPipeline` reads this config and dispatches to the
//! appropriate modules.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

use crate::types::Element;
use crate::config::Metadata;
use super::elements::ElementRenderer;
use crate::config::Target;

// ---------------------------------------------------------------------------
// FormatPipeline
// ---------------------------------------------------------------------------

/// Pipeline executor driven by Target configuration.
///
/// Each pipeline stage dispatches to registered modules declared in the
/// Target's config: body transforms, crossref strategy, page template.
pub struct FormatPipeline {
    pub writer: String,
    pub extension: String,
    pub fig_extension: String,
    pub page_template: Option<String>,
    pub crossref: String,
    /// Module names, resolved via the registry at each pipeline stage.
    module_names: Vec<String>,
    /// Whether to pass headings for TOC generation during page assembly.
    pub toc_headings: bool,
    /// Extra template variables injected during page assembly.
    pub page_vars: HashMap<String, String>,
}

impl FormatPipeline {
    /// Build a pipeline from a resolved Target.
    pub fn from_target(target: &Target) -> Result<Self> {
        let writer = target.writer.clone();

        // Crossref defaults to writer-appropriate strategy
        let crossref = target.crossref.clone().unwrap_or_else(|| {
            match writer.as_str() {
                "html" => "html",
                "latex" => "latex",
                _ => "plain",
            }.to_string()
        });

        // toc_headings defaults to true for html, false for latex
        let toc_headings = target.toc_headings.unwrap_or_else(|| {
            writer != "latex"
        });

        Ok(FormatPipeline {
            writer,
            extension: target.output_extension().to_string(),
            fig_extension: target.fig_ext().to_string(),
            page_template: target.template.clone(),
            crossref,
            module_names: target.modules.clone(),
            toc_headings,
            page_vars: target.page_vars.clone(),
        })
    }

    /// Build a pipeline from just a writer name, using built-in target defaults.
    pub fn from_writer(writer: &str) -> Result<Self> {
        let target = crate::config::resolve_target(writer, &std::collections::HashMap::new())?;
        Self::from_target(&target)
    }

    pub fn writer(&self) -> &str { &self.writer }
    pub fn extension(&self) -> &str { &self.extension }
    pub fn default_fig_ext(&self) -> &str { &self.fig_extension }

    /// Render a list of elements into the final document body.
    pub fn render(&self, elements: &[Element], renderer: &ElementRenderer) -> Result<String> {
        renderer.footnotes.collect_defs(elements);

        let body = elements
            .iter()
            .map(|el| renderer.render(el))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");

        Ok(body)
    }

    /// Resolve cross-references using the configured strategy.
    pub fn resolve_crossrefs(&self, body: &str, renderer: &ElementRenderer) -> String {
        let module_ids = renderer.module_ids();
        let walk_meta = renderer.walk_metadata();
        match self.crossref.as_str() {
            "html" => crate::crossref::resolve_html_with_ids(body, &module_ids, &walk_meta.ids),
            "latex" => crate::crossref::resolve_latex(body, &module_ids),
            _ => crate::crossref::resolve_plain(body, &module_ids),
        }
    }

    /// Collect cross-reference data without resolving (for collection mode pass 1).
    pub fn collect_crossref_data(&self, body: &str, renderer: &ElementRenderer) -> Option<crate::crossref::PageRefData> {
        if self.crossref == "html" {
            let module_ids = renderer.module_ids();
            let walk_meta = renderer.walk_metadata();
            Some(crate::crossref::collect_ids_html(body, &module_ids, &walk_meta.ids))
        } else {
            None
        }
    }

    /// Run pre-render element transforms from active modules.
    /// Each transform is called on every element (including nested children).
    pub fn transform_elements(&self, elements: &mut Vec<Element>, renderer: &ElementRenderer) {
        let registry = renderer.registry();
        for t in registry.resolve_transform_element(&self.module_names) {
            apply_transform_recursive(elements, t);
        }
    }

    /// Run document transforms from active modules (post-assembly).
    pub fn transform_document(&self, document: &str, renderer: &ElementRenderer) -> String {
        let registry = renderer.registry();
        let writer = &self.writer;
        let mut result = document.to_string();
        for t in registry.resolve_document_transforms(&self.module_names) {
            result = t.transform(&result, writer, renderer);
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

        let page_vars = &self.page_vars;

        let html = super::template::assemble_page(
            body, meta, &self.writer, headings, renderer.preamble(),
            renderer.target.as_ref(),
            |vars| {
                // Apply page_vars from target config
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
        std::fs::write(output_path, content)
            .with_context(|| format!("Failed to write output file: {}", output_path.display()))?;
        Ok(())
    }
}

/// Walk the element tree and apply a transform to each element.
fn apply_transform_recursive(elements: &mut Vec<Element>, t: &dyn crate::registry::TransformElement) {
    for element in elements.iter_mut() {
        t.transform(element);
        if let Element::Div { children, .. } = element {
            apply_transform_recursive(children, t);
        }
    }
}

/// Map a file extension to a canonical format name.
pub fn resolve_format(ext: &str) -> &str {
    match ext {
        "tex" => "latex",
        "pdf" => "latex",
        "typ" => "typst",
        "md" => "markdown",
        "html" => "html",
        other => other,
    }
}
