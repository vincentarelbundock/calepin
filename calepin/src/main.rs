#[macro_use]
mod cli;
mod batch;
mod brand;
mod compile;
mod engines;
mod filters;
mod formats;
mod parse;
mod plugin_manifest;
mod preview;
mod registry;
mod render;
mod site;
mod structures;
mod jinja_engine;
mod types;
mod util;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;

use cli::{Cli, Command, RenderArgs, PreviewArgs, SiteAction, PluginAction, HighlightAction};
use render::elements::ElementRenderer;
use engines::r::RSession;
use engines::python::PythonSession;
use engines::EngineContext;
use engines::cache::CacheState;

/// Parse CLI args, injecting "render" as default subcommand when the first
/// positional argument looks like a file path rather than a known subcommand.
fn parse_cli() -> Cli {
    let args: Vec<String> = std::env::args().collect();

    let known = ["render", "preview", "site", "plugin", "highlight", "completions"];

    let needs_inject = args.get(1).map_or(false, |arg| {
        // Don't inject for flags (--help, -v, etc.)
        if arg.starts_with('-') {
            return false;
        }
        // If it's not a known subcommand, assume it's a file path → inject "render"
        !known.contains(&arg.as_str())
    });

    if needs_inject {
        let mut patched = vec![args[0].clone(), "render".to_string()];
        patched.extend_from_slice(&args[1..]);
        Cli::parse_from(patched)
    } else {
        Cli::parse()
    }
}

fn main() -> Result<()> {
    let cli = parse_cli();

    match cli.command {
        Command::Render(args) => handle_render(args),
        Command::Preview(args) => handle_preview(args),
        Command::Site { action } => handle_site(action),
        Command::Plugin { action } => handle_plugin(action),
        Command::Highlight { action } => handle_highlight(action),
        Command::Completions { shell } => {
            let mut cmd = <Cli as clap::CommandFactory>::command();
            clap_complete::generate(shell, &mut cmd, "calepin", &mut std::io::stdout());
            Ok(())
        }
    }
}

fn handle_render(args: RenderArgs) -> Result<()> {
    // Batch mode
    if let Some(ref manifest) = args.batch {
        return batch::run_batch(manifest, !args.stdout, args.quiet);
    }

    let input = args.input.as_ref()
        .context("No input file specified. Run with --help for usage.")?;

    let (output_path, final_output, renderer) = render_file(
        input,
        args.output.as_deref(),
        args.format.as_deref(),
        &args.overrides,
    )?;

    renderer.write_output(&final_output, &output_path)?;

    if !args.quiet {
        eprintln!("→ {}", output_path.display());
    }

    if args.pdf {
        compile::compile_to_pdf(&output_path, args.quiet)?;
    }

    Ok(())
}

fn handle_preview(args: PreviewArgs) -> Result<()> {
    preview::run(&args.input, &args)
}

fn handle_site(action: SiteAction) -> Result<()> {
    match action {
        SiteAction::Build { config, output, clean, quiet } => {
            site::build_site(config.as_deref(), &output, clean, quiet)
        }
        SiteAction::Init { template } => {
            eprintln!("Site init (template: {}) is not yet implemented.", template);
            Ok(())
        }
        SiteAction::Preview { config, port } => {
            let output = std::path::PathBuf::from("_site");
            site::build_site(config.as_deref(), &output, true, false)?;
            site::serve(&output, port)
        }
    }
}

fn handle_plugin(action: PluginAction) -> Result<()> {
    match action {
        PluginAction::Init { ref name } => plugin_init(name),
        PluginAction::List => plugin_list(),
    }
}

