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

    /// Watch file or project and live-reload on changes
    Preview(PreviewArgs),

    /// Initialize a new project
    Init {
        /// Project template: blank, docs, blog
        #[arg(long, default_value = "blank")]
        template: String,
    },

    /// Plugin management
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },

    /// Syntax highlighting utilities
    Highlight {
        #[command(subcommand)]
        action: HighlightAction,
    },

    /// Print shell completions
    Completions {
        /// Shell to generate completions for
        shell: Shell,
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

    /// Compile output to PDF (LaTeX via tectonic, Typst via typst)
    #[arg(long)]
    pub pdf: bool,

    /// Remove output directory before building (project manifests only)
    #[arg(long)]
    pub clean: bool,
}

#[derive(clap::Args, Debug)]
pub struct PreviewArgs {
    /// Input .qmd file or .yaml/.yml project manifest
    pub input: PathBuf,

    /// Port for the preview server
    #[arg(short, long, default_value = "3456")]
    pub port: u16,

    /// Output format: html, latex, typst, markdown
    #[arg(short, long)]
    pub format: Option<String>,

    /// Override YAML metadata fields
    #[arg(short = 's', long = "set", value_name = "KEY=VALUE", num_args = 1..)]
    pub overrides: Vec<String>,

    /// Quiet mode (suppress progress messages)
    #[arg(short, long)]
    pub quiet: bool,
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

#[derive(Subcommand, Debug)]
pub enum HighlightAction {
    /// List available syntax highlighting themes
    List,
    /// Preview a highlighting theme on a sample
    Preview {
        /// Theme name
        theme: String,
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
