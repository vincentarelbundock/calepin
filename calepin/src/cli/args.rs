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
    arg_required_else_help = true
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
    /// Create a new example Typst file or website scaffold
    New(NewArgs),

    /// Check Calepin's local runtime environment
    Health(HealthArgs),

    /// Preprocess, then invoke typst compile
    Compile(CompileArgs),

    /// Watch, preprocess, and delegate recompiles to typst watch
    Watch(WatchArgs),

    /// Serve static files locally
    Serve(ServeArgs),

    /// Stop a running calepin watch process
    Stop(StopArgs),

    /// Remove `.calepin` directories and generated artifacts
    Clean(CleanArgs),
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileFormat {
    Pdf,
    Png,
    Svg,
    Html,
}

impl CompileFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Png => "png",
            Self::Svg => "svg",
            Self::Html => "html",
        }
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct NewArgs {
    /// Path to the new .typ file, or `website` to scaffold a website
    pub path: PathBuf,

    /// Overwrite the file if it already exists
    #[arg(short, long)]
    pub force: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct HealthArgs {
    /// Path to project config TOML
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Print machine-readable JSON
    #[arg(long)]
    pub json: bool,

    /// Exit with an error when warnings are present
    #[arg(long)]
    pub strict: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct CompileArgs {
    /// Input .typ file, or a website source directory when --config is supplied
    pub input: PathBuf,

    /// Output file path, or website output directory when INPUT is a directory
    pub output: Option<PathBuf>,

    /// Output format passed to typst compile
    #[arg(long, value_enum)]
    pub format: Option<CompileFormat>,

    /// HTML theme applied after compilation.
    ///
    /// Use `calepin-html`, `calepin-website`, or a path to a theme directory.
    #[arg(long = "theme", alias = "template")]
    pub theme: Option<String>,

    #[command(flatten)]
    pub common: CommonArgs,

    /// Arguments forwarded to typst compile after `--`
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub typst_args: Vec<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct WatchArgs {
    /// Input .typ file, or a website source directory when --config is supplied
    pub input: PathBuf,

    /// Output file path, or website output directory when INPUT is a directory
    pub output: Option<PathBuf>,

    /// Output format passed to typst watch
    #[arg(long, value_enum)]
    pub format: Option<CompileFormat>,

    /// Serve the website while watching a directory
    #[arg(long)]
    pub serve: bool,

    /// Interface to bind when serving a watched website
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to bind when serving a watched website
    #[arg(long, default_value_t = 8000)]
    pub port: u16,

    #[command(flatten)]
    pub common: CommonArgs,

    /// Arguments forwarded to typst watch after `--`
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub typst_args: Vec<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ServeArgs {
    /// Directory containing static files to serve
    pub dir: PathBuf,

    /// Interface to bind
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to bind
    #[arg(short, long, default_value_t = 8000)]
    pub port: u16,
}

#[derive(clap::Args, Debug, Clone)]
pub struct StopArgs {
    /// Input .typ file to stop the matching calepin watch.
    /// Omit this value to stop all active watches under the current project's `.calepin` directory.
    pub input: Option<PathBuf>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct CleanArgs {
    /// Maximum recursion depth when searching for `.calepin` directories
    #[arg(short, long)]
    pub depth: Option<usize>,

    /// Skip interactive confirmation and delete immediately
    #[arg(short, long)]
    pub yes: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct CommonArgs {
    /// Path to project config TOML
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Quiet mode
    #[arg(short, long)]
    pub quiet: bool,

    /// Per-chunk timeout in seconds
    #[arg(long)]
    pub timeout: Option<u64>,

    /// Override a document parameter as `key=value` (repeatable).
    ///
    /// Takes precedence over `calepin.setup(params: ...)`, so the same document
    /// can render with different values without editing the source.
    #[arg(short = 'P', long = "param", value_name = "KEY=VALUE")]
    pub params: Vec<String>,
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
    fn test_health_args() {
        let cli = Cli::try_parse_from([
            "calepin",
            "health",
            "--config",
            "config.toml",
            "--json",
            "--strict",
        ])
        .unwrap();

        match cli.command {
            Command::Health(args) => {
                assert_eq!(args.config, Some(PathBuf::from("config.toml")));
                assert!(args.json);
                assert!(args.strict);
            }
            other => panic!("expected health command, got {other:?}"),
        }
    }

    #[test]
    fn test_typst_compile_args() {
        let cli = Cli::try_parse_from([
            "calepin",
            "compile",
            "paper.typ",
            "paper.pdf",
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
                assert_eq!(
                    args.typst_args,
                    vec!["--font-path", "fonts", "--input", "theme=dark"]
                );
            }
            other => panic!("expected compile command, got {other:?}"),
        }
    }

    #[test]
    fn test_typst_compile_args_theme_name() {
        let cli = Cli::try_parse_from([
            "calepin",
            "compile",
            "paper.typ",
            "--format",
            "html",
            "--theme",
            "calepin-html",
        ])
        .unwrap();

        match cli.command {
            Command::Compile(args) => {
                assert_eq!(args.theme, Some("calepin-html".to_string()));
            }
            other => panic!("expected compile command, got {other:?}"),
        }
    }

    #[test]
    fn test_typst_compile_args_theme_user_theme_name() {
        let cli = Cli::try_parse_from([
            "calepin",
            "compile",
            "paper.typ",
            "--format",
            "html",
            "--theme",
            "zensical",
        ])
        .unwrap();

        match cli.command {
            Command::Compile(args) => {
                assert_eq!(args.theme, Some("zensical".to_string()));
            }
            other => panic!("expected compile command, got {other:?}"),
        }
    }

    #[test]
    fn test_typst_compile_args_template_alias() {
        let cli = Cli::try_parse_from([
            "calepin",
            "compile",
            "paper.typ",
            "--format",
            "html",
            "--template",
            "calepin-html",
        ])
        .unwrap();

        match cli.command {
            Command::Compile(args) => {
                assert_eq!(args.theme, Some("calepin-html".to_string()));
            }
            other => panic!("expected compile command, got {other:?}"),
        }
    }

    #[test]
    fn test_compile_param_overrides() {
        let cli = Cli::try_parse_from([
            "calepin",
            "compile",
            "paper.typ",
            "-P",
            "region=NY",
            "--param",
            "min_count=25",
        ])
        .unwrap();

        match cli.command {
            Command::Compile(args) => {
                assert_eq!(args.common.params, vec!["region=NY", "min_count=25"]);
            }
            other => panic!("expected compile command, got {other:?}"),
        }
    }

    #[test]
    fn test_watch_param_overrides() {
        let cli =
            Cli::try_parse_from(["calepin", "watch", "paper.typ", "-P", "region=CA"]).unwrap();
        match cli.command {
            Command::Watch(args) => {
                assert_eq!(args.common.params, vec!["region=CA"]);
            }
            other => panic!("expected watch command, got {other:?}"),
        }
    }

    #[test]
    fn test_typst_watch_args() {
        let cli = Cli::try_parse_from([
            "calepin",
            "watch",
            "paper.typ",
            "out/paper.html",
            "--format",
            "html",
            "--quiet",
            "--timeout",
            "42",
            "--",
            "--font-path",
            "fonts",
        ])
        .unwrap();

        match cli.command {
            Command::Watch(args) => {
                assert_eq!(args.input, PathBuf::from("paper.typ"));
                assert_eq!(args.output, Some(PathBuf::from("out/paper.html")));
                assert_eq!(args.format, Some(CompileFormat::Html));
                assert!(!args.serve);
                assert!(args.common.quiet);
                assert_eq!(args.common.timeout, Some(42));
                assert_eq!(args.typst_args, vec!["--font-path", "fonts"]);
            }
            other => panic!("expected watch command, got {other:?}"),
        }
    }

    #[test]
    fn test_watch_website_serve_args() {
        let cli = Cli::try_parse_from([
            "calepin",
            "watch",
            "docs",
            "--config",
            "website.toml",
            "--serve",
            "--host",
            "0.0.0.0",
            "--port",
            "3000",
        ])
        .unwrap();

        match cli.command {
            Command::Watch(args) => {
                assert_eq!(args.input, PathBuf::from("docs"));
                assert_eq!(args.common.config, Some(PathBuf::from("website.toml")));
                assert!(args.serve);
                assert_eq!(args.host, "0.0.0.0");
                assert_eq!(args.port, 3000);
            }
            other => panic!("expected watch command, got {other:?}"),
        }
    }

    #[test]
    fn test_serve_args() {
        let cli = Cli::try_parse_from([
            "calepin", "serve", "docs", "--host", "0.0.0.0", "--port", "3000",
        ])
        .unwrap();

        match cli.command {
            Command::Serve(args) => {
                assert_eq!(args.dir, PathBuf::from("docs"));
                assert_eq!(args.host, "0.0.0.0");
                assert_eq!(args.port, 3000);
            }
            other => panic!("expected serve command, got {other:?}"),
        }
    }

    #[test]
    fn test_typst_stop_args() {
        let cli = Cli::try_parse_from(["calepin", "stop"]).unwrap();

        match cli.command {
            Command::Stop(args) => {
                assert!(args.input.is_none());
            }
            other => panic!("expected stop command, got {other:?}"),
        }
    }

    #[test]
    fn test_typst_stop_args_with_input() {
        let cli = Cli::try_parse_from(["calepin", "stop", "paper.typ"]).unwrap();

        match cli.command {
            Command::Stop(args) => {
                assert_eq!(args.input, Some(PathBuf::from("paper.typ")));
            }
            other => panic!("expected stop command, got {other:?}"),
        }
    }

    #[test]
    fn test_clean_args_depth() {
        let cli = Cli::try_parse_from(["calepin", "clean", "--depth", "3", "--yes"]).unwrap();

        match cli.command {
            Command::Clean(args) => {
                assert_eq!(args.depth, Some(3));
                assert!(args.yes);
            }
            other => panic!("expected clean command, got {other:?}"),
        }
    }

    #[test]
    fn test_new_args() {
        let cli = Cli::try_parse_from(["calepin", "new", "paper.typ", "--force"]).unwrap();

        match cli.command {
            Command::New(args) => {
                assert_eq!(args.path, PathBuf::from("paper.typ"));
                assert!(args.force);
            }
            other => panic!("expected new command, got {other:?}"),
        }
    }

    #[test]
    fn test_new_website_args() {
        let cli = Cli::try_parse_from(["calepin", "new", "website", "--force"]).unwrap();

        match cli.command {
            Command::New(args) => {
                assert_eq!(args.path, PathBuf::from("website"));
                assert!(args.force);
            }
            other => panic!("expected new command, got {other:?}"),
        }
    }

    #[test]
    fn test_executable_path_flags_removed() {
        for flag in ["--typst", "--rscript", "--python"] {
            let err = Cli::try_parse_from(["calepin", "compile", "paper.typ", flag, "custom"])
                .unwrap_err();
            assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
        }
    }
}
