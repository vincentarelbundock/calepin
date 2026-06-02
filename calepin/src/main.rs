#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[macro_use]
mod cli;
mod config;
mod engines;
mod typst;
mod types;
mod utils;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command};

fn parse_cli() -> Cli {
    Cli::parse()
}

fn main() -> Result<()> {
    let cli = parse_cli();

    match cli.command {
        Command::Preprocess(args) => typst::cli::handle_preprocess(args),
        Command::Compile(args) => typst::cli::handle_compile(args),
    }
}
