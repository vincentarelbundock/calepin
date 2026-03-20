#[macro_use]
mod cli;
mod batch;
mod compile;
mod engines;
mod filters;
mod formats;
mod parse;
mod plugins;
mod preview;
mod render;
mod structures;
mod types;
mod util;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;

use cli::Cli;
use render::elements::ElementRenderer;
use engines::r::RSession;
use engines::python::PythonSession;
use engines::EngineContext;
use engines::cache::CacheState;

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Shell completions: print and exit
    if let Some(shell) = cli.completions {
        let mut cmd = <Cli as clap::CommandFactory>::command();
        clap_complete::generate(shell, &mut cmd, "calepin", &mut std::io::stdout());
        return Ok(());
    }

    // List highlight styles: print and exit
    if cli.list_highlight_styles {
        println!("Built-in syntax highlighting themes:\n");
        let themes = [
            ("github", "Light"),
            ("catppuccin-latte", "Light"),
            ("coldark-cold", "Light"),
            ("gruvbox-light", "Light"),
            ("monokai-extended-light", "Light"),
            ("base16-ocean-light", "Light"),
            ("onehalf-light", "Light"),
            ("solarized-light", "Light"),
            ("solarized-light-alt", "Light"),
            ("1337", "Dark"),
            ("ansi", "Dark"),
            ("base16", "Dark"),
            ("base16-256", "Dark"),
            ("base16-eighties-dark", "Dark"),
            ("base16-mocha-dark", "Dark"),
            ("base16-ocean-dark", "Dark"),
            ("catppuccin-frappe", "Dark"),
            ("catppuccin-macchiato", "Dark"),
            ("catppuccin-mocha", "Dark"),
            ("coldark-dark", "Dark"),
            ("darkneon", "Dark"),
            ("dracula", "Dark"),
            ("gruvbox-dark", "Dark"),
            ("monokai-extended", "Dark"),
            ("monokai-extended-bright", "Dark"),
            ("monokai-extended-origin", "Dark"),
            ("nord", "Dark"),
            ("onehalf-dark", "Dark"),
            ("snazzy", "Dark"),
            ("solarized-dark", "Dark"),
            ("solarized-dark-alt", "Dark"),
            ("twodark", "Dark"),
        ];
        for (name, style) in themes {
            println!("  {:<28} {}", name, style);
        }
        println!("\nCustom themes: set highlight-style to a .tmTheme file path.");
        return Ok(());
    }

    // Batch mode: render multiple files from a JSON manifest
    if let Some(ref manifest) = cli.batch {
        return batch::run_batch(manifest, !cli.batch_stdout, cli.quiet);
    }

    let input = cli.input.as_ref()
        .context("No input file specified. Run with --help for usage.")?;

    if cli.preview {
        preview::run(input, &cli)
    } else {
        let (output_path, final_output) = render_file(
            input,
            cli.output.as_deref(),
            cli.format.as_deref(),
            &cli.overrides,
        )?;

        fs::write(&output_path, &final_output)
            .with_context(|| format!("Failed to write output file: {}", output_path.display()))?;

        if !cli.quiet {
            eprintln!("→ {}", output_path.display());
        }

        if cli.compile {
            compile::compile_to_pdf(&output_path, cli.quiet)?;
        }

        Ok(())
    }
}

/// Result of the core render pipeline (before page template wrapping).
pub struct RenderResult {
    pub rendered: String,
    pub metadata: types::Metadata,
    pub element_renderer: ElementRenderer,
}

