//! Core render pipeline: parse, evaluate, render.
//!
//! This module contains the main rendering pipeline that transforms a .qmd file
//! into output (HTML, LaTeX, Typst, Markdown). It orchestrates parsing, code
//! execution, bibliography processing, and format conversion.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::engines;
use super::formats;
use super::formats::FormatPipeline;
use super::template;
use crate::parse;
use crate::paths;
use crate::config;
use crate::registry;
use super::elements::ElementRenderer;

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
/// If `format` is None, falls back to the format declared in front matter, then "html".
pub fn render_core(
    input: &Path,
    output_path: &Path,
    format: Option<&str>,
    overrides: &[String],
    project_root_override: Option<&Path>,
    options: &RenderCoreOptions,
    project_metadata: Option<&crate::config::Metadata>,
    target: Option<&config::Target>,
) -> Result<RenderResult> {

    // 1. Read input file
    let input_text = fs::read_to_string(input)
        .with_context(|| format!("Failed to read input file: {}", input.display()))?;

    // 2. Parse TOML front matter
    let (frontmatter, body) = crate::config::split_frontmatter(&input_text)?;
    let body = super::markers::sanitize(&body);

    // Resolve sidecar directory ({stem}_calepin/) for per-document overrides
    let sidecar_dir = paths::resolve_sidecar_dir(input);
    paths::set_page_sidecar(sidecar_dir.as_deref());

    // Merge: project < sidecar config < front matter < CLI
    // Clear document-identity fields from project config so they don't
    // override page-specific values (e.g., collection title leaking into pages).
    let mut metadata = if let Some(project_meta) = project_metadata {
        let mut m = project_meta.clone();
        m.title = None;
        m.subtitle = None;
        m.authors.clear();
        m
    } else {
        crate::config::Metadata::default()
    };
    if let Some(ref dir) = sidecar_dir {
        let sidecar_config = dir.join("config.toml");
        if sidecar_config.exists() {
            let sidecar_meta = config::load_project_metadata(&sidecar_config)?;
            metadata = metadata.merge(sidecar_meta);
        }
    }
    metadata = metadata.merge(frontmatter);
    metadata.apply_overrides(overrides);
    metadata.resolve_date(Some(input));

    // 2b. Construct path context and validate paths
    let path_ctx = paths::PathContext::new(input, output_path, project_root_override);
    let input_name = input.file_name().unwrap_or_default().to_string_lossy();
    paths::validate_paths(&metadata, &path_ctx, &input_name)?;

    // 3. Build format pipeline from target or engine name

    let format_str = format
        .map(|s| s.to_string())
        .or_else(|| metadata.target.clone())
        .unwrap_or_else(|| "html".to_string());

    // If a target object was passed, use it directly.
    // Otherwise, try to resolve from extensions, then fall back to writer.
    let pipeline = if let Some(t) = target {
        FormatPipeline::from_target(t)?
    } else {
        // Try resolving as a target (from extensions or built-in config)
        let empty_targets = std::collections::HashMap::new();
        let user_targets = project_metadata.map(|m| &m.targets).unwrap_or(&empty_targets);
        match config::resolve_target(&format_str, user_targets) {
            Ok(resolved) => FormatPipeline::from_target(&resolved)?,
            Err(_) => FormatPipeline::from_writer(&format_str)?,
        }
    };

    // Only set active target if not already set by the caller (render_file sets
    // it to the target name, which may differ from the writer/format name).
    if paths::get_active_target().is_none() {
        let empty_targets = std::collections::HashMap::new();
        let user_targets = project_metadata.map(|m| &m.targets).unwrap_or(&empty_targets);
        let chain = crate::config::extension::inheritance_chain(
            &paths::get_project_dir(), &format_str, user_targets,
        );
        paths::set_active_target_with_chain(Some(&format_str), chain);
    }

    // 3b. Inject extension vars into metadata (namespaced by extension name).
    //     Walks the extension inheritance chain and merges [vars] from each.
    //     Also includes side-loaded extensions (calepin.extensions = [...]).
    //     User vars in front matter override extension defaults.
    let project_root = paths::get_project_dir();
    inject_extension_vars(&mut metadata, &project_root);
    for ext_name in paths::get_sideloaded_extensions() {
        inject_extension_vars_for(&mut metadata, &project_root, &ext_name);
    }

    // 3c. Set active template variant selections for template resolution.
    paths::set_active_tpl(metadata.tpl.clone());

    // 4. Expand includes before block parsing (so included code chunks are parsed)
    let body = template::expand_includes(&body, &path_ctx.project_root, &format_str);

    // 4a. Parse body into blocks
    let blocks = parse::blocks::parse_body(&body)?;

    // 5. Initialize code engines (R, Python, sh) -- only starts what's needed
    let mut engines = engines::EnginePool::init(
        &blocks, &body, &metadata, pipeline.writer(),
        paths::PathContext::code_working_dir(input),
    )?;
    let mut ctx = engines.context();

    // 5b. Evaluate inline code in metadata fields (title, date, etc.)
    metadata.evaluate_inline(&mut ctx);

    // 6. Load module registry
    let mut module_names = metadata.plugins.clone();
    for ext in &metadata.extensions {
        if !module_names.contains(ext) {
            module_names.push(ext.clone());
        }
    }
    let registry = std::rc::Rc::new(
        registry::ModuleRegistry::load(&module_names, &path_ctx.project_root)
    );

    // 7. Create element renderer
    let mut element_renderer = ElementRenderer::from_metadata(pipeline.writer(), &metadata, options);
    element_renderer.set_target(target.cloned());

    // Shift headings down one level only when the document has a title AND
    // the body uses h1 (`#`) headers. If the author starts at `##`, headings
    // render at their natural level. Only check Text blocks (not code blocks).
    if metadata.title.is_some() {
        let has_h1 = blocks.iter().any(|b| match b {
            crate::types::Block::Text(t) => t.content.lines().any(|l| l.starts_with("# ")),
            _ => false,
        });
        if has_h1 {
            element_renderer.set_shift_headings(true);
        }
    }

    // 8. Evaluate: execute code chunks and produce elements
    let eval_result = engines::evaluate_document(
        input, &blocks, &body, pipeline.writer(), &metadata, &registry,
        &mut ctx, &path_ctx, pipeline.default_fig_ext(),
    )?;
    let mut elements = eval_result.elements;

    // 9. Resolve bibliography
    pipeline.resolve_bibliography(&mut elements, &metadata, &path_ctx.project_root)?;

    // 10. Set registry on element renderer
    element_renderer.set_registry(registry);
    element_renderer.set_preamble(eval_result.preamble);

    // 10b. Prepare elements (pre-render: SVG-to-PDF, etc.)
    pipeline.transform_elements(&mut elements, &element_renderer);

    // 11. Render elements to body string
    let rendered = pipeline.render(&elements, &element_renderer)?;

    // 12. Cross-ref resolution (skipped in collection mode pass 1)

    let rendered = if options.skip_crossref {
        rendered
    } else {
        pipeline.resolve_crossrefs(&rendered, &element_renderer)
    };


    Ok(RenderResult { rendered, metadata, element_renderer })
}

