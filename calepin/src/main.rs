#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[macro_use]
mod cli;
mod config;
mod engines;
mod health;
mod html;
mod syntax_theme;
mod theme;
mod typst;
mod update;
mod utils;
mod website;

use anyhow::Result;
use clap::{error::ErrorKind, CommandFactory, Parser};

use cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            if error.kind() == ErrorKind::MissingRequiredArgument {
                eprintln!("{error}");
                if let Some(help) = current_subcommand_help() {
                    eprintln!("\nFull help:\n{help}");
                }
                std::process::exit(2);
            }
            error.exit();
        }
    };

    match cli.command {
        Command::New(args) => typst::cli::handle_new(args),
        Command::Health(args) => health::handle_health(args),
        Command::Compile(args) => typst::cli::handle_compile(args),
        Command::Watch(args) => typst::cli::handle_watch(args),
        Command::Serve(args) => website::serve(args),
        Command::Update => update::handle_update(),
        Command::Clean(args) => typst::cli::handle_clean(args),
    }
}

fn current_subcommand_help() -> Option<String> {
    let mut args = std::env::args_os();
    let _program = args.next()?;
    let subcommand = args.next()?.into_string().ok()?;
    let command = Cli::command();
    command.find_subcommand(&subcommand).map(|command| {
        let mut command = command.clone().bin_name(format!("calepin {subcommand}"));
        command.render_help().to_string()
    })
}
