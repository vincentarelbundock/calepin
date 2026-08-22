use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use calepin_docs::{generate, Package};

/// Generate Typst API reference pages from a Python package.
#[derive(Parser)]
#[command(name = "calepin-docs", version, about)]
struct Cli {
    /// Package directory (containing `__init__.py`) or a single `.py` file.
    package: PathBuf,

    /// Directory to write the generated `.typ` files into.
    #[arg(short, long, default_value = "reference")]
    out: PathBuf,

    /// Print what would be written without touching the filesystem.
    #[arg(long)]
    dry_run: bool,

    /// Emit Calepin website metadata so the pages join a site's navigation.
    #[arg(long)]
    website: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let package = Package::open(&cli.package)?;

    if cli.dry_run {
        let resolution = package.resolve()?;
        for item in &resolution.items {
            println!("{}.typ", item.qualname());
        }
        report_unresolved(&resolution.unresolved);
        println!("{} definitions", resolution.items.len());
        return Ok(());
    }

    let report = generate(&package, &cli.out, cli.website)?;

    if report.template_written {
        println!(
            "wrote {}/api.typ (template — edit freely, never overwritten)",
            cli.out.display()
        );
    }
    println!(
        "wrote {} files to {}",
        report.written.len(),
        cli.out.display()
    );
    report_unresolved(&report.unresolved);

    Ok(())
}

fn report_unresolved(unresolved: &[calepin_docs::resolve::Unresolved]) {
    if unresolved.is_empty() {
        return;
    }
    eprintln!(
        "\n{} name(s) in __all__ could not be resolved:",
        unresolved.len()
    );
    for item in unresolved {
        eprintln!("  {} — {}", item.name, item.reason);
    }
}
