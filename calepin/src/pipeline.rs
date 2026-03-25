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
use crate::jinja;
use crate::parse;
use crate::paths;
use crate::project;
use crate::registry;
use crate::render;
use crate::render::elements::ElementRenderer;
use crate::types;
use crate::value;

/// Result of the core render pipeline (before page template wrapping).
pub struct RenderResult {
    pub rendered: String,
    pub metadata: types::Metadata,
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
    project_var: Option<&toml::Value>,
    project_root_override: Option<&Path>,
    options: &RenderCoreOptions,
) -> Result<RenderResult> {

    // 1. Read input file
    let input_text = fs::read_to_string(input)
        .with_context(|| format!("Failed to read input file: {}", input.display()))?;

    // 2. Parse YAML front matter, then apply CLI overrides
    let (mut metadata, body) = parse::yaml::split_yaml(&input_text)?;
    let body = render::markers::sanitize(&body);
    metadata.apply_overrides(overrides);
    metadata.resolve_date(Some(input));

    // Merge project-level var as defaults (front matter wins)
    if let Some(pv) = project_var {
        if let Some(table) = pv.as_table() {
            for (key, val) in table {
                if !metadata.var.contains_key(key) {
                    metadata.var.insert(key.clone(), value::from_toml(val.clone()));
                }
            }
        }
    }

    // 2b. Construct path context and validate paths
    let path_ctx = if let Some(root) = project_root_override {
        paths::PathContext {
            project_root: root.to_path_buf(),
            output_dir: output_path.parent().unwrap_or(Path::new(".")).to_path_buf(),
        }
    } else {
        paths::PathContext::for_document(input, output_path)
    };
    let input_name = input.file_name()
        .unwrap_or_default()
        .to_string_lossy();
    paths::validate_paths(&metadata, &path_ctx, &input_name)?;

    // 2c. Diagnostic: show effective project root (and code chunk cwd when different)
    if !crate::cli::is_quiet() {
        let input_dir = input.parent().unwrap_or(Path::new("."));
        let root = if path_ctx.project_root.as_os_str().is_empty() { Path::new(".") } else { &path_ctx.project_root };
        let idir = if input_dir.as_os_str().is_empty() { Path::new(".") } else { input_dir };
        if idir != root {
            eprintln!(
                "  root: {}  (code chunks run from {})",
                root.display(),
                idir.display()
            );
        } else {
            eprintln!("  root: {}", root.display());
        }
    }

    // 3. Create renderer for this format
    let format_str = format
        .map(|s| s.to_string())
        .or_else(|| metadata.target.clone())
        .unwrap_or_else(|| "html".to_string());
    let renderer = formats::create_renderer(&format_str)?;

    // 4. Expand includes before block parsing (so included code chunks are parsed)
    let body = jinja::expand_includes(&body, &path_ctx.project_root, &format_str);

    // 4a. Parse body into blocks
    let blocks = parse::blocks::parse_body(&body)?;

    // 5. Initialize code engines (R, Python, sh) -- only starts what's needed
    let input_dir = input.parent().and_then(|p| if p.as_os_str().is_empty() { None } else { Some(p) });
    let mut engines = engines::EnginePool::init(&blocks, &body, &metadata, renderer.engine(), input_dir)?;
    let mut ctx = engines.context();

    // 5b. Evaluate inline code in metadata fields (title, date, etc.)
    metadata.evaluate_inline(&mut ctx);

    // 6. Load plugin registry
    let registry = std::rc::Rc::new(
        registry::PluginRegistry::load(&metadata.plugins, &path_ctx.project_root)
    );

    // 7. Create element renderer
    let mut element_renderer = ElementRenderer::from_metadata(renderer.engine(), &metadata, options);

    // 8. Evaluate: execute code chunks and produce elements
    let eval_result = engines::evaluate_document(
        input, &blocks, &body, renderer.engine(), &metadata, &registry,
        &mut ctx, &path_ctx, renderer.default_fig_ext(),
    )?;
    let mut elements = eval_result.elements;

    // 9. Resolve bibliography
    renderer.resolve_bibliography(&mut elements, &metadata, &path_ctx.project_root)?;

    // 10. Set registry on element renderer
    element_renderer.set_registry(registry);
    element_renderer.set_sc_fragments(eval_result.sc_fragments);
    element_renderer.set_preamble(eval_result.preamble);

    // 11. Render elements to body string
    let rendered = renderer.render(&elements, &element_renderer)?;

    // 12. Transform body (format-specific: slide splitting, color defs)
    let rendered = renderer.transform_body(&rendered, &element_renderer);

    // 13. Cross-ref resolution (skipped in collection mode pass 1)
    let rendered = if options.skip_crossref {
        rendered
    } else {
        renderer.resolve_crossrefs(&rendered, &element_renderer)
    };

    Ok(RenderResult { rendered, metadata, element_renderer })
}

/// Full render pipeline. Returns (output_path, rendered_content, renderer).
pub fn render_file(
    input: &Path,
    output: Option<&Path>,
    format: Option<&str>,
    overrides: &[String],
    target: Option<&project::Target>,
    project_root: Option<&Path>,
    project_var: Option<&toml::Value>,
    output_dir: Option<&str>,
) -> Result<(PathBuf, String, Box<dyn formats::OutputRenderer>)> {
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

    // Determine output extension (target override or renderer default)
    let preliminary_format = resolved_format.as_deref().unwrap_or("html");
    let renderer = formats::create_renderer(preliminary_format)?;
    let ext = target.map(|t| t.output_extension()).unwrap_or(renderer.extension());

    // Resolve output path
    let output_path = if let Some(o) = output {
        o.to_path_buf()
    } else if let (Some(_), Some(fmt)) = (target, format) {
        // Use target-aware output path when a target is specified
        project::resolve_target_output_path(input, fmt, ext, project_root, output_dir)
    } else {
        input.with_extension(ext)
    };

    let result = render_core(input, &output_path, resolved_format.as_deref(), overrides, project_var, None, &RenderCoreOptions::default())?;

    // Assemble page (page template wrapping)
    let final_output = renderer
        .assemble_page(&result.rendered, &result.metadata, &result.element_renderer)
        .unwrap_or(result.rendered);

    // Transform document (format-specific post-template)
    let final_output = renderer.transform_document(&final_output, &result.element_renderer);

    Ok((output_path, final_output, renderer))
}
