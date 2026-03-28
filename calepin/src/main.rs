#[macro_use]
mod cli;
mod man;
mod types;
mod config;
mod emit;
mod engines;
mod jinja;
mod modules;
mod parse;
mod preview;
mod references;
mod render;
mod collection;
mod themes;
mod utils;

// Crate-level re-exports: short paths for pervasive types and modules.
pub(crate) use config::{ProjectContext, resolve_context, apply_writer_override};
pub(crate) use config::{paths, value};
pub(crate) use modules::{registry, manifest as module_manifest};
pub(crate) use references::{bibliography, crossref};
pub(crate) use utils::util;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command};

/// Parse CLI args, injecting "render" as default subcommand when the first
/// positional argument looks like a file path rather than a known subcommand.
fn parse_cli() -> Cli {
    let args: Vec<String> = std::env::args().collect();

    let known = ["render", "preview", "flush", "new", "man", "info"];

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
        Command::Render(args) => cli::render::handle_render(args),
        Command::Preview(args) => cli::preview::handle_preview(args),
        Command::Flush { path, yes, cache, files, compilation, all } => {
            // Default to --all when no category flag is given
            let (do_cache, do_files, do_compilation) = if all || (!cache && !files && !compilation) {
                (true, true, true)
            } else {
                (cache, files, compilation)
            };
            // When no path is given, auto-discover _calepin/ and *_calepin/
            // sidecar directories in the current directory.
            let (root, stem) = if let Some(path) = path {
                if path.is_dir() {
                    (path, None)
                } else {
                    let name = path.to_string_lossy().to_string();
                    (PathBuf::from("."), Some(name))
                }
            } else {
                (PathBuf::from("."), None)
            };
            cli::flush::handle_flush(&root, stem.as_deref(), yes, do_cache, do_files, do_compilation)
        }
        Command::New { action } => match action {
            cli::NewAction::Notebook { path, theme } => cli::new_notebook::handle_new_notebook(&path, theme.as_deref()),
            cli::NewAction::Website { dir, theme } => cli::new_website::handle_new_website(&dir, &theme),
            cli::NewAction::Book { dir, theme } => cli::new_book::handle_new_book(&dir, &theme),
            cli::NewAction::Completions { shell } => {
                use clap::CommandFactory;
                let mut cmd = <cli::Cli as CommandFactory>::command();
                clap_complete::generate(shell, &mut cmd, "calepin", &mut std::io::stdout());
                Ok(())
            }
            cli::NewAction::Gibberish { files, paragraphs, dir, complexity } => {
                cli::new_gibberish::handle_new_gibberish(&dir, files, paragraphs, complexity)
            }
        },
        Command::Man { action } => match action {
            cli::ManAction::R { package, output, quiet } => man::handle_man_r(&package, &output, quiet),
            cli::ManAction::Python { package, output, quiet, style, exports_only, imports, include_tests, include_private } => {
                let opts = man::python::ManPythonOptions {
                    style: Some(&style),
                    exports_only,
                    include_imports: imports,
                    include_tests,
                    include_private,
                };
                man::handle_man_python(&package, &output, quiet, opts)
            }
        },
        Command::Info { action } => cli::info::handle_info(action),
    }
}
