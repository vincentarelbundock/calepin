use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::cli::{is_quiet, set_quiet, CleanArgs, CompileArgs, NewArgs, NewTheme, WatchArgs};
use crate::html::SiteContextInput;
use crate::typst::compile::{
    compile_with_typst, resolve_output_format, CompileOptions, OutputFormat,
};
use crate::typst::preprocess::{preprocess_cached, PreprocessOptions};

const NEW_FILE_TEMPLATE: &str = include_str!("../assets/scaffolds/notebook/notebook.typ");

pub fn handle_new(args: NewArgs) -> Result<()> {
    let theme = args.theme.unwrap_or(NewTheme::Calepin).as_str();
    if args.path == Path::new("theme") {
        let dest = match args.output.as_deref() {
            Some(output) => crate::theme::eject_builtin_to(theme, output, args.force)?,
            None => crate::theme::eject_builtin_to(theme, default_theme_dir(), args.force)?,
        };
        if !is_quiet() {
            eprintln!("Created {}", dest.display());
            eprintln!(
                "Select it with `theme = \"{}\"` in calepin.toml",
                dest.display()
            );
        }
        return Ok(());
    }
    if args.path == Path::new("website") {
        let dest = match args.output.as_deref() {
            Some(output) => output,
            None => default_website_dir(),
        };
        crate::website::scaffold_website(dest, theme, args.force)?;
        if !is_quiet() {
            eprintln!("Created {theme} website scaffold in {}", dest.display());
        }
        return Ok(());
    }

    if args.output.is_some() {
        return Err(anyhow::anyhow!(
            "an output path only applies to `calepin new website` or `calepin new theme`"
        ));
    }

    if args.theme.is_some() {
        return Err(anyhow::anyhow!(
            "`--theme` only applies to `calepin new website` or `calepin new theme`"
        ));
    }

    write_notebook_scaffold(&args.path, args.force)?;

    if !is_quiet() {
        eprintln!("Created {}", args.path.display());
    }

    Ok(())
}

fn default_website_dir() -> &'static Path {
    Path::new("calepin_website")
}

fn default_theme_dir() -> &'static Path {
    Path::new("calepin_theme")
}

fn write_notebook_scaffold(path: &Path, force: bool) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut options = OpenOptions::new();
    options.write(true);
    if force {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }

    let mut file = options
        .open(path)
        .with_context(|| format!("failed to create {}", path.display()))?;
    file.write_all(NEW_FILE_TEMPLATE.as_bytes())
        .with_context(|| format!("failed to write {}", path.display()))
}

pub fn handle_watch(mut args: WatchArgs) -> Result<()> {
    set_quiet(args.common.quiet);
    if args.input.is_dir() {
        return crate::website::watch_from_watch_args(args);
    }

    let format = resolve_output_format(args.format.map(OutputFormat::from), args.output.as_deref());
    let is_html = format == Some(OutputFormat::Html);

    validate_single_file_watch_flags(&args, is_html)?;
    apply_html_watch_typst_args(&mut args, is_html);

    crate::typst::watch::run_watch(args)
}

fn validate_single_file_watch_flags(args: &WatchArgs, is_html: bool) -> Result<()> {
    if is_html {
        return Ok(());
    }
    if args.serve {
        return Err(anyhow::anyhow!(
            "`calepin watch --serve` can only be used for HTML output"
        ));
    }
    if args.open {
        return Err(anyhow::anyhow!(
            "`calepin watch --open` can only be used for HTML output"
        ));
    }
    if args.port.is_some() {
        return Err(anyhow::anyhow!(
            "`calepin watch --port` can only be used for HTML output"
        ));
    }
    Ok(())
}

fn apply_html_watch_typst_args(args: &mut WatchArgs, is_html: bool) {
    if is_html {
        if args.open && !has_typst_open_flag(&args.typst_args) {
            args.typst_args.push("--open".to_string());
        }
        if let Some(port) = args.port {
            if !has_typst_port_flag(&args.typst_args) {
                args.typst_args.push("--port".to_string());
                args.typst_args.push(port.to_string());
            }
        }
    }
}

fn has_typst_open_flag(typst_args: &[String]) -> bool {
    typst_args
        .iter()
        .any(|arg| arg == "--open" || arg.starts_with("--open="))
}

fn has_typst_port_flag(typst_args: &[String]) -> bool {
    typst_args
        .iter()
        .any(|arg| arg == "--port" || arg.starts_with("--port="))
}