/// Render a single page: render_core + assemble_page + transform_document.
///
/// This is the shared per-page pipeline used by both single-document rendering
/// and collection rendering. Returns a `RenderedPage` with structured metadata
/// that project modules can transform.
pub fn render_page(
    input: &Path,
    output_path: &Path,
    format: &str,
    overrides: &[String],
    project_root: Option<&Path>,
    options: &RenderCoreOptions,
    project_metadata: Option<&config::Metadata>,
    target: Option<&config::Target>,
) -> Result<(crate::registry::RenderedPage, RenderResult)> {
    let result = render_core(input, output_path, Some(format), overrides, project_root, options, project_metadata, target)?;

    let pipeline = FormatPipeline::from_target_or_writer(target, format)?;

    // Assemble page (page template wrapping)
    let assembled = pipeline
        .assemble_page(&result.rendered, &result.metadata, &result.element_renderer)
        .unwrap_or_else(|| result.rendered.clone());

    // Document transforms (post-assembly: highlight CSS, footnotes, etc.)
    let final_output = pipeline.transform_document(&assembled, &result.element_renderer);

    // Build TOC
    let toc = if format == "html" && result.metadata.toc.as_ref().and_then(|t| t.enabled).unwrap_or(true) {
        let (depth, title) = result.metadata.toc_depth_title();
        let toc_html = crate::render::template::build_toc_html_from_body(&final_output, depth, title);
        if toc_html.is_empty() { None } else { Some(toc_html) }
    } else {
        None
    };

    let page = crate::registry::RenderedPage {
        source: input.to_path_buf(),
        output: output_path.to_path_buf(),
        body: final_output,
        title: result.metadata.title.clone(),
        date: result.metadata.date.clone(),
        subtitle: result.metadata.subtitle.clone(),
        abstract_text: result.metadata.abstract_text.clone(),
        url: String::new(),
        toc,
        lang: None,
        doc_meta: Default::default(),
        metadata: result.metadata.clone(),
    };

    Ok((page, result))
}

