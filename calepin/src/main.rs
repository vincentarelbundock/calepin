#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[macro_use]
mod cli;
mod config;
mod engines;
mod health;
mod html;
mod theme;
mod typst;
mod utils;
mod website;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command};

fn parse_cli() -> Cli {
    Cli::parse()
}

fn main() -> Result<()> {
    let cli = parse_cli();

    match cli.command {
        Command::New(args) => typst::cli::handle_new(args),
        Command::Health(args) => health::handle_health(args),
        Command::Compile(args) => typst::cli::handle_compile(args),
        Command::Watch(args) => typst::cli::handle_watch(args),
        Command::Serve(args) => website::serve(args),
        Command::Stop(args) => typst::cli::handle_stop(args),
        Command::Clean(args) => typst::cli::handle_clean(args),
    }
}
