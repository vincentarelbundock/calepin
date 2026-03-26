//! Core render pipeline: parse, evaluate, render.
//!
//! This module contains the main rendering pipeline that transforms a .qmd file
//! into output (HTML, LaTeX, Typst, Markdown). It orchestrates parsing, code
//! execution, bibliography processing, and format conversion.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::engines;
use crate::formats;
use crate::formats::FormatPipeline;
use crate::jinja;
use crate::parse;
use crate::paths;
use crate::project;
use crate::registry;
use crate::render;
use crate::render::elements::ElementRenderer;

/// Result of the core render pipeline (before page template wrapping).
pub struct RenderResult {
    pub rendered: String,
    pub metadata: crate::config::Metadata,
    pub element_renderer: ElementRenderer,
}

/// Options for the core render pipeline.
#[derive(Default)]
pub struct RenderCoreOptions {
    /// Chapter number for this page in a collection. When set, section numbering
    /// uses this as the top-level counter (e.g., chapter 2 -> sections 2.1, 2.2).
    pub chapter_number: Option<usize>,
    /// When true, skip cross-reference resolution (collection mode collects ref_data separately).
    pub skip_crossref: bool,
}

/// Core render pipeline: parse, evaluate, render. Does NOT apply the page template.
/// If `format` is None, falls back to the format declared in YAML front matter, then "html".
pub fn render_core(
    input: &Path,
    output_path: &Path,
    format: Option<&str>,
    overrides: &[String],
    project_root_override: Option<&Path>,
    options: &RenderCoreOptions,
    project_metadata: Option<&crate::config::Metadata>,
    target: Option<&project::Target>,
) -> Result<RenderResult> {

    // 1. Read input file
    let input_text = fs::read_to_string(input)
        .with_context(|| format!("Failed to read input file: {}", input.display()))?;

    // 2. Parse YAML front matter, then apply CLI overrides
    let (mut metadata, body) = crate::config::split_frontmatter(&input_text)?;
    let body = render::markers::sanitize(&body);
    metadata.apply_overrides(overrides);
    metadata.resolve_date(Some(input));

    // Merge project metadata as base layer (front matter wins)
    if let Some(project_meta) = project_metadata {
        metadata = project_meta.clone().merge(metadata);
    }

    // 2b. Construct path context and validate paths
    let path_ctx = paths::PathContext::new(input, output_path, project_root_override);
    let input_name = input.file_name().unwrap_or_default().to_string_lossy();
    paths::validate_paths(&metadata, &path_ctx, &input_name)?;
    path_ctx.print_root_diagnostic(input);

    // 3. Build format pipeline from target or engine name
    let format_str = format
        .map(|s| s.to_string())
        .or_else(|| metadata.target.clone())
        .unwrap_or_else(|| "html".to_string());
    let pipeline = if let Some(t) = target {
        FormatPipeline::from_target(t, &format_str)?
    } else {
        FormatPipeline::from_engine(&format_str)?
    };

    // 4. Expand includes before block parsing (so included code chunks are parsed)
    let body = jinja::expand_includes(&body, &path_ctx.project_root, &format_str);

    // 4a. Parse body into blocks
    let blocks = parse::blocks::parse_body(&body)?;

    // 5. Initialize code engines (R, Python, sh) -- only starts what's needed
    let mut engines = engines::EnginePool::init(
        &blocks, &body, &metadata, pipeline.engine(),
        paths::PathContext::code_working_dir(input),
    )?;
    let mut ctx = engines.context();

    // 5b. Evaluate inline code in metadata fields (title, date, etc.)
    metadata.evaluate_inline(&mut ctx);

    // 6. Load plugin registry
    let registry = std::rc::Rc::new(
        registry::ModuleRegistry::load(&metadata.plugins, &path_ctx.project_root)
    );

    // 7. Create element renderer
    let mut element_renderer = ElementRenderer::from_metadata(pipeline.engine(), &metadata, options);
    element_renderer.set_target(target.cloned());

    // 8. Evaluate: execute code chunks and produce elements
    let eval_result = engines::evaluate_document(
        input, &blocks, &body, pipeline.engine(), &metadata, &registry,
        &mut ctx, &path_ctx, pipeline.default_fig_ext(),
    )?;
    let mut elements = eval_result.elements;

    // 9. Resolve bibliography
    pipeline.resolve_bibliography(&mut elements, &metadata, &path_ctx.project_root)?;

    // 10. Set registry on element renderer
    element_renderer.set_registry(registry);
    element_renderer.set_sc_fragments(eval_result.sc_fragments);
    element_renderer.set_preamble(eval_result.preamble);

    // 10b. Pre-render element transforms (SVG-to-PDF, etc.)
    pipeline.transform_elements_pre(&mut elements, &element_renderer);

    // 11. Render elements to body string
    let rendered = pipeline.render(&elements, &element_renderer)?;

    // 12. Transform body (module pipeline: footnotes, slides, color defs, etc.)
    let rendered = pipeline.transform_body(&rendered, &element_renderer,
        target.unwrap_or(&default_target_for_engine(pipeline.engine())));

    // 13. Cross-ref resolution (skipped in collection mode pass 1)
    let rendered = if options.skip_crossref {
        rendered
    } else {
        pipeline.resolve_crossrefs(&rendered, &element_renderer)
    };

    Ok(RenderResult { rendered, metadata, element_renderer })
}