/// Full render pipeline. Returns (output_path, rendered_content, pipeline).
pub fn render_file(
    input: &Path,
    output: Option<&Path>,
    format: Option<&str>,
    overrides: &[String],
    target: Option<&config::Target>,
    project_root: Option<&Path>,
    output_dir: Option<&str>,
    project_metadata: Option<&crate::config::Metadata>,
) -> Result<(PathBuf, String, FormatPipeline)> {
    // If we have a target, use its writer as the format
    let resolved_format = if let Some(t) = target {
        Some(t.writer.clone())
    } else {
        format
            .map(|s| s.to_string())
            .or_else(|| {
                output
                    .and_then(|p| p.extension())
                    .and_then(|e| e.to_str())
                    .map(|ext| formats::resolve_format(ext).to_string())
            })
    };

    // Build pipeline
    let preliminary_format = resolved_format.as_deref().unwrap_or("html");
    let target_name = format.unwrap_or(preliminary_format);
    // Set active target and inheritance chain for template resolution
    let empty_targets = std::collections::HashMap::new();
    let user_targets_ref = project_metadata.map(|m| &m.targets).unwrap_or(&empty_targets);
    let chain = crate::config::extension::inheritance_chain(
        &paths::get_project_dir(), target_name, user_targets_ref,
    );
    paths::set_active_target_with_chain(Some(target_name), chain);
    let pipeline = FormatPipeline::from_target_or_writer(target, preliminary_format)?;
    // When the target's writer extension differs from the output extension
    // (e.g., typst writer -> .typ but target wants .pdf), use the writer's
    // native extension for the rendered file. Post commands handle conversion.
    let ext = if let Some(t) = target {
        let writer_ext = paths::resolve_extension(&t.writer);
        if writer_ext != t.output_extension() {
            writer_ext
        } else {
            t.output_extension()
        }
    } else {
        pipeline.extension()
    };

    // Resolve output path
    let output_path = if let Some(o) = output {
        o.to_path_buf()
    } else if let (Some(_), Some(fmt)) = (target, format) {
        // Use target-aware output path when a target is specified
        config::resolve_target_output_path(input, fmt, ext, project_root, output_dir)
    } else {
        input.with_extension(ext)
    };

    // Use the shared per-page pipeline: render_core + assemble_page + transform_document
    let (mut page, result) = render_page(input, &output_path, preliminary_format, overrides, None, &RenderCoreOptions::default(), project_metadata, target)?;

    // Run project-level modules from extensions (single-doc mode).
    if let Some(t) = target {
        let pr = project_root.unwrap_or(Path::new("."));
        let registry = crate::registry::ModuleRegistry::load(&t.modules, pr);
        let ctx = crate::registry::ProjectTransformContext {
            base_dir: pr.to_path_buf(),
            output_dir: output_path.parent().unwrap_or(Path::new(".")).to_path_buf(),
            target_name: format.unwrap_or("html").to_string(),
        };
        let mut pages = vec![page];
        if let Err(e) = registry.run_project_transforms(&mut pages, &result.metadata, pipeline.writer(), &ctx, &t.modules) {
            cwarn!("project module error: {}", e);
        }
        page = pages.into_iter().next().unwrap();
    }

    Ok((output_path, page.body, pipeline))
}

/// Inject extension vars into metadata, namespaced by extension name.
///
/// Walks the active target's extension inheritance chain and merges each
/// extension's `[vars]` into `metadata.var` as a nested table:
/// `metadata.var["ext_name"] = Table { key: value, ... }`
///
/// User vars in front matter take precedence (they're already in metadata.var).
fn inject_extension_vars(metadata: &mut config::Metadata, project_root: &Path) {
    let target_name = paths::get_active_target().unwrap_or_default();
    inject_extension_vars_for(metadata, project_root, &target_name);
}

/// Inject vars from a specific extension's inheritance chain into metadata.
fn inject_extension_vars_for(metadata: &mut config::Metadata, project_root: &Path, start_name: &str) {
    // Collect (ext_name, vars) in child-first order
    let chain: Vec<(String, std::collections::HashMap<String, toml::Value>)> =
        config::extension::walk_chain(project_root, start_name, |name, _, manifest| {
            if manifest.vars.is_empty() { None }
            else { Some((name.to_string(), manifest.vars.clone())) }
        });

    // Apply parent-first (so child vars override parent)
    for (ext_name, vars) in chain.iter().rev() {
        // Only inject if the user hasn't already set vars for this extension
        if metadata.var.contains_key(ext_name) {
            continue;
        }
        let mut table = indexmap::IndexMap::new();
        for (k, v) in vars {
            table.insert(k.clone(), crate::value::from_toml(v.clone()));
        }
        metadata.var.insert(ext_name.clone(), crate::value::Value::Table(table));
    }
}

