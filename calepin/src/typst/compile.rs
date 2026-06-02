use anyhow::{anyhow, Context, Result};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::html::{apply_html_theme_file, inline_html_images_file, prepare_html_theme};
use crate::typst::model::LayoutPaths;
use crate::typst::paths::artifact_reference;

pub struct CompileOptions<'a> {
    pub output: Option<PathBuf>,
    pub format: Option<&'a str>,
    pub typst_args: &'a [String],
    pub html_in_markdown: bool,
    pub template_theme: Option<&'a str>,
    pub themes_dir: &'a Path,
}

pub fn reject_reserved_typst_inputs(args: &[String]) -> Result<()> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if let Some(value) = arg.strip_prefix("--input=") {
            reject_reserved_input_value(value)?;
        } else if arg == "--input" {
            let Some(value) = iter.next() else {
                return Err(anyhow!(
                    "`--input` in forwarded Typst args requires a value"
                ));
            };
            reject_reserved_input_value(value)?;
        }
    }
    Ok(())
}

fn reject_reserved_input_value(value: &str) -> Result<()> {
    if value.starts_with("calepin-mode=")
        || value.starts_with("calepin-results=")
        || value.starts_with("calepin-target=")
        || value.starts_with("calepin-raw-theme=")
        || value.starts_with("calepin-assets=")
    {
        return Err(anyhow!(
            "forwarded Typst args may not override reserved Calepin input `{}`",
            value
        ));
    }
    Ok(())
}

fn typst_subcommand_args(
    subcommand: &str,
    layout: &LayoutPaths,
    output: Option<&Path>,
    format: Option<&str>,
    typst_args: &[String],
    raw_theme_input: Option<&str>,
    asset_base_input: Option<&str>,
) -> Vec<OsString> {
    let results_input = artifact_reference(&layout.root, &layout.results_path);
    let target = if format == Some("html") {
        "html"
    } else {
        "paged"
    };

    let mut args = vec![
        subcommand.into(),
        "--root".into(),
        layout.root.as_os_str().into(),
        "--input".into(),
        "calepin-mode=render".into(),
        "--input".into(),
        format!("calepin-results={results_input}").into(),
        "--input".into(),
        format!("calepin-target={target}").into(),
    ];

    if let Some(format) = format {
        args.push("--format".into());
        args.push(format.into());
        if format == "html" {
            args.push("--features=html".into());
            if let Some(raw_theme_input) = raw_theme_input {
                args.push("--input".into());
                args.push(format!("calepin-raw-theme={raw_theme_input}").into());
            }
        }
    }
    if let Some(asset_base_input) = asset_base_input {
        args.push("--input".into());
        args.push(format!("calepin-assets={asset_base_input}").into());
    }

    args.push(layout.render_input.as_os_str().into());
    if let Some(output) = output {
        args.push(output.as_os_str().into());
    } else {
        let default_output = resolve_output_path(layout, output, format, false);
        args.push(default_output.as_os_str().into());
    }
    args.extend(typst_args.iter().map(|arg| OsString::from(arg.as_str())));
    args
}

pub(crate) fn typst_compile_args(
    layout: &LayoutPaths,
    output: Option<&Path>,
    format: Option<&str>,
    typst_args: &[String],
    raw_theme_input: Option<&str>,
) -> Vec<OsString> {
    typst_subcommand_args(
        "compile",
        layout,
        output,
        format,
        typst_args,
        raw_theme_input,
        None,
    )
}

pub(crate) fn typst_watch_args(
    layout: &LayoutPaths,
    output: Option<&Path>,
    format: Option<&str>,
    typst_args: &[String],
    raw_theme_input: Option<&str>,
    asset_base_input: Option<&str>,
) -> Vec<OsString> {
    typst_subcommand_args(
        "watch",
        layout,
        output,
        format,
        typst_args,
        raw_theme_input,
        asset_base_input,
    )
}

fn html_output_path(
    layout: &LayoutPaths,
    output: Option<&Path>,
    format: Option<&str>,
    html_in_markdown: bool,
) -> Option<PathBuf> {
    if format != Some("html") {
        return None;
    }
    Some(resolve_output_path(layout, output, format, html_in_markdown))
}

