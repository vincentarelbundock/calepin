#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[macro_use]
mod cli;
mod man;
mod types;
mod config;
mod writers;
mod engines;
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
pub(crate) use config::value;
pub(crate) use utils::paths;
pub(crate) use modules::registry;
pub(crate) use references::{bibliography, crossref};
pub(crate) use utils::util;
use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command};

/// Parse CLI args, injecting "render" as default subcommand when the first
/// positional argument looks like a file path rather than a known subcommand.
fn parse_cli() -> Cli {
    let args: Vec<String> = std::env::args().collect();

    let known = ["render", "preview", "init", "man", "extra", "templates"];

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
        Command::Init(args) => cli::init_sidecar::handle_init(args),
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
        Command::Extra { action } => cli::info::handle_extra(action),
        Command::Templates { action } => cli::templates::handle_templates(action),
    }
}
