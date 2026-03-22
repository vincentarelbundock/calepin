#[macro_use]
mod cli;
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
mod paths;
mod project;
mod types;
mod util;
mod value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;

use cli::{Cli, Command, RenderArgs, PreviewArgs, InfoAction};
use render::elements::ElementRenderer;
use engines::r::RSession;
use engines::python::PythonSession;
use engines::EngineContext;
use engines::cache::CacheState;

/// Resolved project context: project config + target, shared by render and preview.
struct ProjectContext {
    project_root: Option<PathBuf>,
    project_config: Option<project::ProjectConfig>,
    target_name: String,
    target: project::Target,
}

impl ProjectContext {
    /// Get the project-level `[var]` table, if any.
    fn project_var(&self) -> Option<&toml::Value> {
        self.project_config.as_ref().and_then(|c| c.var.as_ref())
    }
}

/// Resolve project config and target from an input file and optional CLI target flag.
/// Falls back to front matter `target:`, then "html".
fn resolve_context(input: &Path, cli_target: Option<&str>) -> Result<ProjectContext> {
    let input_dir = input.parent().unwrap_or(Path::new("."));
    let abs_input_dir = if input_dir.is_relative() {
        std::env::current_dir().unwrap_or_default().join(input_dir)
    } else {
        input_dir.to_path_buf()
    };

    let project_root = project::find_project_root(&abs_input_dir);
    let project_config = project_root.as_ref().and_then(|root| {
        let cfg_path = project::config_path(root)?;
        match project::load_project_config(&cfg_path) {
            Ok(config) => Some(config),
            Err(e) => {
                eprintln!("Warning: failed to load {}: {}", cfg_path.display(), e);
                None
            }
        }
    });

    // Target name: CLI flag -> front matter -> "html"
    let target_name = if let Some(name) = cli_target {
        name.to_string()
    } else {
        // Read front matter to check for target:
        if let Ok(text) = fs::read_to_string(input) {
            if let Ok((meta, _)) = parse::yaml::split_yaml(&text) {
                meta.target.unwrap_or_else(|| "html".to_string())
            } else {
                "html".to_string()
            }
        } else {
            "html".to_string()
        }
    };

    let target = project::resolve_target(&target_name, project_config.as_ref())?;

    Ok(ProjectContext {
        project_root,
        project_config,
        target_name,
        target,
    })
}

/// Parse CLI args, injecting "render" as default subcommand when the first
/// positional argument looks like a file path rather than a known subcommand.
fn parse_cli() -> Cli {
    let args: Vec<String> = std::env::args().collect();

    let known = ["render", "preview", "init", "info"];

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
        Command::Init { template } => {
            eprintln!("Project init (template: {}) is not yet implemented.", template);
            Ok(())
        }
        Command::Info { action } => handle_info(action),
    }
}

fn handle_render(args: RenderArgs) -> Result<()> {
    let input = &args.input;

    // Site mode: .toml config with [site] section, or legacy .yaml manifest
    if cli::is_site_config(input) {
        let output = args.output.unwrap_or_else(|| PathBuf::from("output"));
        return site::build_site(Some(input.as_path()), &output, args.clean, args.quiet);
    }

    let ctx = resolve_context(input, args.target.as_deref())?;

    let (output_path, final_output, renderer) = render_file(
        input,
        args.output.as_deref(),
        Some(&ctx.target_name),
        &args.overrides,
        Some(&ctx.target),
        ctx.project_root.as_deref(),
        ctx.project_var(),
    )?;

    renderer.write_output(&final_output, &output_path)?;

    if !args.quiet {
        eprintln!("→ {}", output_path.display());
    }

    // Compile step: runs automatically when the target defines [compile]
    if let Some(ref compile_cfg) = ctx.target.compile {
        run_compile_step(&output_path, compile_cfg, args.quiet)?;
    }

    Ok(())
}

