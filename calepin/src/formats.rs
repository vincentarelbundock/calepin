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
use crate::modules::highlight::ColorScope;
use crate::project::Target;
use crate::modules;

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
    pub embed_resources: bool,
    /// Body transform names, resolved via the plugin registry at call time.
    transform_names: Vec<String>,
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
            transform_names: target.body_transforms.clone(),
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
    pub fn has_transform(&self, name: &str) -> bool {
        self.transform_names.iter().any(|n| n == name)
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
        for name in &self.transform_names {
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
        let has_syntax_css = self.has_transform("inject_syntax_css_html");
        let engine = &self.engine;

        let html = crate::render::template::assemble_page(
            body, meta, &self.target_name, headings, renderer.preamble(),
            renderer.target.as_ref(),
            |vars| {
                // Inject syntax highlighting CSS when the transform is active
                if has_syntax_css {
                    let syntax_css = modules::inject_syntax_css_html::generate(
                        renderer, ColorScope::Both,
                    );
                    if !syntax_css.is_empty() {
                        let css = vars.entry("css".to_string()).or_default();
                        css.push('\n');
                        css.push_str(&syntax_css);
                        // Also set as a standalone var for templates that use it separately
                        vars.insert("syntax_css".to_string(),
                            modules::inject_syntax_css_html::generate(renderer, ColorScope::Both));
                    }
                }

                // Render math include for html-engine targets
                if engine == "html" {
                    let defs = &renderer.metadata;
                    let mut math_vars = HashMap::new();
                    math_vars.insert("html_math_method".to_string(),
                        meta.html_math_method.as_deref()
                            .unwrap_or_else(|| defs.math.as_deref().unwrap_or("katex")).to_string());
                    let math_html = crate::render::template::render_element("math", "html", &math_vars);
                    if !math_html.is_empty() {
                        vars.insert("math".to_string(), math_html);
                    }
                }

                // Apply page_vars from target config (overrides computed values)
                for (k, v) in page_vars {
                    vars.insert(k.clone(), v.clone());
                }
            },
        );

        // Embed images as base64 if configured
        if self.embed_resources && self.engine == "html" {
            Some(modules::embed_images_html::embed_images_base64(&html))
        } else {
            Some(html)
        }
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
