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
    arg_required_else_help = true,
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
        path: Option<PathBuf>,

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

    /// Initialize projects, notebooks, and sidecars
    Init {
        #[command(subcommand)]
        action: InitAction,
    },

    /// Extract package documentation as .qmd files
    Man {
        #[command(subcommand)]
        action: ManAction,
    },

    /// Show information and utilities
    Extra {
        #[command(subcommand)]
        action: ExtraAction,
    },

    /// Manage templates (list, eject, diff, update, reset)
    Templates {
        #[command(subcommand)]
        action: TemplatesAction,
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

    /// Output format: a target name from sidecar config.toml (e.g., web, article)
    /// or a base name (html, latex, typst, slides, website, markdown).
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

    /// Generate page-relative URLs for file:// browsing (no server needed)
    #[arg(long)]
    pub portable: bool,
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
#[command(arg_required_else_help = true)]
pub enum InitAction {
    /// Create a new project or document from a scaffold
    #[command(after_help = "\
\x1B[1;4mExamples:\x1B[0m
  calepin init new my-site                          # website (default)
  calepin init new my-site --scaffold book          # book project
  calepin init new handout.qmd                      # single document
  calepin init new my-site --extension ./tufte      # from extension scaffold")]
    New {
        /// Project directory or .qmd file path
        #[arg(default_value = "website")]
        path: std::path::PathBuf,

        /// Scaffold name (default: inferred from path -- "website" for directories, "document" for .qmd files)
        #[arg(long)]
        scaffold: Option<String>,

        /// Use scaffold from an extension directory
        #[arg(long)]
        extension: Option<String>,
    },

    /// Scaffold a new extension
    Extension {
        /// Extension name (creates a directory with this name)
        name: String,

        /// Parent target to inherit from (e.g., html, latex, website)
        #[arg(long, default_value = "html")]
        inherits: String,
    },

    /// Extract a sidecar directory from an existing .qmd document
    Sidecar {
        /// Path to the existing .qmd file
        path: std::path::PathBuf,

        /// Overwrite an existing sidecar directory
        #[arg(long)]
        force: bool,

        /// Also scaffold built-in templates into the sidecar
        #[arg(long)]
        templates: bool,

        /// Target extension to apply to the new sidecar
        #[arg(long)]
        target: Option<String>,

        /// Print what would happen without writing anything
        #[arg(long)]
        dry_run: bool,

        /// Skip creating the .qmd.bak backup file
        #[arg(long)]
        no_backup: bool,
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
#[command(arg_required_else_help = true)]
pub enum ManAction {
    /// Extract R package documentation
    R {
        /// Package name or path to source directory
        package: String,

        /// Output directory for .qmd files
        #[arg(short, long, default_value = "man")]
        output: PathBuf,

        /// Quiet mode
        #[arg(short, long)]
        quiet: bool,
    },

    /// Extract Python package documentation
    Python {
        /// Package name or path to source directory
        package: String,

        /// Output directory for .qmd files
        #[arg(short, long, default_value = "man")]
        output: PathBuf,

        /// Quiet mode
        #[arg(short, long)]
        quiet: bool,

        /// Docstring style: google, numpy, sphinx, markdown, or auto (default: auto)
        #[arg(long, default_value = "auto")]
        style: String,

        /// Only include names exported in __all__
        #[arg(long, name = "all")]
        exports_only: bool,

        /// Also include names re-exported via __init__.py imports
        #[arg(long)]
        imports: bool,

        /// Include test modules and directories
        #[arg(long)]
        include_tests: bool,

        /// Include private (_-prefixed) modules and directories
        #[arg(long)]
        include_private: bool,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum ExtraAction {
    /// List available citation styles
    Csl,
    /// List available syntax highlighting themes
    Highlight,
    /// Print shell completions
    #[command(arg_required_else_help = true, after_help = "\
\x1B[1;4mExamples:\x1B[0m
  calepin extra completions zsh  > ~/.zfunc/_calepin
  calepin extra completions bash > ~/.local/share/bash-completion/completions/calepin
  calepin extra completions fish > ~/.config/fish/completions/calepin.fish")]
    Completions {
        /// Shell to generate completions for (bash, zsh, fish, elvish, powershell)
        shell: Shell,
    },

    /// Install the calepin agent skill for coding assistants (Claude, Codex, opencode, pi)
    #[command(after_help = "\
\x1B[1;4mExamples:\x1B[0m
  calepin extra skill                    # install for all tools (personal)
  calepin extra skill --project          # install into current project
  calepin extra skill --claude --codex   # specific tools only")]
    Skill {
        /// Install to current project directory instead of personal home directory
        #[arg(long)]
        project: bool,

        /// Install for Claude Code
        #[arg(long)]
        claude: bool,

        /// Install for OpenAI Codex CLI
        #[arg(long)]
        codex: bool,

        /// Install for opencode
        #[arg(long)]
        opencode: bool,

        /// Install for pi
        #[arg(long)]
        pi: bool,

        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
#[command(arg_required_else_help = true)]
pub enum TemplatesAction {
    /// Show all templates with their resolution source
    List {
        /// Input .qmd file (determines the sidecar)
        input: PathBuf,

        /// Override the target (default: read from front matter)
        #[arg(short = 't', long)]
        target: Option<String>,
    },

    /// Eject built-in templates into the sidecar for customization
    Eject {
        /// Input .qmd file (determines the sidecar)
        input: PathBuf,

        /// Override the target (default: read from front matter)
        #[arg(short = 't', long)]
        target: Option<String>,

        /// Overwrite existing local templates
        #[arg(long)]
        force: bool,

        /// Show what would be written without writing anything
        #[arg(long)]
        dry_run: bool,
    },

    /// Print a template's resolved content
    Show {
        /// Template name (e.g., figure, code_source, page)
        name: String,

        /// Input .qmd file (determines the sidecar)
        input: PathBuf,

        /// Override the target (default: read from front matter)
        #[arg(short = 't', long)]
        target: Option<String>,
    },

    /// Compare local template overrides against built-in defaults
    Diff {
        /// Input .qmd file (determines the sidecar)
        input: PathBuf,

        /// Override the target (default: read from front matter)
        #[arg(short = 't', long)]
        target: Option<String>,
    },

    /// Refresh unmodified templates after a Calepin upgrade
    Update {
        /// Input .qmd file (determines the sidecar)
        input: PathBuf,

        /// Override the target (default: read from front matter)
        #[arg(short = 't', long)]
        target: Option<String>,

        /// Overwrite even modified templates
        #[arg(long)]
        force: bool,

        /// Show what would be changed without writing anything
        #[arg(long)]
        dry_run: bool,
    },

    /// Remove a local template override, reverting to built-in
    Reset {
        /// Template name (e.g., figure, code_source, page)
        name: String,

        /// Input .qmd file (determines the sidecar)
        input: PathBuf,

        /// Override the target (default: read from front matter)
        #[arg(short = 't', long)]
        target: Option<String>,
    },
}

/// Find the project config file in a directory.
/// Checks `{stem}_calepin/config.toml` for each .qmd file, then falls back
/// to legacy `_calepin/config.toml`.
pub fn find_project_config(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    // New convention: look for any {stem}_calepin/config.toml
    // Prefer index_calepin/config.toml as the canonical entry point
    let index_sidecar = dir.join("index_calepin").join("config.toml");
    if index_sidecar.exists() {
        return index_sidecar.canonicalize().ok()
            .or_else(|| Some(crate::paths::normalize_path(&index_sidecar)));
    }

    // Legacy fallback: _calepin/config.toml
    let legacy = crate::paths::calepin_dir(dir, &[]).join("config.toml");
    if legacy.exists() {
        return legacy.canonicalize().ok()
            .or_else(|| Some(crate::paths::normalize_path(&legacy)));
    }

    None
}

/// Print a yellow warning to stderr.
macro_rules! cwarn {
    ($($arg:tt)*) => {
        eprint!("\x1b[33mWarning:\x1b[0m ");
        eprintln!($($arg)*);
    };
}