/// Run a target's compile step.
pub fn run_compile_step(
    rendered_path: &Path,
    compile_cfg: &project::CompileConfig,
    quiet: bool,
) -> Result<()> {
    let command = compile_cfg.command.as_deref()
        .ok_or_else(|| anyhow::anyhow!("Target compile section has no command"))?;
    let compile_ext = compile_cfg.extension.as_deref()
        .ok_or_else(|| anyhow::anyhow!("Target compile section has no extension"))?;

    let output_path = rendered_path.with_extension(compile_ext);
    let cmd = command
        .replace("{input}", &rendered_path.to_string_lossy())
        .replace("{output}", &output_path.to_string_lossy());

    if !quiet {
        eprintln!("  compiling: {}", cmd);
    }

    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .status()
        .with_context(|| format!("Failed to run compile command: {}", cmd))?;

    if !status.success() {
        anyhow::bail!("Compile command failed: {}", cmd);
    }

    if !quiet {
        eprintln!("→ {}", output_path.display());
    }

    Ok(())
}

fn handle_preview(args: PreviewArgs) -> Result<()> {
    // Directory: serve it over HTTP
    if args.input.is_dir() {
        return site::serve(&args.input, args.port);
    }
    // Project manifest: build then serve
    if cli::is_site_config(&args.input) {
        let config_dir = args.input.parent().unwrap_or(Path::new("."));
        let output = config_dir.join("_site");
        site::build_site(Some(args.input.as_path()), &PathBuf::from("_site"), true, false)?;
        return site::serve(&output, args.port);
    }
    // Resolve target using the same path as render
    let ctx = resolve_context(&args.input, args.target.as_deref())?;
    preview::run(&args.input, &args, &ctx.target_name, &ctx.target)
}


