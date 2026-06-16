use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::cli::{set_quiet, CleanArgs, CompileArgs, NewArgs, StopArgs, WatchArgs};
use crate::typst::compile::{compile_with_typst, CompileOptions};
use crate::typst::preprocess::{preprocess_cached, PreprocessOptions};

const NEW_FILE_TEMPLATE: &str = include_str!("../assets/scaffolds/notebook/notebook.typ");

pub fn handle_new(args: NewArgs) -> Result<()> {
    if args.path == Path::new("theme") {
        let name = args
            .theme
            .as_deref()
            .unwrap_or(crate::theme::DEFAULT_THEME_NAME);
        let dest = match args.output.as_deref() {
            Some(output) => crate::theme::eject_builtin_to(name, output, args.force)?,
            None => crate::theme::eject_builtin(name, Path::new("themes"), args.force)?,
        };
        if !crate::cli::QUIET.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!("Created {}", dest.display());
            eprintln!(
                "Select it with `theme = \"{}\"` in calepin.toml,",
                dest.display()
            );
            eprintln!("or `--theme {}` on calepin compile/watch.", dest.display());
        }
        return Ok(());
    }
    if args.theme.is_some() {
        return Err(anyhow::anyhow!(
            "`--theme` only applies to `calepin new theme`"
        ));
    }

    if args.path == Path::new("website") {
        let dest = args.output.as_deref().unwrap_or(Path::new("docs"));
        crate::website::scaffold_website(dest, args.force)?;
        if !crate::cli::QUIET.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!("Created website scaffold in {}", dest.display());
        }
        return Ok(());
    }

    if args.output.is_some() {
        return Err(anyhow::anyhow!(
            "an output path only applies to `calepin new website` or `calepin new theme`"
        ));
    }

    if let Some(parent) = args
        .path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let mut options = OpenOptions::new();
    options.write(true);
    if args.force {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }

    let mut file = options
        .open(&args.path)
        .with_context(|| format!("failed to create {}", args.path.display()))?;
    file.write_all(NEW_FILE_TEMPLATE.as_bytes())
        .with_context(|| format!("failed to write {}", args.path.display()))?;

    if !crate::cli::QUIET.load(std::sync::atomic::Ordering::Relaxed) {
        eprintln!("Created {}", args.path.display());
    }

    Ok(())
}

pub fn handle_watch(args: WatchArgs) -> Result<()> {
    set_quiet(args.common.quiet);
    if args.input.is_dir() {
        return crate::website::watch_from_watch_args(args);
    }
    if args.serve {
        return Err(anyhow::anyhow!(
            "`calepin watch --serve` can only be used when watching a website directory"
        ));
    }
    if args.open {
        return Err(anyhow::anyhow!(
            "`calepin watch --open` is only for website serving; pass Typst's flag after `--` as `calepin watch paper.typ -- --open`"
        ));
    }
    crate::typst::watch::run_watch(args)
}

pub fn handle_stop(args: StopArgs) -> Result<()> {
    crate::typst::watch::run_stop(args)
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

    let format = args.format.map(|format| format.as_str().to_string());
    let current_dir = std::env::current_dir()?;
    let theme = args
        .theme
        .as_deref()
        .map(|value| crate::theme::ThemeSelection::parse(value, &current_dir))
        .transpose()?;
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
        theme,
        fallback_theme: crate::theme::ThemeSelection::Default,
        html_syntax_theme: None,
        param_overrides: args.common.params,
    })?;
    compile_with_typst(
        &output.executables.typst,
        &output.layout,
        CompileOptions {
            output: args.output,
            format: format.as_deref(),
            typst_args: &args.typst_args,
            theme: &output.theme,
            html_scope: crate::theme::HtmlScope::Document,
            html_entry: None,
            html_syntax_theme: None,
            site_context: None,
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

    #[test]
    fn new_writes_example_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes").join("example.typ");

        handle_new(NewArgs {
            path: path.clone(),
            output: None,
            force: false,
            theme: None,
        })
        .unwrap();

        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains(r#"#import "@preview/calepin:0.0.1" as calepin"#));
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
            output: None,
            force: false,
            theme: None,
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
            output: None,
            force: true,
            theme: None,
        })
        .unwrap();

        assert!(fs::read_to_string(path)
            .unwrap()
            .contains("Calepin example"));
    }

    #[test]
    fn new_rejects_theme_flag_for_plain_files() {
        let dir = tempfile::tempdir().unwrap();
        let err = handle_new(NewArgs {
            path: dir.path().join("x.typ"),
            output: None,
            force: false,
            theme: Some("calepin".to_string()),
        })
        .unwrap_err();

        assert!(err.to_string().contains("--theme"));
    }

    #[test]
    fn new_website_writes_to_requested_dir() {
        let dir = tempfile::tempdir().unwrap();
        let site = dir.path().join("site");

        handle_new(NewArgs {
            path: PathBuf::from("website"),
            output: Some(site.clone()),
            force: false,
            theme: None,
        })
        .unwrap();

        assert!(site.join("calepin.toml").exists());
        let config = std::fs::read_to_string(site.join("calepin.toml")).unwrap();
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
    fn new_theme_writes_to_requested_dir() {
        let dir = tempfile::tempdir().unwrap();
        let theme = dir.path().join("trash");

        handle_new(NewArgs {
            path: PathBuf::from("theme"),
            output: Some(theme.clone()),
            force: false,
            theme: Some("academic".to_string()),
        })
        .unwrap();

        assert!(theme.join("layouts/webpage.html").exists());
        assert!(theme.join("partials/navbar-item.html").exists());
        assert!(theme.join("styles/main.css").exists());
        assert!(theme.join("scripts/main.js").exists());
        assert!(!theme.join("academic").exists());
    }

    #[test]
    fn new_rejects_output_for_plain_files() {
        let dir = tempfile::tempdir().unwrap();
        let err = handle_new(NewArgs {
            path: dir.path().join("x.typ"),
            output: Some(dir.path().join("site")),
            force: false,
            theme: None,
        })
        .unwrap_err();

        assert!(err.to_string().contains("output path"));
        assert!(err.to_string().contains("new theme"));
    }
}
