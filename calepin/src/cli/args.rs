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
    /// Create a new example Typst file
    New(NewArgs),

    /// Preprocess, then invoke typst compile
    Compile(CompileArgs),

    /// Watch, preprocess, and delegate recompiles to typst watch
    Watch(WatchArgs),

    /// Stop a running calepin watch process
    Stop(StopArgs),
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

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompileOutputTemplate {
    /// Extract only `<head>` styles and `<body>` HTML for markdown embedding.
    HtmlInMd,
    /// Render HTML using the built-in `basic` theme.
    Basic,
    /// Render HTML using the built-in `pico` theme.
    Pico,
}

impl CompileOutputTemplate {
    pub fn html_template_name(self) -> Option<&'static str> {
        match self {
            Self::Basic => Some("basic"),
            Self::Pico => Some("pico"),
            Self::HtmlInMd => None,
        }
    }

    pub fn is_html_in_markdown(&self) -> bool {
        matches!(self, Self::HtmlInMd)
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct NewArgs {
    /// Path to the new .typ file
    pub path: PathBuf,

    /// Overwrite the file if it already exists
    #[arg(short, long)]
    pub force: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct CompileArgs {
    /// Input .typ file
    pub input: PathBuf,

    /// Output path passed to typst compile
    pub output: Option<PathBuf>,

    /// Output format passed to typst compile
    #[arg(long, value_enum)]
    pub format: Option<CompileFormat>,

    /// Output template applied after compilation.
    ///
    /// Use `html-in-md` to embed only CSS and body content for markdown pages.
    /// Use `basic` or `pico` to apply a built-in HTML theme.
    #[arg(long, value_enum)]
    pub template: Option<CompileOutputTemplate>,

    #[command(flatten)]
    pub common: CommonArgs,

    /// Arguments forwarded to typst compile after `--`
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub typst_args: Vec<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct WatchArgs {
    /// Input .typ file
    pub input: PathBuf,

    /// Output path passed to typst watch
    pub output: Option<PathBuf>,

    /// Output format passed to typst watch
    #[arg(long, value_enum)]
    pub format: Option<CompileFormat>,

    #[command(flatten)]
    pub common: CommonArgs,

    /// Arguments forwarded to typst watch after `--`
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub typst_args: Vec<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct StopArgs {
    /// Input .typ file to stop the matching calepin watch.
    /// Omit this value to stop all active watches under the current project's `.calepin` directory.
    pub input: Option<PathBuf>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct CommonArgs {
    /// Override results JSON path
    #[arg(long)]
    pub results: Option<PathBuf>,

    /// Remove generated results and figures before preprocessing
    #[arg(long)]
    pub clean: bool,

    /// Quiet mode
    #[arg(short, long)]
    pub quiet: bool,

    /// Per-chunk timeout in seconds
    #[arg(long)]
    pub timeout: Option<u64>,
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
    fn test_typst_compile_args_template() {
        let cli = Cli::try_parse_from([
            "calepin",
            "compile",
            "paper.typ",
            "--format",
            "html",
            "--template",
            "html-in-md",
        ])
        .unwrap();

        match cli.command {
            Command::Compile(args) => {
                assert_eq!(args.template, Some(CompileOutputTemplate::HtmlInMd));
            }
            other => panic!("expected compile command, got {other:?}"),
        }
    }

    #[test]
    fn test_typst_compile_args_template_theme_name() {
        let cli = Cli::try_parse_from([
            "calepin",
            "compile",
            "paper.typ",
            "--format",
            "html",
            "--template",
            "pico",
        ])
        .unwrap();

        match cli.command {
            Command::Compile(args) => {
                assert_eq!(args.template, Some(CompileOutputTemplate::Pico));
                assert_eq!(args.template.unwrap().html_template_name(), Some("pico"));
            }
            other => panic!("expected compile command, got {other:?}"),
        }
    }

    #[test]
    fn test_typst_compile_args_template_basic() {
        let cli = Cli::try_parse_from([
            "calepin",
            "compile",
            "paper.typ",
            "--format",
            "html",
            "--template",
            "basic",
        ])
        .unwrap();

        match cli.command {
            Command::Compile(args) => {
                assert_eq!(args.template, Some(CompileOutputTemplate::Basic));
                assert_eq!(args.template.unwrap().html_template_name(), Some("basic"));
            }
            other => panic!("expected compile command, got {other:?}"),
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
            "--results",
            "out/results.json",
            "--clean",
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
                assert_eq!(args.common.results, Some(PathBuf::from("out/results.json")));
                assert!(args.common.clean);
                assert!(args.common.quiet);
                assert_eq!(args.common.timeout, Some(42));
                assert_eq!(args.typst_args, vec!["--font-path", "fonts"]);
            }
            other => panic!("expected watch command, got {other:?}"),
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
    fn test_executable_path_flags_removed() {
        for flag in ["--typst", "--rscript", "--python", "--julia", "--shell"] {
            let err = Cli::try_parse_from(["calepin", "compile", "paper.typ", flag, "custom"])
                .unwrap_err();
            assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
        }
    }
}