pub(crate) fn resolve_output_path(
    layout: &LayoutPaths,
    output: Option<&Path>,
    format: Option<&str>,
    html_in_markdown: bool,
) -> PathBuf {
    let extension = match format {
        Some("html") => {
            if html_in_markdown {
                "md"
            } else {
                "html"
            }
        }
        Some("png") => "png",
        Some("svg") => "svg",
        _ => "pdf",
    };
    match output {
        Some(path) if path.is_absolute() => {
            if path.extension().is_some() {
                path.to_path_buf()
            } else {
                path.with_extension(extension)
            }
        }
        Some(path) => {
            let with_root = layout.root.join(path);
            if path.extension().is_some() {
                with_root
            } else {
                with_root.with_extension(extension)
            }
        }
        None => layout
            .root
            .join(&layout.input_rel)
            .with_extension(extension),
    }
}

fn resolve_default_output_path(
    layout: &LayoutPaths,
    output: Option<&Path>,
    format: Option<&str>,
    html_in_markdown: bool,
) -> PathBuf {
    resolve_output_path(layout, output, format, html_in_markdown)
}

pub fn compile_with_typst(
    typst: &Path,
    layout: &LayoutPaths,
    options: CompileOptions<'_>,
) -> Result<()> {
    let html_theme = options.template_theme;
    reject_reserved_typst_inputs(options.typst_args)?;
    let prepared_theme = prepare_html_theme(
        &layout.root,
        options.format,
        html_theme,
        None,
        None,
    )?;
    let output_path = resolve_default_output_path(
        layout,
        options.output.as_deref(),
        options.format,
        options.html_in_markdown,
    );
    let html_output = html_output_path(
        layout,
        Some(output_path.as_path()),
        options.format,
        options.html_in_markdown,
    );
    let args = typst_compile_args(
        layout,
        Some(output_path.as_path()),
        options.format,
        options.typst_args,
        prepared_theme.raw_theme_input.as_deref(),
    );
    let mut command = Command::new(typst);
    command.args(args).current_dir(&layout.root);
    let command_output = command
        .output()
        .with_context(|| format!("failed to run {}", typst.display()))?;
    if !command_output.status.success() {
        return Err(anyhow!(
            "typst compile failed:\n{}",
            String::from_utf8_lossy(&command_output.stderr)
        ));
    }
    if let Some(path) = html_output {
        apply_html_theme_file(
            &path,
            html_theme,
            options.themes_dir,
            &prepared_theme.syntax_theme,
        )?;
        inline_html_images_file(&path, &layout.root)?;

        if options.html_in_markdown {
            crate::html::render_html_in_markdown(&path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::paths::resolve_layout;

    #[test]
    fn rejects_reserved_forwarded_inputs() {
        let err = reject_reserved_typst_inputs(&[
            "--input".to_string(),
            "calepin-mode=query".to_string(),
        ])
        .unwrap_err()
        .to_string();
        assert!(err.contains("reserved Calepin input"));

        let err = reject_reserved_typst_inputs(&[
            "--input=calepin-results=.calepin/paper/results.json".to_string(),
        ])
        .unwrap_err()
        .to_string();
        assert!(err.contains("reserved Calepin input"));

        let err = reject_reserved_typst_inputs(&["--input=calepin-target=html".to_string()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("reserved Calepin input"));

        let err = reject_reserved_typst_inputs(&[
            "--input=calepin-assets=http://127.0.0.1:3001".to_string()
        ])
        .unwrap_err()
        .to_string();
        assert!(err.contains("reserved Calepin input"));

        let err = reject_reserved_typst_inputs(&[
            "--input".to_string(),
            "calepin-raw-theme=theme.tmTheme".to_string(),
        ])
        .unwrap_err()
        .to_string();
        assert!(err.contains("reserved Calepin input"));
    }

    #[test]
    fn accepts_unrelated_forwarded_inputs() {
        reject_reserved_typst_inputs(&[
            "--input".to_string(),
            "theme=dark".to_string(),
            "--font-path".to_string(),
            "fonts".to_string(),
        ])
        .unwrap();
    }

    #[test]
    fn html_format_enables_typst_html_feature() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(dir.path()), None).unwrap();

        let args = typst_compile_args(
            &layout,
            Some(&PathBuf::from("paper.html")),
            Some("html"),
            &[],
            None,
        );
        let args: Vec<_> = args
            .into_iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        assert!(args
            .windows(2)
            .any(|pair| pair == ["--input", "calepin-target=html"]));
        assert!(args.windows(2).any(|pair| pair == ["--format", "html"]));
        assert!(args.contains(&"--features=html".to_string()));
    }

    #[test]
    fn html_compile_passes_prepared_raw_theme_path() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(dir.path()), None).unwrap();

        let args = typst_compile_args(
            &layout,
            Some(&PathBuf::from("paper.html")),
            Some("html"),
            &[],
            Some("/.calepin/calepin-input-light.tmTheme"),
        );
        let args: Vec<_> = args
            .into_iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        assert!(args.windows(2).any(|pair| {
            pair == [
                "--input",
                "calepin-raw-theme=/.calepin/calepin-input-light.tmTheme",
            ]
        }));
    }

    #[test]
    fn non_html_formats_do_not_enable_typst_html_feature() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(dir.path()), None).unwrap();

        let args = typst_compile_args(
            &layout,
            Some(&PathBuf::from("paper.pdf")),
            Some("pdf"),
            &[],
            None,
        );
        let args: Vec<_> = args
            .into_iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        assert!(args
            .windows(2)
            .any(|pair| pair == ["--input", "calepin-target=paged"]));
        assert!(args.windows(2).any(|pair| pair == ["--format", "pdf"]));
        assert!(!args.contains(&"--features=html".to_string()));
    }

    #[test]
    fn resolve_output_path_uses_explicit_then_default_extension() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(dir.path()), None).unwrap();

        assert_eq!(
            resolve_output_path(&layout, Some(Path::new("out/report.pdf")), Some("pdf"), false),
            layout.root.join("out/report.pdf")
        );
        assert_eq!(
            resolve_output_path(&layout, None, Some("pdf"), false),
            layout.root.join("paper.pdf")
        );
        assert_eq!(
            resolve_output_path(&layout, None, Some("html"), false),
            layout.root.join("paper.html")
        );
        assert_eq!(
            resolve_output_path(&layout, None, None, false),
            layout.root.join("paper.pdf")
        );
    }

    #[test]
    fn compile_args_defaults_output_to_input_stem() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(dir.path()), None).unwrap();

        let args = typst_compile_args(&layout, None, Some("pdf"), &[], None);
        let args: Vec<_> = args
            .into_iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        assert!(args.contains(&layout.root.join("paper.pdf").to_string_lossy().to_string()));
    }

    #[test]
    fn watch_args_mirror_compile_args_with_watch_subcommand() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(dir.path()), None).unwrap();

        let args = typst_watch_args(
            &layout,
            Some(&PathBuf::from("paper.pdf")),
            Some("pdf"),
            &[],
            None,
            None,
        );
        let args: Vec<_> = args
            .into_iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        assert_eq!(args.first().map(String::as_str), Some("watch"));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--input", "calepin-mode=render"]));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--input", "calepin-target=paged"]));
        assert!(args.windows(2).any(|pair| pair == ["--format", "pdf"]));
        assert!(args.contains(&"paper.pdf".to_string()));
    }

    #[test]
    fn watch_args_default_output_to_input_stem() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(dir.path()), None).unwrap();

        let args = typst_watch_args(
            &layout,
            None,
            Some("pdf"),
            &[],
            None,
            None,
        );
        let args: Vec<_> = args
            .into_iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        assert!(args.contains(&layout.root.join("paper.pdf").to_string_lossy().to_string()));
    }

    #[test]
    fn html_watch_passes_calepin_asset_base() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(dir.path()), None).unwrap();

        let args = typst_watch_args(
            &layout,
            Some(&PathBuf::from("paper.html")),
            Some("html"),
            &[],
            None,
            Some("http://127.0.0.1:3002"),
        );
        let args: Vec<_> = args
            .into_iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        assert!(args
            .windows(2)
            .any(|pair| { pair == ["--input", "calepin-assets=http://127.0.0.1:3002"] }));
    }
}
