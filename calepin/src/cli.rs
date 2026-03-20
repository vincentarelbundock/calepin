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
    /// Input .qmd file path
    pub input: Option<PathBuf>,

    /// Output file path (e.g., output.html, output.tex, output.typ, output.md).
    /// If omitted, replaces .qmd extension with the format's default.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Output format: html, latex, typst, markdown.
    /// If omitted, auto-detected from output extension or YAML front matter.
    #[arg(short, long)]
    pub format: Option<String>,

    /// Quiet mode (suppress progress messages)
    #[arg(short, long)]
    pub quiet: bool,

    /// Override YAML metadata fields. Accepts multiple values per flag.
    /// Example: --set title="My Title" bibliography=refs.bib toc=true
    #[arg(short = 's', long = "set", value_name = "KEY=VALUE", num_args = 1..)]
    pub overrides: Vec<String>,

    /// Print shell completions and exit
    #[arg(long, value_name = "SHELL")]
    pub completions: Option<Shell>,

    /// Watch file, serve HTML, and live-reload on changes
    #[arg(long)]
    pub preview: bool,

    /// Port for the preview server
    #[arg(long, default_value = "3456")]
    pub port: u16,

    /// Compile output to PDF (LaTeX via tectonic, Typst via typst)
    #[arg(short, long)]
    pub compile: bool,

    /// Render multiple files in parallel from a JSON manifest.
    /// Pass a file path or "-" to read from stdin.
    #[arg(long, value_name = "MANIFEST")]
    pub batch: Option<String>,

    /// With --batch: emit rendered bodies in JSON stdout instead of writing files
    #[arg(long)]
    pub batch_stdout: bool,

    /// List available syntax highlighting themes and exit
    #[arg(long)]
    pub list_highlight_styles: bool,

    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

#[derive(Subcommand, Debug)]
pub enum CliCommand {
    /// Plugin management
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum PluginAction {
    /// Create a new plugin scaffold
    Init {
        /// Plugin name
        name: String,
    },
    /// List all available plugins
    List,
}

/// Print a yellow warning to stderr.
macro_rules! cwarn {
    ($($arg:tt)*) => {
        eprint!("\x1b[33mWarning:\x1b[0m ");
        eprintln!($($arg)*);
    };
}
