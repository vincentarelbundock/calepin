#[macro_use]
mod cli;
mod commands;
mod context;
pub(crate) use context::{ProjectContext, resolve_context, apply_engine_override};
mod references;
pub(crate) use references::{bibliography, crossref};
mod date;
mod engines;
mod jinja;
mod math;
mod metadata;
mod parse;
mod pipeline;
mod preview;
mod render;
mod collection;
mod project;
mod tools;

// Grouped modules with crate-level re-exports for backward compatibility.
mod base;
pub(crate) use base::{types, value, paths, util};

mod modules;
pub(crate) use modules::{registry, manifest as module_manifest, theme as theme_manifest};

mod emit;
mod formats;
pub(crate) use render::typst_compile;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command};

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
