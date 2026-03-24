#[macro_use]
mod cli;
mod commands;
mod engines;
mod filters;
mod formats;
mod jinja;
mod parse;
mod pipeline;
mod plugin_manifest;
mod preview;
mod registry;
mod render;
mod collection;
mod structures;
mod paths;
mod project;
#[allow(dead_code)]
mod tools;
mod types;
mod typst_compile;
mod util;
mod value;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command};

/// Resolved project context: project config + target, shared by render and preview.
pub(crate) struct ProjectContext {
    project_root: Option<PathBuf>,
    project_config: Option<project::ProjectConfig>,
    target_name: String,
    target: project::Target,
    /// True when the target was explicitly set (CLI flag or front matter),
    /// false when it fell back to the default "html".
    explicit_target: bool,
}

impl ProjectContext {
    /// Get the project-level `[var]` table, if any.
    fn project_var(&self) -> Option<&toml::Value> {
        self.project_config.as_ref().and_then(|c| c.var.as_ref())
    }

    /// Get the configured output directory, if any.
    fn output_dir(&self) -> Option<&str> {
        self.project_config.as_ref().and_then(|c| c.output.as_deref())
    }
}

/// Resolve project config and target from an input file and optional CLI target flag.
/// Falls back to front matter `target:`, then "html".
pub(crate) fn resolve_context(input: &Path, cli_target: Option<&str>) -> Result<ProjectContext> {
    let input_dir = input.parent().unwrap_or(Path::new("."));
    let abs_input_dir = if input_dir.is_relative() {
        std::env::current_dir().unwrap_or_default().join(input_dir)
    } else {
        input_dir.to_path_buf()
    };

    // Project root is the directory containing the input file.
    let (project_root, project_config) = {
        let cfg_path = abs_input_dir.join("_calepin.toml");
        if cfg_path.exists() {
            match project::load_project_config(&cfg_path) {
                Ok(config) => (Some(abs_input_dir.clone()), Some(config)),
                Err(e) => {
                    eprintln!("Warning: failed to load {}: {}", cfg_path.display(), e);
                    (Some(abs_input_dir.clone()), None)
                }
            }
        } else {
            (None, None)
        }
    };

    // Target name: CLI flag -> front matter -> default from config
    let default_format = project::get_defaults().format.clone().unwrap_or_else(|| "html".to_string());
    let (target_name, explicit_target) = if let Some(name) = cli_target {
        (name.to_string(), true)
    } else {
        // Read front matter to check for target:
        if let Ok(text) = fs::read_to_string(input) {
            if let Ok((meta, _)) = parse::yaml::split_yaml(&text) {
                match meta.target {
                    Some(t) => (t, true),
                    None => (default_format.clone(), false),
                }
            } else {
                (default_format.clone(), false)
            }
        } else {
            (default_format.clone(), false)
        }
    };

    let target = project::resolve_target(&target_name, project_config.as_ref())?;

    let mut defaults = project::resolve_defaults(project_config.as_ref());
    if let Some(embed) = target.embed_resources {
        defaults.embed_resources = Some(embed);
    }
    project::set_active_defaults(defaults);

    // In document mode (no _calepin.toml), the project root is the
    // input file's parent directory so that all paths resolve relative to it.
    let effective_root = project_root.clone().unwrap_or_else(|| abs_input_dir.clone());
    paths::set_project_root(Some(&effective_root));

    Ok(ProjectContext {
        project_root: Some(effective_root),
        project_config,
        target_name,
        target,
        explicit_target,
    })
}

/// Apply `--base` override to a resolved project context.
///
/// Validates that the base is allowed for the target:
///   - `pdf`: html, latex, typst, markdown
///   - `book`: latex, typst
///   - `website`: html
///   - others: no override allowed (base is fixed)
pub(crate) fn apply_base_override(ctx: &mut ProjectContext, base: Option<&str>) -> Result<()> {
    let Some(base) = base else { return Ok(()) };

    let allowed: &[&str] = match ctx.target_name.as_str() {
        "pdf" => &["html", "latex", "typst", "markdown"],
        "book" => &["latex", "typst"],
        other => anyhow::bail!(
            "--base is only valid for pdf or book targets (got '{}')", other
        ),
    };

    if !allowed.contains(&base) {
        anyhow::bail!(
            "--base '{}' is not valid for target '{}'. Allowed: {}",
            base, ctx.target_name, allowed.join(", ")
        );
    }

    ctx.target.base = base.to_string();

    // Update extension and fig-extension to match the new base
    let builtin = project::builtin_config().targets.get(base);
    if let Some(b) = builtin {
        ctx.target.extension = b.extension.clone();
        ctx.target.fig_extension = b.fig_extension.clone();
        ctx.target.compile = b.compile.clone();
        ctx.target.preview = b.preview.clone();
    }

    Ok(())
}

/// Parse CLI args, injecting "render" as default subcommand when the first
/// positional argument looks like a file path rather than a known subcommand.
fn parse_cli() -> Cli {
    let args: Vec<String> = std::env::args().collect();

    let known = ["render", "preview", "flush", "new", "info"];

    let needs_inject = args.get(1).map_or(false, |arg| {
        // Don't inject for flags (--help, -v, etc.)
        if arg.starts_with('-') {
            return false;
        }
        // If it's not a known subcommand, assume it's a file path -> inject "render"
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
        Command::Render(args) => commands::render::handle_render(args),
        Command::Preview(args) => commands::preview::handle_preview(args),
        Command::Flush { path, yes, cache, files, compilation, all } => {
            // Default to --all when no category flag is given
            let (do_cache, do_files, do_compilation) = if all || (!cache && !files && !compilation) {
                (true, true, true)
            } else {
                (cache, files, compilation)
            };
            // If path is not a directory, search for it as a name within
            // the cache/files structure
            let (root, stem) = if path.is_dir() {
                (path, None)
            } else {
                let name = path.to_string_lossy().to_string();
                (PathBuf::from("."), Some(name))
            };
            commands::flush::handle_flush(&root, stem.as_deref(), yes, do_cache, do_files, do_compilation)
        }
        Command::New { action } => commands::new::handle_new(action),
        Command::Info { action } => commands::info::handle_info(action),
    }
}