/// Full render pipeline. Returns (output_path, rendered_content, pipeline).
pub fn render_file(
    input: &Path,
    output: Option<&Path>,
    format: Option<&str>,
    overrides: &[String],
    target: Option<&project::Target>,
    project_root: Option<&Path>,
    output_dir: Option<&str>,
    project_metadata: Option<&crate::config::Metadata>,
) -> Result<(PathBuf, String, FormatPipeline)> {
    // If we have a target, use its engine as the format
    let resolved_format = if let Some(t) = target {
        Some(t.engine.clone())
    } else {
        format
            .map(|s| s.to_string())
            .or_else(|| {
                output
                    .and_then(|p| p.extension())
                    .and_then(|e| e.to_str())
                    .map(|ext| formats::resolve_format_from_extension(ext).to_string())
            })
    };

    // Build pipeline
    let preliminary_format = resolved_format.as_deref().unwrap_or("html");
    let target_name = format.unwrap_or(preliminary_format);
    let pipeline = if let Some(t) = target {
        FormatPipeline::from_target(t, target_name)?
    } else {
        FormatPipeline::from_engine(preliminary_format)?
    };
    let ext = target.map(|t| t.output_extension()).unwrap_or(pipeline.extension());

    // Resolve output path
    let output_path = if let Some(o) = output {
        o.to_path_buf()
    } else if let (Some(_), Some(fmt)) = (target, format) {
        // Use target-aware output path when a target is specified
        project::resolve_target_output_path(input, fmt, ext, project_root, output_dir)
    } else {
        input.with_extension(ext)
    };

    let result = render_core(input, &output_path, resolved_format.as_deref(), overrides, None, &RenderCoreOptions::default(), project_metadata, target)?;

    // Assemble page (page template wrapping)
    let final_output = pipeline
        .assemble_page(&result.rendered, &result.metadata, &result.element_renderer)
        .unwrap_or(result.rendered);

    // Document transforms (post-assembly: image embedding, etc.)
    let final_output = pipeline.transform_document(&final_output, &result.element_renderer);

    Ok((output_path, final_output, pipeline))
}

/// Construct a minimal default Target for an engine name.
/// Used when render_core is called without an explicit target.
fn default_target_for_engine(engine: &str) -> project::Target {
    crate::project::resolve_target(engine, &std::collections::HashMap::new())
        .unwrap_or_else(|_| project::Target {
            inherits: None,
            engine: engine.to_string(),
            template: Some("page".to_string()),
            extension: None,
            fig_extension: None,
            preview: None,
            compile: None,
            embed_resources: None,
            vars: None,
            post: Vec::new(),
            modules: Vec::new(),
            crossref: None,
            writer: None,
            toc_headings: None,
            page_vars: std::collections::HashMap::new(),
            fig_formats: Vec::new(),
        })
}
