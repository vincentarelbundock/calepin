//! Core render pipeline: parse, evaluate, render.
//!
//! This module contains the main rendering pipeline that transforms a .qmd file
//! into output (HTML, LaTeX, Typst, Markdown). It orchestrates parsing, code
//! execution, bibliography processing, and format conversion.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::engines;
use crate::engines::r::RSession;
use crate::engines::python::PythonSession;
use crate::engines::EngineContext;
use crate::engines::cache::CacheState;
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
    /// Cross-reference data collected from this page (populated when skip_crossref is true).
    pub ref_data: Option<crate::crossref::PageRefData>,
}

/// Options for the core render pipeline that control collection-specific behavior.
#[derive(Default)]
pub struct RenderCoreOptions {
    /// When true, skip cross-reference resolution (pass 1 of two-pass pipeline).
    /// The caller is responsible for resolving refs globally in pass 2.
    pub skip_crossref: bool,
    /// Chapter number for this page in a collection. When set, section numbering
    /// uses this as the top-level counter (e.g., chapter 2 -> sections 2.1, 2.2).
    pub chapter_number: Option<usize>,
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
) -> Result<RenderResult> {
    render_core_with_options(input, output_path, format, overrides, project_var, project_root_override, &RenderCoreOptions::default())
}

/// Core render pipeline with collection options (chapter numbering, skip_crossref).
pub fn render_core_with_options(
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

    // 4a. Preprocess body (custom format hook)
    let body = renderer.preprocess_body(&body)?;

    // 4b. Parse body into blocks
    let blocks = parse::blocks::parse_body(&body)?;

    // 5. Initialize engine subprocesses only if needed
    //    Working directory is set to the input file's parent so that relative
    //    paths in code chunks (e.g., read.csv("data.csv")) resolve correctly.
    let input_dir = input.parent().and_then(|p| if p.as_os_str().is_empty() { None } else { Some(p) });
    let mut r_session = if engines::util::needs_engine(&blocks, &body, &metadata, "r") {
        Some(RSession::init(renderer.engine(), input_dir)?)
    } else {
        None
    };
    let mut py_session = if engines::util::needs_engine(&blocks, &body, &metadata, "python") {
        Some(PythonSession::init(input_dir)?)
    } else {
        None
    };
    let mut sh_session = if engines::util::needs_engine(&blocks, &body, &metadata, "sh") {
        Some(engines::sh::ShSession::init(input_dir)?)
    } else {
        None
    };
    let mut ctx = EngineContext {
        r: r_session.as_mut(),
        python: py_session.as_mut(),
        sh: sh_session.as_mut(),
    };

    // 5b. Evaluate inline code in metadata fields (title, date, etc.)
    metadata.evaluate_inline(&mut ctx);

    // 6. Load plugin registry
    let registry = std::rc::Rc::new(
        registry::PluginRegistry::load(&metadata.plugins, &path_ctx.project_root)
    );

    // 7. Create element renderer
    let highlight_config = metadata.var.get("highlight-style")
        .map(|v| crate::render::highlighting::parse_highlight_config(v))
        .unwrap_or_else(|| {
            let defs = project::get_defaults();
            let hl = defs.highlight.as_ref();
            let cfg = project::builtin_config();
            let meta_hl = cfg.highlight.as_ref();
            crate::render::highlighting::HighlightConfig::LightDark {
                light: hl.and_then(|h| h.light.clone())
                    .or_else(|| meta_hl.and_then(|h| h.light.clone()))
                    .unwrap_or_else(|| "github".to_string()),
                dark: hl.and_then(|h| h.dark.clone())
                    .or_else(|| meta_hl.and_then(|h| h.dark.clone()))
                    .unwrap_or_else(|| "nord".to_string()),
            }
        });
    let mut element_renderer = ElementRenderer::new(renderer.engine(), highlight_config);
    element_renderer.number_sections = metadata.number_sections;
    element_renderer.convert_math = metadata.convert_math;
    element_renderer.shift_headings = metadata.title.is_some();
    element_renderer.chapter_number = options.chapter_number;
    // Initialize section counters with chapter number as top-level counter
    if let Some(ch) = options.chapter_number {
        let mut counters = [0usize; 6];
        counters[0] = ch;
        element_renderer.set_section_counters(counters);
    }
    element_renderer.default_fig_cap_location = metadata.var.get("fig_cap_location")
        .and_then(|v| v.as_str()).map(|s| s.to_string());

    // 8. Evaluate: execute code chunks and produce elements
    //    Use relative path from project root (without extension) as the cache/figure
    //    key, so site builds with nested pages (e.g., posts/foo/index.qmd) don't collide.
    let rel_stem = input.strip_prefix(&path_ctx.project_root)
        .unwrap_or(input)
        .with_extension("")
        .to_string_lossy()
        .replace('\\', "/");
    let fig_dir = path_ctx.figures_dir(&rel_stem);
    let fig_ext = renderer.default_fig_ext();
    let cache_enabled = metadata.var.get("execute")
        .and_then(|v| v.get("cache"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let cache_dir = path_ctx.cache_root(&rel_stem);
    let mut cache = CacheState::new(input, &cache_dir, cache_enabled);
    let eval_result = engines::evaluate(&blocks, &fig_dir, fig_ext, renderer.engine(), &metadata, &registry, &mut ctx, &mut cache)?;
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

    // 13. Cross-ref resolution
    let (rendered, ref_data) = if options.skip_crossref {
        // Collection mode pass 1: collect IDs but don't resolve refs yet
        let ref_data = renderer.collect_crossref_data(&rendered, &element_renderer);
        (rendered, ref_data)
    } else {
        // Single-file mode: resolve refs immediately
        let rendered = renderer.resolve_crossrefs(&rendered, &element_renderer);
        (rendered, None)
    };

    // 14. Number sections (HTML only) -- now handled in the AST walker
    //     (render/html_emit.rs) via ElementRenderer.number_sections

    // Clean up empty fig_dir
    if fig_dir.is_dir() && std::fs::read_dir(&fig_dir).map_or(false, |mut d| d.next().is_none()) {
        std::fs::remove_dir(&fig_dir).ok();
    }

    Ok(RenderResult { rendered, metadata, element_renderer, ref_data })
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

    let result = render_core(input, &output_path, resolved_format.as_deref(), overrides, project_var, None)?;

    // Assemble page (page template wrapping)
    let final_output = renderer
        .assemble_page(&result.rendered, &result.metadata, &result.element_renderer)
        .unwrap_or(result.rendered);

    // Transform document (format-specific post-template)
    let final_output = renderer.transform_document(&final_output, &result.element_renderer);

    Ok((output_path, final_output, renderer))
}