/// Core render pipeline: parse, evaluate, render. Does NOT apply the page template.
/// If `format` is None, falls back to the format declared in YAML front matter, then "html".
pub fn render_core(
    input: &Path,
    output_path: &Path,
    format: Option<&str>,
    overrides: &[String],
) -> Result<RenderResult> {
    // 1. Read input file
    let input_text = fs::read_to_string(input)
        .with_context(|| format!("Failed to read input file: {}", input.display()))?;

    // 2. Parse YAML front matter, then apply CLI overrides
    let (mut metadata, body) = parse::yaml::split_yaml(&input_text)?;
    let body = render::markers::sanitize(&body);
    metadata.apply_overrides(overrides);
    metadata.resolve_date(Some(input));

    // 3. Create renderer for this format
    let format_str = format
        .map(|s| s.to_string())
        .or_else(|| metadata.format.clone())
        .unwrap_or_else(|| "html".to_string());
    let renderer = formats::create_renderer(&format_str)?;

    // 4. Expand includes before block parsing (so included code chunks are parsed)
    let body = filters::shortcodes::expand_includes(&body);

    // 4a. Preprocess hook: pipe body through script if custom format defines one
    let body = if let Some(script) = renderer.preprocess() {
        let input = serde_json::json!({
            "body": body,
            "format": format_str,
        });
        formats::run_script(script, &input.to_string(), &[])?
    } else {
        body
    };

    // 4b. Parse body into blocks
    let blocks = parse::blocks::parse_body(&body)?;

    // 5. Initialize engine subprocesses only if needed
    let mut r_session = if engines::util::needs_engine(&blocks, &body, &metadata, "r") {
        Some(RSession::init()?)
    } else {
        None
    };
    let mut py_session = if engines::util::needs_engine(&blocks, &body, &metadata, "python") {
        Some(PythonSession::init()?)
    } else {
        None
    };
    let mut sh_session = if engines::util::needs_engine(&blocks, &body, &metadata, "sh") {
        Some(engines::sh::ShSession::init()?)
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

    // 6. Load WASM plugins declared in front matter
    let plugins = plugins::load_plugins(&metadata.plugins);

    // 7. Create element renderer
    let highlight_config = metadata.extra.get("highlight-style")
        .map(|v| filters::highlighting::parse_highlight_config(v))
        .unwrap_or(filters::highlighting::HighlightConfig::LightDark {
            light: "github".to_string(),
            dark: "nord".to_string(),
        });
    let mut element_renderer = ElementRenderer::new(renderer.base_format(), highlight_config);
    element_renderer.number_sections = metadata.number_sections;
    element_renderer.shift_headings = metadata.title.is_some();
    element_renderer.default_fig_cap_location = metadata.extra.get("fig-cap-location")
        .and_then(|v| v.as_str()).map(|s| s.to_string());

    // 8. Evaluate: execute code chunks and produce elements
    let fig_dir = output_path.with_file_name(format!(
        "{}_files",
        output_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
    ));
    let fig_ext = renderer.default_fig_ext();
    let cache_enabled = metadata.extra.get("execute")
        .and_then(|v| v.as_mapping_get("cache"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let mut cache = CacheState::new(input, cache_enabled);
    let eval_result = engines::evaluate(&blocks, &fig_dir, fig_ext, renderer.base_format(), &metadata, &plugins, &mut ctx, &mut cache)?;
    let mut elements = eval_result.elements;

    // 9. Bibliography
    filters::bibliography::process_citations(&mut elements, &metadata)?;

    // 10. Set plugins on element renderer
    element_renderer.set_plugins(plugins);
    element_renderer.set_sc_fragments(eval_result.sc_fragments, eval_result.escaped_sc_fragments);

    // 12. Render elements to final format
    let rendered = renderer.render(&elements, &element_renderer)?;

    // 13. Cross-ref resolution
    let thm_nums = element_renderer.theorem_numbers();
    let rendered = match renderer.base_format() {
        "html" => filters::crossref::resolve_html(&rendered, &thm_nums),
        "latex" => filters::crossref::resolve_latex(&rendered, &thm_nums),
        _ => filters::crossref::resolve_plain(&rendered, &thm_nums),
    };

    // 14. Number sections (HTML only)
    let rendered = if metadata.number_sections && renderer.base_format() == "html" {
        formats::html::number_sections_html(&rendered)
    } else {
        rendered
    };

    // Clean up empty fig_dir
    if fig_dir.is_dir() && std::fs::read_dir(&fig_dir).map_or(false, |mut d| d.next().is_none()) {
        std::fs::remove_dir(&fig_dir).ok();
    }

    Ok(RenderResult { rendered, metadata, element_renderer })
}

/// Full render pipeline. Returns (output_path, rendered_content).
pub fn render_file(
    input: &Path,
    output: Option<&Path>,
    format: Option<&str>,
    overrides: &[String],
) -> Result<(PathBuf, String)> {
    // Resolve format from CLI flag or output extension; None falls back to metadata in render_core
    let resolved_format = format
        .map(|s| s.to_string())
        .or_else(|| {
            output
                .and_then(|p| p.extension())
                .and_then(|e| e.to_str())
                .map(|ext| formats::format_from_extension(ext).to_string())
        });

    // We need a preliminary format to determine the output extension/path.
    // render_core will finalize from metadata if this is None.
    let preliminary_format = resolved_format.as_deref().unwrap_or("html");
    let renderer = formats::create_renderer(preliminary_format)?;
    let output_path = resolve_output_path(input, output, renderer.extension());

    let result = render_core(input, &output_path, resolved_format.as_deref(), overrides)?;

    let final_output = renderer
        .apply_template(&result.rendered, &result.metadata, &result.element_renderer)
        .unwrap_or(result.rendered);

    Ok((output_path, final_output))
}

fn resolve_output_path(input: &Path, output: Option<&Path>, ext: &str) -> PathBuf {
    match output {
        Some(path) => path.to_path_buf(),
        None => input.with_extension(ext),
    }
}