pub fn handle_clean(args: CleanArgs) -> Result<()> {
    let root = std::env::current_dir()?;
    let mut calepin_dirs = find_calepin_dirs(&root, args.depth)?;
    calepin_dirs.sort();

    if calepin_dirs.is_empty() {
        eprintln!("No .calepin directories found under {}", root.display());
        return Ok(());
    }

    eprintln!("The following directories will be removed:");
    for path in &calepin_dirs {
        eprintln!("  {}", path.display());
    }

    if !args.yes && !confirm_deletion()? {
        return Ok(());
    }

    for path in calepin_dirs {
        fs::remove_dir_all(&path)
            .with_context(|| format!("failed to remove {}", path.display()))?;
    }

    Ok(())
}

pub fn handle_compile(args: CompileArgs) -> Result<()> {
    set_quiet(args.common.quiet);
    if args.input.is_dir() {
        return crate::website::build_from_compile_args(args);
    }

    let format = args.format.map(OutputFormat::from);
    let current_dir = std::env::current_dir()?;
    let calepin_config =
        crate::config::CalepinConfig::load(&current_dir, args.common.config.as_deref())?;
    let config_styles = calepin_config.styles.clone();
    let site_context = SiteContextInput {
        revealjs: calepin_config.revealjs.clone(),
        ..Default::default()
    };
    let output = preprocess_cached(PreprocessOptions {
        input: args.input,
        root: None,
        config: args.common.config,
        display_root: None,
        quiet: args.common.quiet,
        status: true,
        progress: true,
        timeout: args.common.timeout,
        sync_pages: false,
        theme: None,
        fallback_theme: crate::theme::ThemeSelection::Default,
        html_syntax_theme: None,
        asset_dir: None,
        param_overrides: args.common.params,
    })?;
    compile_with_typst(
        &output.executables.typst,
        &output.layout,
        CompileOptions {
            output: args.output,
            format,
            typst_args: &args.typst_args,
            theme: &output.theme,
            html_scope: crate::theme::HtmlScope::Document,
            html_entry: None,
            config_styles: &config_styles,
            html_syntax_theme: None,
            site_context: Some(&site_context),
            pages_input: None,
            current_href_input: None,
            minify_html: args.minify,
            progress: true,
        },
    )?;
    Ok(())
}

fn find_calepin_dirs(root: &Path, max_depth: Option<usize>) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::from([(root.to_path_buf(), 0)]);

    while let Some((dir, depth)) = queue.pop_front() {
        if !dir.is_dir() {
            continue;
        }

        if dir.file_name().is_some_and(|name| name == ".calepin") {
            out.push(dir);
            continue;
        }

        let skip_children = max_depth.is_some_and(|max| depth >= max);
        if skip_children {
            continue;
        }

        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };

        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                queue.push_back((entry.path(), depth + 1));
            }
        }
    }

    Ok(out)
}

