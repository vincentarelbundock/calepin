use clap::{Parser, Subcommand};
use clap_complete::Shell;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "calepin",
    about = "Render .qmd files to HTML, LaTeX, Typst, or Markdown",
    version,
    disable_version_flag = true,
)]
#[command(arg(clap::Arg::new("version")
    .short('v')
    .long("version")
    .action(clap::ArgAction::Version)
    .help("Print version")
))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Render a .qmd file or a project .yaml manifest
    Render(RenderArgs),

    /// Preview a file, project, or directory with live-reload
    Preview(PreviewArgs),

    /// Initialize a new project
    Init {
        /// Project template: blank, docs, blog
        #[arg(long, default_value = "blank")]
        template: String,
    },

    /// Show information and utilities
    Info {
        #[command(subcommand)]
        action: InfoAction,
    },
}

#[derive(clap::Args, Debug)]
pub struct RenderArgs {
    /// Input .qmd file or .yaml/.yml project manifest
    pub input: PathBuf,

    /// Output file path (single-file only; not valid with project manifests).
    /// If omitted, replaces .qmd extension with the format's default.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Output target: a target name from calepin.toml (e.g., web, article)
    /// or a base name (html, latex, typst, markdown).
    /// If omitted, auto-detected from output extension or YAML front matter.
    #[arg(short, long)]
    pub target: Option<String>,

    /// Quiet mode (suppress progress messages)
    #[arg(short, long)]
    pub quiet: bool,

    /// Override YAML metadata fields. Accepts multiple values per flag.
    /// Example: --set title="My Title" bibliography=refs.bib toc=true
    #[arg(short = 's', long = "set", value_name = "KEY=VALUE", num_args = 1..)]
    pub overrides: Vec<String>,

    /// Remove output directory before building (project manifests only)
    #[arg(long)]
    pub clean: bool,
}

#[derive(clap::Args, Debug)]
pub struct PreviewArgs {
    /// Input .qmd file, .yaml/.yml project manifest, or directory to serve
    pub input: PathBuf,

    /// Port for the preview server
    #[arg(short, long, default_value = "3456")]
    pub port: u16,

    /// Output target: a target name or base name
    #[arg(short, long)]
    pub target: Option<String>,

    /// Override YAML metadata fields
    #[arg(short = 's', long = "set", value_name = "KEY=VALUE", num_args = 1..)]
    pub overrides: Vec<String>,

    /// Quiet mode (suppress progress messages)
    #[arg(short, long)]
    pub quiet: bool,
}

#[derive(Subcommand, Debug)]
pub enum InfoAction {
    /// List available citation styles
    Csl,
    /// List available syntax highlighting themes
    Themes,
    /// Print shell completions (bash, zsh, fish, elvish, powershell)
    Completions {
        /// Shell to generate completions for (bash, zsh, fish, elvish, powershell)
        shell: Shell,
    },
}

/// Returns true if the input path looks like a project manifest (.yaml or .yml).
pub fn is_project_manifest(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("yaml") | Some("yml")
    )
}

/// Print a yellow warning to stderr.
macro_rules! cwarn {
    ($($arg:tt)*) => {
        eprint!("\x1b[33mWarning:\x1b[0m ");
        eprintln!($($arg)*);
    };
}
