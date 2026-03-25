use clap::{Parser, Subcommand};
use clap_complete::Shell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// Global quiet flag, set once from CLI args and readable anywhere.
pub static QUIET: AtomicBool = AtomicBool::new(false);

pub fn set_quiet(q: bool) { QUIET.store(q, Ordering::Relaxed); }
pub fn is_quiet() -> bool { QUIET.load(Ordering::Relaxed) }

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

    /// Delete generated files (_calepin/cache/, _calepin/files/, and LaTeX artefacts).
    /// Pass a stem name (e.g., "index") to flush only that page's cache/files.
    Flush {
        /// Directory to clean, or a stem name to flush selectively
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,

        /// Delete only _calepin/cache/ directories
        #[arg(long)]
        cache: bool,

        /// Delete only _calepin/files/ directories
        #[arg(long)]
        files: bool,

        /// Delete only LaTeX compilation artefacts (.aux, .log, etc.)
        #[arg(long)]
        compilation: bool,

        /// Delete everything (default when no category flag is given)
        #[arg(long)]
        all: bool,
    },

    /// Generate scaffolding files
    New {
        #[command(subcommand)]
        action: NewAction,
    },

    /// Show information and utilities
    Info {
        #[command(subcommand)]
        action: InfoAction,
    },
}

#[derive(clap::Args, Debug)]
pub struct RenderArgs {
    /// Input .qmd file(s) or .yaml/.yml project manifest.
    /// Multiple files are rendered in parallel.
    #[arg(required = true)]
    pub input: Vec<PathBuf>,

    /// Output path. With a single input, specifies the output file.
    /// With multiple inputs, specifies the output directory.
    /// If omitted, output goes next to each input file.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Output target: a target name from _calepin.toml (e.g., web, article)
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

    /// Disable syntax highlighting for code blocks
    #[arg(long)]
    pub no_highlight: bool,

    /// Override the engine for compound targets (pdf, book).
    /// Allowed values depend on the target: pdf accepts html/latex/typst/markdown,
    /// book accepts latex/typst.
    #[arg(long, value_parser = ["html", "latex", "typst", "markdown"])]
    pub engine: Option<String>,

    /// Theme to apply on top of the target.
    #[arg(long)]
    pub theme: Option<String>,

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
pub enum NewAction {
    /// Generate .qmd files filled with lorem ipsum text
    Gibberish {
        /// Number of .qmd files to generate
        #[arg(short = 'n', long, default_value = "50")]
        files: usize,

        /// Number of paragraphs per file
        #[arg(short, long, default_value = "50")]
        paragraphs: usize,

        /// Output directory
        #[arg(short, long, default_value = "gibberish")]
        dir: std::path::PathBuf,

        /// Complexity level: 0 = prose only, 1 = + code chunks,
        /// 2 = + cross-references, footnotes, citations, and tables
        #[arg(short, long, default_value = "1", value_parser = clap::value_parser!(u8).range(0..=2))]
        complexity: u8,
    },
}

#[derive(Subcommand, Debug)]
pub enum InfoAction {
    /// List available citation styles
    Csl,
    /// List available syntax highlighting themes
    Themes,
    /// List available document themes
    ThemeList,
    /// Print shell completions (bash, zsh, fish, elvish, powershell)
    Completions {
        /// Shell to generate completions for (bash, zsh, fish, elvish, powershell)
        shell: Shell,
    },
}

/// Returns true if the input is a collection config file (_calepin.toml).
pub fn is_collection_config(path: &std::path::Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name == "_calepin.toml"
}

/// Print a yellow warning to stderr.
macro_rules! cwarn {
    ($($arg:tt)*) => {
        eprint!("\x1b[33mWarning:\x1b[0m ");
        eprintln!($($arg)*);
    };
}