fn confirm_deletion() -> Result<bool> {
    let mut line = String::new();
    let mut stdout = io::stdout();

    loop {
        line.clear();
        print!("Proceed with deletion? [y/N] ");
        stdout.flush()?;
        io::stdin().read_line(&mut line)?;

        match line.trim().to_ascii_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "" | "n" | "no" => return Ok(false),
            _ => continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Command, CommonArgs, CompileFormat};
    use clap::Parser;

    #[test]
    fn new_writes_example_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes").join("example.typ");

        handle_new(NewArgs {
            path: path.clone(),
            theme: None,
            output: None,
            force: false,
        })
        .unwrap();

        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains(r#"#import "/.calepin/calepin.typ" as calepin"#));
        assert!(content.contains("calepin.inline.with(\"python\")"));
        assert!(content.contains("fenced-chunks: true"));
        assert!(content.contains("```python\n"));
        assert!(content.contains("print(40 + 2)"));
        assert!(content.contains("hello from a code chunk"));
    }

    #[test]
    fn new_does_not_overwrite_existing_file_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("example.typ");
        fs::write(&path, "existing").unwrap();

        let err = handle_new(NewArgs {
            path: path.clone(),
            theme: None,
            output: None,
            force: false,
        })
        .unwrap_err();

        assert!(err.to_string().contains("failed to create"));
        assert_eq!(fs::read_to_string(path).unwrap(), "existing");
    }

    #[test]
    fn new_force_overwrites_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("example.typ");
        fs::write(&path, "existing").unwrap();

        handle_new(NewArgs {
            path: path.clone(),
            theme: None,
            output: None,
            force: true,
        })
        .unwrap();

        assert!(fs::read_to_string(path)
            .unwrap()
            .contains("Calepin example"));
    }

    #[test]
    fn new_creates_plain_notebook_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.typ");
        handle_new(NewArgs {
            path: path.clone(),
            theme: None,
            output: None,
            force: false,
        })
        .unwrap();

        assert!(path.exists());
    }

    #[test]
    fn new_website_writes_to_requested_dir() {
        let dir = tempfile::tempdir().unwrap();
        let site = dir.path().join("site");

        handle_new(NewArgs {
            path: PathBuf::from("website"),
            theme: None,
            output: Some(site.clone()),
            force: false,
        })
        .unwrap();

        assert!(site.join("calepin.toml").exists());
        let config = std::fs::read_to_string(site.join("calepin.toml")).unwrap();
        assert!(config.contains("[[menus.main]]"));
        assert!(config.contains("[[menus.social]]"));
        assert!(!config.contains("[navbar]"));
        assert!(config
            .contains(r#"target = "https://scholar.google.com/citations?user=RKN66-kAAAAJ&hl""#));
        assert!(config.contains(r#"label = "{icon:simple-icons:googlescholar}""#));
        assert!(config.contains(r#"target = "https://github.com/vincentarelbundock/calepin""#));
        assert!(config.contains(r#"label = "{icon:github}""#));
        assert!(site.join("index.typ").exists());
        let index = std::fs::read_to_string(site.join("index.typ")).unwrap();
        assert!(index.contains("set page(columns: 2)"));
        assert!(index.contains("calepin-scaffold-portrait"));
        assert!(index.contains(r#"src: "assets/portrait.jpg""#));
        assert!(site.join("README.md").exists());
        let readme = std::fs::read_to_string(site.join("README.md")).unwrap();
        assert!(readme.contains("Helmut Koch"));
        assert!(readme.contains("Pixabay"));
        assert!(site.join("assets/portrait.jpg").exists());
        assert!(site.join("404.typ").exists());
        assert!(!site.join("assets/site.typ").exists());
        assert!(site.join("about.typ").exists());
        assert!(site.join("guide/features.typ").exists());
        assert!(site.join("guide/writing.typ").exists());
        assert!(site.join("blog.typ").exists());
        let blog = std::fs::read_to_string(site.join("blog.typ")).unwrap();
        assert!(blog.contains("#let listing("));
        assert!(blog.contains("table.hline"));
        assert!(!blog.contains(r#"#import "/assets/site.typ""#));
        assert!(site.join("posts/first-post.typ").exists());
        assert!(site.join("posts/theme-tour.typ").exists());
        assert!(site.join("posts/code-and-results.typ").exists());
        assert!(site.join("fr/index.typ").exists());
        assert!(site.join("fr/blog.typ").exists());
        assert!(site.join("fr/posts/theme-tour.typ").exists());
    }

    #[test]
    fn new_website_uses_selected_academic_theme() {
        let dir = tempfile::tempdir().unwrap();
        let site = dir.path().join("site");
        let site_arg = site.to_string_lossy().to_string();
        let args = parse_new_args([
            "calepin",
            "new",
            "website",
            site_arg.as_str(),
            "--theme",
            "academic",
        ]);

        handle_new(args).unwrap();

        let config = std::fs::read_to_string(site.join("calepin.toml")).unwrap();
        assert!(config.contains(r#"theme = "academic""#));
        let post = std::fs::read_to_string(site.join("posts/first-post.typ")).unwrap();
        assert!(post.contains(r#"thumbnail: "/assets/flowers_01.jpg""#));
    }

    #[test]
    fn new_theme_writes_to_requested_dir() {
        let dir = tempfile::tempdir().unwrap();
        let theme = dir.path().join("trash");

        handle_new(NewArgs {
            path: PathBuf::from("theme"),
            theme: None,
            output: Some(theme.clone()),
            force: false,
        })
        .unwrap();

        assert!(theme.join("layouts/webpage.html").exists());
        assert!(theme.join("partials/navbar-item.html").exists());
        assert!(theme.join("css/50_site.css").exists());
        assert!(theme.join("js/site.js").exists());
        assert!(!theme.join("academic").exists());
    }

    #[test]
    fn new_theme_uses_selected_academic_theme() {
        let dir = tempfile::tempdir().unwrap();
        let theme = dir.path().join("trash");
        let theme_arg = theme.to_string_lossy().to_string();
        let args = parse_new_args([
            "calepin",
            "new",
            "theme",
            theme_arg.as_str(),
            "--theme",
            "academic",
        ]);

        handle_new(args).unwrap();

        assert!(theme.join("layouts/webpage.html").exists());
        assert!(theme.join("partials/site-nav.html").exists());
        assert!(theme.join("partials/theme-toggle.html").exists());
        assert!(theme.join("css/50_main.css").exists());
        assert!(theme.join("js/main.js").exists());
        assert!(theme.join("js/copy-code.js").exists());
        assert!(!theme.parent().unwrap().join("shared").exists());
    }

    #[test]
    fn new_default_dirs_are_fixed() {
        assert_eq!(default_website_dir(), Path::new("calepin_website"));
        assert_eq!(default_theme_dir(), Path::new("calepin_theme"));
    }

    #[test]
    fn new_rejects_output_for_plain_files() {
        let dir = tempfile::tempdir().unwrap();
        let err = handle_new(NewArgs {
            path: dir.path().join("x.typ"),
            theme: None,
            output: Some(dir.path().join("site")),
            force: false,
        })
        .unwrap_err();

        assert!(err.to_string().contains("output path"));
        assert!(err.to_string().contains("new theme"));
    }

    #[test]
    fn new_rejects_theme_for_plain_files() {
        let dir = tempfile::tempdir().unwrap();
        let err = handle_new(NewArgs {
            path: dir.path().join("x.typ"),
            theme: Some(NewTheme::Academic),
            output: None,
            force: false,
        })
        .unwrap_err();

        assert!(err.to_string().contains("--theme"));
        assert!(err.to_string().contains("new website"));
    }

    fn parse_new_args<const N: usize>(args: [&str; N]) -> NewArgs {
        match Cli::try_parse_from(args).unwrap().command {
            Command::New(args) => args,
            other => panic!("expected new command, got {other:?}"),
        }
    }

    #[test]
    fn has_typst_open_flag_detects_typst_open_flags() {
        assert!(has_typst_open_flag(&["--open".to_string()]));
        assert!(has_typst_open_flag(&["--open=chromium".to_string()]));
        assert!(!has_typst_open_flag(&["--port".to_string()]));
    }

    #[test]
    fn has_typst_port_flag_detects_typst_port_flags() {
        assert!(has_typst_port_flag(&["--port".to_string()]));
        assert!(has_typst_port_flag(&["--port=3001".to_string()]));
        assert!(!has_typst_port_flag(&["--open".to_string()]));
    }

    #[test]
    fn watch_rejects_serve_for_non_html_notebooks() {
        let mut args = watch_args(PathBuf::from("missing.typ"), Some(CompileFormat::Pdf));
        args.serve = true;

        let err = handle_watch(args).unwrap_err().to_string();

        assert!(err.contains("--serve"), "{err}");
        assert!(err.contains("HTML"), "{err}");
    }

    #[test]
    fn watch_rejects_port_for_non_html_notebooks() {
        let mut args = watch_args(PathBuf::from("missing.typ"), Some(CompileFormat::Pdf));
        args.port = Some(3000);

        let err = handle_watch(args).unwrap_err().to_string();

        assert!(err.contains("--port"), "{err}");
        assert!(err.contains("HTML"), "{err}");
    }

    #[test]
    fn html_watch_allows_serve_and_forwards_open_and_port_to_typst() {
        let mut args = watch_args(PathBuf::from("missing.typ"), Some(CompileFormat::Html));
        args.serve = true;
        args.open = true;
        args.port = Some(3000);

        validate_single_file_watch_flags(&args, true).unwrap();
        apply_html_watch_typst_args(&mut args, true);

        assert!(args.typst_args.contains(&"--open".to_string()));
        assert!(args
            .typst_args
            .windows(2)
            .any(|pair| pair == ["--port", "3000"]));
    }

    fn watch_args(input: PathBuf, format: Option<CompileFormat>) -> WatchArgs {
        WatchArgs {
            input,
            output: None,
            format,
            serve: false,
            open: false,
            host: "127.0.0.1".to_string(),
            port: None,
            common: CommonArgs {
                config: None,
                quiet: true,
                timeout: None,
                params: Vec::new(),
            },
            typst_args: Vec::new(),
        }
    }
}
