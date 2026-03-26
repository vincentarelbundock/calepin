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

    /// Delete cache, generated files, and build artefacts
    Flush {
        /// Directory or stem name (e.g., "index") to flush selectively
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Skip confirmation
        #[arg(short = 'y', long)]
        yes: bool,

        /// Cache only
        #[arg(long)]
        cache: bool,

        /// Generated files only
        #[arg(long)]
        files: bool,

        /// LaTeX artefacts only (.aux, .log, etc.)
        #[arg(long)]
        compilation: bool,

        /// Everything (default)
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

    /// Output format: a format name from _calepin/config.toml (e.g., web, article)
    /// or a base name (html, latex, typst, revealjs, website, markdown).
    /// If omitted, auto-detected from output extension or YAML front matter.
    #[arg(short = 't', long)]
    pub format: Option<String>,

    /// Quiet mode (suppress progress messages)
    #[arg(short, long)]
    pub quiet: bool,

    /// Override YAML metadata fields. Accepts multiple values per flag.
    /// Example: --set title="My Title" bibliography=refs.bib toc=true
    #[arg(short = 's', long = "set", value_name = "KEY=VALUE", num_args = 1..)]
    pub overrides: Vec<String>,

    /// Compile output to PDF (for typst and latex formats)
    #[arg(long)]
    pub compile: bool,

    /// Disable syntax highlighting for code blocks
    #[arg(long)]
    pub no_highlight: bool,

    /// Override the writer for compound targets (pdf, book).
    /// Allowed values depend on the target: pdf accepts html/latex/typst/markdown,
    /// book accepts latex/typst.
    #[arg(long, value_parser = ["html", "latex", "typst", "markdown"])]
    pub writer: Option<String>,

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

    /// Output format: a format name or base name
    #[arg(short = 't', long)]
    pub format: Option<String>,

    /// Override YAML metadata fields
    #[arg(short = 's', long = "set", value_name = "KEY=VALUE", num_args = 1..)]
    pub overrides: Vec<String>,

    /// Quiet mode (suppress progress messages)
    #[arg(short, long)]
    pub quiet: bool,
}

#[derive(Subcommand, Debug)]
pub enum NewAction {
    /// Scaffold a .qmd notebook with its sidecar directory
    Notebook {
        /// Path for the new .qmd file
        #[arg(default_value = "my_calepin_notebook.qmd")]
        path: std::path::PathBuf,
    },

    /// Scaffold a website project
    Website {
        /// Directory name for the new website
        #[arg(default_value = "my_calepin_website")]
        dir: std::path::PathBuf,
    },

    /// Scaffold a book project
    Book {
        /// Directory name for the new book
        #[arg(default_value = "my_calepin_book")]
        dir: std::path::PathBuf,
    },

    /// Print shell completions (bash, zsh, fish, elvish, powershell)
    Completions {
        /// Shell to generate completions for (bash, zsh, fish, elvish, powershell)
        shell: Shell,
    },

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
}

/// Returns true if the input is a collection config file (_calepin/config.toml).
pub fn is_collection_config(path: &std::path::Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name == "config.toml" {
        if let Some(parent) = path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()) {
            return parent == "_calepin";
        }
    }
    false
}

/// Find the project config file in a directory.
/// Checks `_calepin/config.toml`.
pub fn find_project_config(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let path = dir.join("_calepin").join("config.toml");
    if path.exists() { Some(path) } else { None }
}

/// Print a yellow warning to stderr.
macro_rules! cwarn {
    ($($arg:tt)*) => {
        eprint!("\x1b[33mWarning:\x1b[0m ");
        eprintln!($($arg)*);
    };
}
