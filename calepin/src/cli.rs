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
    /// Render a .qmd file to HTML, LaTeX, Typst, or Markdown
    Render(RenderArgs),

    /// Watch file and live-reload on changes
    Preview(PreviewArgs),

    /// Static site operations
    Site {
        #[command(subcommand)]
        action: SiteAction,
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

    /// Compile output to PDF (LaTeX via tectonic, Typst via typst)
    #[arg(long)]
    pub pdf: bool,

    /// Render multiple files in parallel from a JSON manifest.
    /// Pass a file path or "-" to read from stdin.
    #[arg(long, value_name = "MANIFEST")]
    pub batch: Option<String>,

    /// With --batch: emit rendered bodies in JSON stdout instead of writing files
    #[arg(long)]
    pub stdout: bool,
}

#[derive(clap::Args, Debug)]
pub struct PreviewArgs {
    /// Input .qmd file path
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
pub enum SiteAction {
    /// Build a static site from .qmd files
    Build {
        /// Path to site config file (_calepin.yaml)
        #[arg(short, long, value_name = "PATH")]
        config: Option<PathBuf>,

        /// Output directory
        #[arg(short, long, default_value = "_site")]
        output: PathBuf,

        /// Remove output directory before building
        #[arg(long)]
        clean: bool,

        /// Quiet mode
        #[arg(short, long)]
        quiet: bool,
    },

    /// Initialize a new site project
    Init {
        /// Site template: blank, docs, blog
        #[arg(long, default_value = "blank")]
        template: String,
    },

    /// Preview the site with live-reload
    Preview {
        /// Path to site config file
        #[arg(short, long, value_name = "PATH")]
        config: Option<PathBuf>,

        /// Port for the preview server
        #[arg(short, long, default_value = "3456")]
        port: u16,
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

/// Print a yellow warning to stderr.
macro_rules! cwarn {
    ($($arg:tt)*) => {
        eprint!("\x1b[33mWarning:\x1b[0m ");
        eprintln!($($arg)*);
    };
}
