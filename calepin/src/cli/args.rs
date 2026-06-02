use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// Global quiet flag, set once from CLI args and readable anywhere.
pub static QUIET: AtomicBool = AtomicBool::new(false);

pub fn set_quiet(q: bool) {
    QUIET.store(q, Ordering::Relaxed);
}

#[derive(Parser, Debug)]
#[command(
    name = "calepin",
    about = "Preprocess Typst documents with executable code chunks",
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
    /// Execute chunks and write Typst-readable result artifacts
    Preprocess(PreprocessArgs),

    /// Preprocess, then invoke typst compile
    Compile(CompileArgs),
}

#[derive(clap::Args, Debug, Clone)]
pub struct PreprocessArgs {
    /// Input .typ file
    pub input: PathBuf,

    /// Typst project root. Defaults to the input file directory
    #[arg(long)]
    pub root: Option<PathBuf>,

    /// Execution working directory. Defaults to the input file directory
    #[arg(long)]
    pub cwd: Option<PathBuf>,

    /// Override results JSON path
    #[arg(long)]
    pub results: Option<PathBuf>,

    /// Force chunk caching on
    #[arg(long = "cache", action = clap::ArgAction::SetTrue, conflicts_with = "no_cache")]
    pub cache: bool,

    /// Force chunk caching off
    #[arg(long = "no-cache", action = clap::ArgAction::SetTrue)]
    pub no_cache: bool,

    /// Force chunk execution on
    #[arg(long = "execute", action = clap::ArgAction::SetTrue, conflicts_with = "no_execute")]
    pub execute: bool,

    /// Force chunk execution off
    #[arg(long = "no-execute", action = clap::ArgAction::SetTrue)]
    pub no_execute: bool,

    /// Remove generated results and figures before preprocessing
    #[arg(long)]
    pub clean: bool,

    /// Quiet mode
    #[arg(short, long)]
    pub quiet: bool,

    /// Path to Typst executable
    #[arg(long, default_value = "typst")]
    pub typst: PathBuf,

    /// Path to Rscript executable
    #[arg(long, default_value = "Rscript")]
    pub rscript: PathBuf,

    /// Path to Python executable
    #[arg(long, default_value = "python3")]
    pub python: PathBuf,

    /// Path to shell executable
    #[arg(long, default_value = "/bin/sh")]
    pub shell: PathBuf,

    /// Per-chunk timeout in seconds
    #[arg(long)]
    pub timeout: Option<u64>,
}

impl PreprocessArgs {
    pub fn cache_override(&self) -> Option<bool> {
        if self.cache {
            Some(true)
        } else if self.no_cache {
            Some(false)
        } else {
            None
        }
    }