fn handle_highlight(action: HighlightAction) -> Result<()> {
    match action {
        HighlightAction::List => {
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
            Ok(())
        }
        HighlightAction::Preview { theme } => {
            eprintln!("Highlight preview ({}) is not yet implemented.", theme);
            Ok(())
        }
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
    render_core_with_brand(input, output_path, format, overrides, None)
}

/// Whether `CALEPIN_TIMING=1` is set (checked once at startup).
static TIMING: LazyLock<bool> = LazyLock::new(|| std::env::var("CALEPIN_TIMING").is_ok());

/// Print a timing line to stderr if `CALEPIN_TIMING` is set.
macro_rules! timed {
    ($label:expr, $block:expr) => {{
        if *TIMING {
            let _t = Instant::now();
            let _r = $block;
            eprintln!("[timing] {:.<30} {:>8.3}ms", $label, _t.elapsed().as_secs_f64() * 1000.0);
            _r
        } else {
            $block
        }
    }};
}

/// Core render pipeline with optional site-level brand fallback.
pub fn render_core_with_brand(
    input: &Path,
    output_path: &Path,
    format: Option<&str>,
    overrides: &[String],
    site_brand: Option<&brand::Brand>,
) -> Result<RenderResult> {
    let t_total = if *TIMING { Some(Instant::now()) } else { None };

    // 1. Read input file
    let input_text = fs::read_to_string(input)
        .with_context(|| format!("Failed to read input file: {}", input.display()))?;

    // 2. Parse YAML front matter, then apply CLI overrides
    let (mut metadata, body) = timed!("parse_yaml", parse::yaml::split_yaml(&input_text)?);
    let body = render::markers::sanitize(&body);
    metadata.apply_overrides(overrides);
    metadata.resolve_date(Some(input));
    // Fall back to site-level brand if page doesn't define its own
    if metadata.brand.is_none() {
        if let Some(sb) = site_brand {
            metadata.brand = Some(sb.clone());
        }
    }

    // 3. Create renderer for this format
    let format_str = format
        .map(|s| s.to_string())
        .or_else(|| metadata.format.clone())
        .unwrap_or_else(|| "html".to_string());
    let renderer = formats::create_renderer(&format_str)?;

    // 4. Expand includes before block parsing (so included code chunks are parsed)
    let body = timed!("expand_includes", jinja_engine::expand_includes(&body));

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
    let blocks = timed!("parse_blocks", parse::blocks::parse_body(&body)?);

    // 5. Initialize engine subprocesses only if needed
    let mut r_session = if engines::util::needs_engine(&blocks, &body, &metadata, "r") {
        Some(timed!("init_r", RSession::init(renderer.base_format())?))
    } else {
        None
    };
    let mut py_session = if engines::util::needs_engine(&blocks, &body, &metadata, "python") {
        Some(timed!("init_python", PythonSession::init()?))
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

    // 6. Load plugin registry
    let registry = timed!("load_plugins", std::rc::Rc::new(registry::PluginRegistry::load(&metadata.plugins)));

    // 7. Create element renderer
    let highlight_config = metadata.var.get("highlight-style")
        .map(|v| filters::highlighting::parse_highlight_config(v))
        .unwrap_or(filters::highlighting::HighlightConfig::LightDark {
            light: "github".to_string(),
            dark: "nord".to_string(),
        });
    let mut element_renderer = ElementRenderer::new(renderer.base_format(), highlight_config);
    element_renderer.number_sections = metadata.number_sections;
    element_renderer.shift_headings = metadata.title.is_some();
    element_renderer.default_fig_cap_location = metadata.var.get("fig-cap-location")
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
    let cache_enabled = metadata.var.get("execute")
        .and_then(|v| v.as_mapping_get("cache"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let mut cache = CacheState::new(input, cache_enabled);
    let eval_result = timed!("evaluate", engines::evaluate(&blocks, &fig_dir, fig_ext, renderer.base_format(), &metadata, &registry, &mut ctx, &mut cache)?);
    let mut elements = eval_result.elements;

    // 9. Bibliography
    timed!("bibliography", filters::bibliography::process_citations(&mut elements, &metadata)?);

    // 10. Set registry on element renderer
    element_renderer.set_registry(registry);
    element_renderer.set_sc_fragments(eval_result.sc_fragments);

    // 12. Render elements to final format
    let rendered = timed!("render", renderer.render(&elements, &element_renderer)?);

    // 13. Cross-ref resolution (section IDs pre-collected from AST walk)
    let thm_nums = element_renderer.theorem_numbers();
    let walk_meta = element_renderer.walk_metadata();
    let rendered = timed!("crossref", match renderer.base_format() {
        "html" => filters::crossref::resolve_html_with_ids(&rendered, &thm_nums, &walk_meta.ids),
        "latex" => filters::crossref::resolve_latex(&rendered, &thm_nums),
        _ => filters::crossref::resolve_plain(&rendered, &thm_nums),
    });

    // 14. Number sections (HTML only) — now handled in the AST walker
    //     (render/html_ast.rs) via ElementRenderer.number_sections

    // Clean up empty fig_dir
    if fig_dir.is_dir() && std::fs::read_dir(&fig_dir).map_or(false, |mut d| d.next().is_none()) {
        std::fs::remove_dir(&fig_dir).ok();
    }

    if let Some(t) = t_total {
        eprintln!("[timing] {:=<30} {:>8.3}ms", "TOTAL ", t.elapsed().as_secs_f64() * 1000.0);
    }

    Ok(RenderResult { rendered, metadata, element_renderer })
}

/// Full render pipeline. Returns (output_path, rendered_content, renderer).
pub fn render_file(
    input: &Path,
    output: Option<&Path>,
    format: Option<&str>,
    overrides: &[String],
) -> Result<(PathBuf, String, Box<dyn formats::OutputRenderer>)> {
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

    Ok((output_path, final_output, renderer))
}

fn resolve_output_path(input: &Path, output: Option<&Path>, ext: &str) -> PathBuf {
    match output {
        Some(path) => path.to_path_buf(),
        None => input.with_extension(ext),
    }
}

fn plugin_init(name: &str) -> Result<()> {
    let dir = Path::new("_calepin").join("plugins").join(name);
    if dir.exists() {
        anyhow::bail!("Plugin directory already exists: {}", dir.display());
    }
    fs::create_dir_all(&dir)?;
    let manifest = format!(
        "name: {name}\nversion: 0.1.0\ndescription: \"\"\n\nprovides:\n  filter:\n    run: filter.py\n    match:\n      classes: [{name}]\n    contexts: [div, span]\n",
        name = name,
    );
    fs::write(dir.join("plugin.yml"), manifest)?;

    // Create a minimal filter script
    let filter_script = r#"#!/usr/bin/env python3
import json, sys

data = json.load(sys.stdin)
context = data["context"]
content = data["content"]
classes = data["classes"]
fmt = data["format"]

# Return rendered output on stdout, or exit non-zero to pass
print(content)
"#;
    fs::write(dir.join("filter.py"), filter_script)?;

    // Make it executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(dir.join("filter.py"))?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(dir.join("filter.py"), perms)?;
    }

    eprintln!("Created plugin scaffold at {}", dir.display());
    eprintln!("  plugin.yml  — manifest");
    eprintln!("  filter.py   — filter script (edit this)");
    Ok(())
}

fn plugin_list() -> Result<()> {
    // Scan project and user plugin directories
    let dirs = [
        PathBuf::from("_calepin/plugins"),
        dirs::config_dir().map(|d| d.join("calepin/plugins")).unwrap_or_default(),
    ];

    let mut found = false;
    for dir in &dirs {
        if !dir.is_dir() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.join("plugin.yml").exists() {
                    if let Ok(manifest) = plugin_manifest::PluginManifest::load(&path) {
                        let desc = manifest.description.as_deref().unwrap_or("");
                        let mut caps = Vec::new();
                        if !manifest.provides.filters.is_empty() { caps.push("filter"); }
                        if manifest.provides.shortcode.is_some() { caps.push("shortcode"); }
                        if manifest.provides.postprocess.is_some() { caps.push("postprocess"); }
                        if manifest.provides.elements.is_some() { caps.push("elements"); }
                        if manifest.provides.templates.is_some() { caps.push("templates"); }
                        if manifest.provides.csl.is_some() { caps.push("csl"); }
                        if manifest.provides.format.is_some() { caps.push("format"); }
                        println!(
                            "  {:<20} [{}] {}",
                            manifest.name,
                            caps.join(", "),
                            desc,
                        );
                        found = true;
                    }
                }
            }
        }
    }

    if !found {
        println!("No plugins found.");
        println!("  Project: _calepin/plugins/");
        println!("  User:    ~/.config/calepin/plugins/");
    }

    // Also list built-in plugins
    println!("\nBuilt-in plugins:");
    println!("  {:<20} [filter] Panel tabset rendering", "tabset");
    println!("  {:<20} [filter] Layout grid rendering", "layout");
    println!("  {:<20} [filter] Figure div rendering", "figure-div");
    println!("  {:<20} [filter] Theorem auto-numbering", "theorem");
    println!("  {:<20} [filter] Callout enrichment", "callout");

    Ok(())
}

/// Get the user config directory (cross-platform).
mod dirs {
    use std::path::PathBuf;
    pub fn config_dir() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config"))
    }
}
