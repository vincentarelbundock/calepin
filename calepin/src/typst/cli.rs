use std::fs::{self, OpenOptions};
use std::io::Write;

use anyhow::{Context, Result};

use crate::cli::{set_quiet, CompileArgs, CompileOutputTemplate, NewArgs, StopArgs, WatchArgs};
use crate::typst::compile::{compile_with_typst, CompileOptions};
use crate::typst::preprocess::{preprocess, PreprocessOptions};

const NEW_FILE_TEMPLATE: &str = r#"#import ".calepin/calepin.typ"

#set document(title: [Calepin example])

#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
  raw-chunks: true,
)

#let py = calepin.inline.with("python")

= Calepin example

Inline Python result: #py[`print(40 + 2)`].

```python
message = "hello from a code chunk"
print(message)
```
"#;

pub fn handle_new(args: NewArgs) -> Result<()> {
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
    crate::typst::watch::run_watch(args)
}

pub fn handle_stop(args: StopArgs) -> Result<()> {
    crate::typst::watch::run_stop(args)
}

pub fn handle_compile(args: CompileArgs) -> Result<()> {
    set_quiet(args.common.quiet);
    let format = args.format.map(|format| format.as_str().to_string());
    let template_name = args.template.map(CompileOutputTemplate::html_template_name).flatten();
    let html_in_markdown = args
        .template
        .as_ref()
        .is_some_and(CompileOutputTemplate::is_html_in_markdown);
    if args.template.is_some() && format.as_deref() != Some("html") {
        return Err(anyhow::anyhow!(
            "`--template` can only be used with `--format html`"
        ));
    }
    let output = preprocess(PreprocessOptions {
        input: args.input,
        results: args.common.results,
        clean: args.common.clean,
        quiet: args.common.quiet,
        timeout: args.common.timeout,
        sync_pages: false,
    })?;
    compile_with_typst(
        &output.executables.typst,
        &output.layout,
        CompileOptions {
            output: args.output,
            format: format.as_deref(),
            typst_args: &args.typst_args,
            html_in_markdown,
            template_theme: template_name,
            themes_dir: &output.themes_dir,
        },
    )?;
    Ok(())
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
        assert!(content.contains(r#"#import ".calepin/calepin.typ""#));
        assert!(content.contains("calepin.inline.with(\"python\")"));
        assert!(content.contains("raw-chunks: true"));
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