    pub fn execute_override(&self) -> Option<bool> {
        if self.execute {
            Some(true)
        } else if self.no_execute {
            Some(false)
        } else {
            None
        }
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct CompileArgs {
    /// Input .typ file
    pub input: PathBuf,

    /// Output path passed to typst compile
    pub output: Option<PathBuf>,

    #[command(flatten)]
    pub common: CommonArgs,

    /// Arguments forwarded to typst compile after `--`
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub typst_args: Vec<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct CommonArgs {
    /// Typst project root. Defaults to the input file directory
    #[arg(long)]
    pub root: Option<PathBuf>,

    /// Execution working directory. Defaults to the input file directory
    #[arg(long)]
    pub cwd: Option<PathBuf>,

    /// Override results JSON path
    #[arg(long)]
    pub results: Option<PathBuf>,

    /// Force chunk caching on
    #[arg(long = "cache", action = clap::ArgAction::SetTrue, conflicts_with = "no_cache")]
    pub cache: bool,

    /// Force chunk caching off
    #[arg(long = "no-cache", action = clap::ArgAction::SetTrue)]
    pub no_cache: bool,

    /// Force chunk execution on
    #[arg(long = "execute", action = clap::ArgAction::SetTrue, conflicts_with = "no_execute")]
    pub execute: bool,

    /// Force chunk execution off
    #[arg(long = "no-execute", action = clap::ArgAction::SetTrue)]
    pub no_execute: bool,

    /// Remove generated results and figures before preprocessing
    #[arg(long)]
    pub clean: bool,

    /// Quiet mode
    #[arg(short, long)]
    pub quiet: bool,

    /// Path to Typst executable
    #[arg(long, default_value = "typst")]
    pub typst: PathBuf,

    /// Path to Rscript executable
    #[arg(long, default_value = "Rscript")]
    pub rscript: PathBuf,

    /// Path to Python executable
    #[arg(long, default_value = "python3")]
    pub python: PathBuf,

    /// Path to shell executable
    #[arg(long, default_value = "/bin/sh")]
    pub shell: PathBuf,

    /// Per-chunk timeout in seconds
    #[arg(long)]
    pub timeout: Option<u64>,
}

impl CommonArgs {
    pub fn cache_override(&self) -> Option<bool> {
        if self.cache {
            Some(true)
        } else if self.no_cache {
            Some(false)
        } else {
            None
        }
    }

    pub fn execute_override(&self) -> Option<bool> {
        if self.execute {
            Some(true)
        } else if self.no_execute {
            Some(false)
        } else {
            None
        }
    }
}

/// Print a yellow warning to stderr.
macro_rules! cwarn {
    ($($arg:tt)*) => {
        eprint!("\x1b[33mWarning:\x1b[0m ");
        eprintln!($($arg)*);
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_typst_preprocess_args() {
        let cli = Cli::try_parse_from([
            "calepin",
            "preprocess",
            "paper.typ",
            "--root",
            "project",
            "--cwd",
            "work",
            "--results",
            "out/results.json",
            "--no-cache",
            "--no-execute",
            "--clean",
            "--quiet",
            "--typst",
            "typst-dev",
            "--rscript",
            "Rscript-dev",
            "--python",
            "python-dev",
            "--shell",
            "/bin/bash",
            "--timeout",
            "42",
        ])
        .unwrap();

        match cli.command {
            Command::Preprocess(args) => {
                assert_eq!(args.input, PathBuf::from("paper.typ"));
                assert_eq!(args.root, Some(PathBuf::from("project")));
                assert_eq!(args.cwd, Some(PathBuf::from("work")));
                assert_eq!(args.results, Some(PathBuf::from("out/results.json")));
                assert_eq!(args.cache_override(), Some(false));
                assert_eq!(args.execute_override(), Some(false));
                assert!(args.clean);
                assert!(args.quiet);
                assert_eq!(args.typst, PathBuf::from("typst-dev"));
                assert_eq!(args.rscript, PathBuf::from("Rscript-dev"));
                assert_eq!(args.python, PathBuf::from("python-dev"));
                assert_eq!(args.shell, PathBuf::from("/bin/bash"));
                assert_eq!(args.timeout, Some(42));
            }
            other => panic!("expected preprocess command, got {other:?}"),
        }
    }

    #[test]
    fn test_typst_compile_args() {
        let cli = Cli::try_parse_from([
            "calepin",
            "compile",
            "paper.typ",
            "paper.pdf",
            "--root",
            "project",
            "--",
            "--font-path",
            "fonts",
            "--input",
            "theme=dark",
        ])
        .unwrap();

        match cli.command {
            Command::Compile(args) => {
                assert_eq!(args.input, PathBuf::from("paper.typ"));
                assert_eq!(args.output, Some(PathBuf::from("paper.pdf")));
                assert_eq!(args.common.root, Some(PathBuf::from("project")));
                assert_eq!(args.typst_args, vec!["--font-path", "fonts", "--input", "theme=dark"]);
            }
            other => panic!("expected compile command, got {other:?}"),
        }
    }

    #[test]
    fn test_old_subcommands_removed() {
        for command in ["render", "preview", "init", "man", "extra", "templates"] {
            let err = Cli::try_parse_from(["calepin", command]).unwrap_err();
            assert_eq!(err.kind(), clap::error::ErrorKind::InvalidSubcommand);
        }
    }
}
