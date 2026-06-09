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
    if args.path == Path::new("website") {
        crate::website::scaffold_website(Path::new("."), args.force)?;
        if !crate::cli::QUIET.load(std::sync::atomic::Ordering::Relaxed) {
            eprintln!("Created website scaffold in .");
        }
        return Ok(());
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
    let template_name = args.template.as_deref();
    if args.template.is_some() && format.as_deref() != Some("html") {
        return Err(anyhow::anyhow!(
            "`--template` can only be used with `--format html`"
        ));
    }
    let output = preprocess_cached(PreprocessOptions {
        input: args.input,
        config: args.common.config,
        quiet: args.common.quiet,
        timeout: args.common.timeout,
        sync_pages: false,
        param_overrides: args.common.params,
    })?;
    compile_with_typst(
        &output.executables.typst,
        &output.layout,
        CompileOptions {
            output: args.output,
            format: format.as_deref(),
            typst_args: &args.typst_args,
            template_theme: template_name,
            themes_dir: &output.themes_dir,
            site_context: None,
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
            force: false,
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
            force: true,
        })
        .unwrap();

        assert!(fs::read_to_string(path)
            .unwrap()
            .contains("Calepin example"));
    }
}