fn handle_info(action: InfoAction) -> Result<()> {
    match action {
        InfoAction::Csl => {
            use hayagriva::archive::ArchivedStyle;

            println!("Calepin uses CSL (Citation Style Language) for bibliography");
            println!("formatting. Over 2,600 styles are available from the Zotero");
            println!("style repository:");
            println!();
            println!("  https://www.zotero.org/styles");
            println!();
            println!("Download a .csl file and place it in assets/csl/, then set");
            println!("csl: in calepin.toml or in document front matter.");
            println!();
            println!("The following shortcuts are also available as built-in names");
            println!("(no download required):");
            println!();

            let mut names: Vec<&str> = ArchivedStyle::all().iter()
                .map(|s| s.names()[0])
                .collect();
            names.sort();

            // Print comma-separated, wrapped at 79 characters
            let joined = names.join(", ");
            let mut line = String::from("  ");
            for word in joined.split(' ') {
                if line.len() + 1 + word.len() > 79 && line.len() > 2 {
                    println!("{}", line);
                    line = format!("  {}", word);
                } else {
                    if line.len() > 2 { line.push(' '); }
                    line.push_str(word);
                }
            }
            if !line.trim().is_empty() {
                println!("{}", line);
            }
            Ok(())
        }
        InfoAction::Themes => {
            println!("Built-in syntax highlighting themes:\n");
            if let Some(dir) = render::elements::BUILTIN_PROJECT.get_dir("assets/highlighting") {
                let mut names: Vec<&str> = dir.files()
                    .filter_map(|f| {
                        if f.path().extension()?.to_str()? == "tmTheme" {
                            f.path().file_stem()?.to_str()
                        } else {
                            None
                        }
                    })
                    .collect();
                names.sort();
                for name in &names {
                    println!("  {}", name);
                }
                println!("\n{} themes available.", names.len());
            }
            println!("Custom themes: place a .tmTheme file in assets/highlighting/");
            Ok(())
        }
        InfoAction::Completions { shell } => {
            let mut cmd = <Cli as clap::CommandFactory>::command();
            clap_complete::generate(shell, &mut cmd, "calepin", &mut std::io::stdout());
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
    project_var: Option<&toml::Value>,
) -> Result<RenderResult> {

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

    let t_total = if *TIMING { Some(Instant::now()) } else { None };

    // 1. Read input file
    let input_text = fs::read_to_string(input)
        .with_context(|| format!("Failed to read input file: {}", input.display()))?;

    // 2. Parse YAML front matter, then apply CLI overrides
    let (mut metadata, body) = timed!("parse_yaml", parse::yaml::split_yaml(&input_text)?);
    let body = render::markers::sanitize(&body);
    metadata.apply_overrides(overrides);
    metadata.resolve_date(Some(input));

    // Merge project-level var as defaults (front matter wins)
    if let Some(pv) = project_var {
        if let Some(table) = pv.as_table() {
            for (key, val) in table {
                if !metadata.var.contains_key(key) {
                    metadata.var.insert(key.clone(), crate::value::from_toml(val.clone()));
                }
            }
        }
    }

    // 2b. Construct path context and validate paths
    let mut path_ctx = paths::PathContext::for_single_file(input, output_path);
    path_ctx.apply_metadata(&metadata);
    let input_name = input.file_name()
        .unwrap_or_default()
        .to_string_lossy();
    paths::validate_paths(&metadata, &path_ctx, &input_name)?;

    // 3. Create renderer for this format
    let format_str = format
        .map(|s| s.to_string())
        .or_else(|| metadata.target.clone())
        .unwrap_or_else(|| "html".to_string());
    let renderer = formats::create_renderer(&format_str)?;

    // 4. Expand includes before block parsing (so included code chunks are parsed)
    let body = timed!("expand_includes", jinja_engine::expand_includes(&body, &path_ctx.document_dir));

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
    let registry = timed!("load_plugins", std::rc::Rc::new(
        registry::PluginRegistry::load(&metadata.plugins, &path_ctx.document_dir)
    ));

    // 7. Create element renderer
    let highlight_config = metadata.var.get("highlight-style")
        .map(|v| filters::highlighting::parse_highlight_config(v))
        .unwrap_or_else(|| {
            // Defaults from built-in calepin.toml [meta].highlight
            let cfg = project::builtin_config();
            let defaults = cfg.meta.as_ref().and_then(|m| m.highlight.as_ref());
            filters::highlighting::HighlightConfig::LightDark {
                light: defaults.and_then(|h| h.light.clone()).unwrap_or_else(|| "github".to_string()),
                dark: defaults.and_then(|h| h.dark.clone()).unwrap_or_else(|| "nord".to_string()),
            }
        });
    let mut element_renderer = ElementRenderer::new(renderer.base_format(), highlight_config);
    element_renderer.number_sections = metadata.number_sections;
    element_renderer.shift_headings = metadata.title.is_some();
    element_renderer.default_fig_cap_location = metadata.var.get("fig-cap-location")
        .and_then(|v| v.as_str()).map(|s| s.to_string());

    // 8. Evaluate: execute code chunks and produce elements
    let stem = output_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let fig_dir = path_ctx.figures_dir(&stem);
    let fig_ext = renderer.default_fig_ext();
    let cache_enabled = metadata.var.get("execute")
        .and_then(|v| v.get("cache"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let cache_dir = path_ctx.cache_root(&stem);
    let mut cache = CacheState::new(input, &cache_dir, cache_enabled);
    let eval_result = timed!("evaluate", engines::evaluate(&blocks, &fig_dir, fig_ext, renderer.base_format(), &metadata, &registry, &mut ctx, &mut cache)?);
    let mut elements = eval_result.elements;

    // 9. Bibliography
    timed!("bibliography", filters::bibliography::process_citations(&mut elements, &metadata, &path_ctx.document_dir)?);

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
    target: Option<&project::Target>,
    project_root: Option<&Path>,
    project_var: Option<&toml::Value>,
) -> Result<(PathBuf, String, Box<dyn formats::OutputRenderer>)> {
    // If we have a target, use its base as the format
    let resolved_format = if let Some(t) = target {
        Some(t.base.clone())
    } else {
        format
            .map(|s| s.to_string())
            .or_else(|| {
                output
                    .and_then(|p| p.extension())
                    .and_then(|e| e.to_str())
                    .map(|ext| formats::format_from_extension(ext).to_string())
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
        project::resolve_target_output_path(input, fmt, ext, project_root)
    } else {
        input.with_extension(ext)
    };

    let result = render_core(input, &output_path, resolved_format.as_deref(), overrides, project_var)?;

    let final_output = renderer
        .apply_template(&result.rendered, &result.metadata, &result.element_renderer)
        .unwrap_or(result.rendered);

    Ok((output_path, final_output, renderer))
}
