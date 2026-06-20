use anyhow::{anyhow, Result};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::html::{
    apply_html_theme_file_with_site_context, inline_html_images_file, minify_html_file,
    HtmlSyntaxTheme, SiteContextInput,
};
use crate::typst::model::LayoutPaths;
use crate::typst::paths::artifact_reference;
use crate::typst::run::{
    push_calepin_inputs, push_input, run_typst_status, source_dir_input, CalepinMode,
    CalepinTarget, INPUT_ASSETS, INPUT_CURRENT_HREF, INPUT_IMAGE_META, INPUT_PAGES,
    INPUT_SOURCE_DIR, RESERVED_INPUT_KEYS,
};
use crate::typst::version::assert_supported_typst;
use crate::utils::progress::Progress;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Pdf,
    Png,
    Svg,
    Html,
}

impl OutputFormat {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Png => "png",
            Self::Svg => "svg",
            Self::Html => "html",
        }
    }

    fn extension(self) -> &'static str {
        self.as_str()
    }

    fn target(self) -> CalepinTarget {
        match self {
            Self::Html => CalepinTarget::Html,
            Self::Pdf | Self::Png | Self::Svg => CalepinTarget::Paged,
        }
    }

    fn is_html(self) -> bool {
        self == Self::Html
    }
}

impl From<crate::cli::CompileFormat> for OutputFormat {
    fn from(value: crate::cli::CompileFormat) -> Self {
        match value {
            crate::cli::CompileFormat::Pdf => Self::Pdf,
            crate::cli::CompileFormat::Png => Self::Png,
            crate::cli::CompileFormat::Svg => Self::Svg,
            crate::cli::CompileFormat::Html => Self::Html,
        }
    }
}

pub struct CompileOptions<'a> {
    pub output: Option<PathBuf>,
    pub format: Option<OutputFormat>,
    pub typst_args: &'a [String],
    pub theme: &'a crate::theme::ThemeSelection,
    /// Which HTML layout the theme provides for this render: a standalone
    /// document or a website page.
    pub html_scope: crate::theme::HtmlScope,
    /// Pre-resolved HTML layout to use instead of resolving `theme` +
    /// `html_scope`.
    pub html_entry: Option<&'a crate::theme::HtmlEntry>,
    pub config_styles: &'a [crate::config::CssOverride],
    pub html_syntax_theme: Option<&'a HtmlSyntaxTheme>,
    pub site_context: Option<&'a SiteContextInput>,
    /// Root-relative path to the website pages index, exposed to the runtime
    /// as the `calepin-pages` input.
    pub pages_input: Option<&'a str>,
    /// Site-relative html path of the page being rendered, exposed as the
    /// `calepin-current-href` input.
    pub current_href_input: Option<&'a str>,
    /// Minify final HTML output after theming and asset processing.
    pub minify_html: bool,
    /// Show a live status line around the final Typst render.
    pub progress: bool,
}

/// Optional reserved `--input` values forwarded to the typst subcommand.
#[derive(Default, Clone, Copy)]
pub(crate) struct ReservedInputs<'a> {
    pub asset_base: Option<&'a str>,
    pub pages: Option<&'a str>,
    pub current_href: Option<&'a str>,
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

pub(crate) fn validate_forwarded_typst_args(
    args: &[String],
    format: Option<OutputFormat>,
) -> Result<()> {
    reject_reserved_typst_inputs(args)?;
    for arg in args {
        if arg == "--format" || arg.starts_with("--format=") {
            return Err(anyhow!(
                "`--format` is managed by Calepin; use `calepin compile --format` or `calepin watch --format` instead"
            ));
        }
        if arg == "--root" || arg.starts_with("--root=") {
            return Err(anyhow!(
                "`--root` is managed by Calepin and cannot be forwarded to Typst"
            ));
        }
        if format == Some(OutputFormat::Html)
            && (arg == "--features" || arg.starts_with("--features="))
        {
            return Err(anyhow!(
                "`--features` is managed by Calepin for HTML output so the required `html` feature stays enabled"
            ));
        }
    }
    Ok(())
}

fn reject_reserved_input_value(value: &str) -> Result<()> {
    if RESERVED_INPUT_KEYS
        .iter()
        .any(|key| value.starts_with(&format!("{key}=")))
    {
        return Err(anyhow!(
            "forwarded Typst args may not override reserved Calepin input `{}`",
            value
        ));
    }
    Ok(())
}

fn infer_output_format(output: &Path) -> Option<OutputFormat> {
    match output.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => match ext.to_ascii_lowercase().as_str() {
            "html" => Some(OutputFormat::Html),
            "pdf" => Some(OutputFormat::Pdf),
            "png" => Some(OutputFormat::Png),
            "svg" => Some(OutputFormat::Svg),
            _ => None,
        },
        None => None,
    }
}

/// Explicit `--format` wins; otherwise infer from the output extension when possible.
pub(crate) fn resolve_output_format(
    format: Option<OutputFormat>,
    output: Option<&Path>,
) -> Option<OutputFormat> {
    format.or_else(|| output.and_then(infer_output_format))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedOutput {
    path: PathBuf,
    arg_path: PathBuf,
    format: Option<OutputFormat>,
}

impl ResolvedOutput {
    fn target(&self) -> CalepinTarget {
        self.format
            .map(OutputFormat::target)
            .unwrap_or(CalepinTarget::Paged)
    }

    fn is_html(&self) -> bool {
        self.format.is_some_and(OutputFormat::is_html)
    }
}

fn resolve_render_output(
    layout: &LayoutPaths,
    output: Option<&Path>,
    format: Option<OutputFormat>,
) -> ResolvedOutput {
    let path = resolve_output_path(layout, output, format);
    let format = resolve_output_format(format, Some(path.as_path()));
    let arg_path = resolve_output_arg_path(layout, output, format);
    ResolvedOutput {
        path,
        arg_path,
        format,
    }
}

fn typst_subcommand_args(
    subcommand: &str,
    layout: &LayoutPaths,
    output: &ResolvedOutput,
    typst_args: &[String],
    inputs: ReservedInputs<'_>,
) -> Vec<OsString> {
    let results_input = artifact_reference(&layout.root, &layout.results_path);
    let mut args = vec![
        subcommand.into(),
        "--root".into(),
        layout.root.as_os_str().into(),
    ];
    push_calepin_inputs(
        &mut args,
        CalepinMode::Render,
        &results_input,
        output.target(),
    );
    push_input(&mut args, INPUT_SOURCE_DIR, source_dir_input(layout));
    let image_meta = artifact_reference(
        &layout.root,
        &layout
            .root
            .join(crate::typst::preprocess::image_meta_relative_path(layout)),
    );
    push_input(&mut args, INPUT_IMAGE_META, image_meta);

    if let Some(format) = output.format {
        args.push("--format".into());
        args.push(format.as_str().into());
        if format.is_html() {
            args.push("--features=html".into());
        }
    }
    if let Some(asset_base_input) = inputs.asset_base {
        push_input(&mut args, INPUT_ASSETS, asset_base_input);
    }
    if let Some(pages_input) = inputs.pages {
        push_input(&mut args, INPUT_PAGES, pages_input);
    }
    if let Some(current_href_input) = inputs.current_href {
        push_input(&mut args, INPUT_CURRENT_HREF, current_href_input);
    }

    args.push(layout.render_input.as_os_str().into());
    args.push(output.arg_path.as_os_str().into());
    args.extend(typst_args.iter().map(|arg| OsString::from(arg.as_str())));
    args
}

#[cfg(test)]
pub(crate) fn typst_compile_args(
    layout: &LayoutPaths,
    output: Option<&Path>,
    format: Option<OutputFormat>,
    typst_args: &[String],
    inputs: ReservedInputs<'_>,
) -> Vec<OsString> {
    let output = resolve_render_output(layout, output, format);
    typst_subcommand_args("compile", layout, &output, typst_args, inputs)
}

pub(crate) fn typst_watch_args(
    layout: &LayoutPaths,
    output: Option<&Path>,
    format: Option<OutputFormat>,
    typst_args: &[String],
    inputs: ReservedInputs<'_>,
) -> Vec<OsString> {
    let output = resolve_render_output(layout, output, format);
    typst_subcommand_args("watch", layout, &output, typst_args, inputs)
}

fn output_extension(format: Option<OutputFormat>) -> &'static str {
    format.unwrap_or(OutputFormat::Pdf).extension()
}

pub(crate) fn resolve_output_path(
    layout: &LayoutPaths,
    output: Option<&Path>,
    format: Option<OutputFormat>,
) -> PathBuf {
    let extension = output_extension(format);
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

fn resolve_output_arg_path(
    layout: &LayoutPaths,
    output: Option<&Path>,
    format: Option<OutputFormat>,
) -> PathBuf {
    let extension = output_extension(format);
    match output {
        Some(path) if path.extension().is_some() => path.to_path_buf(),
        Some(path) => path.with_extension(extension),
        None => layout
            .root
            .join(&layout.input_rel)
            .with_extension(extension),
    }
}

pub(crate) fn resolve_html_entry_with_styles(
    theme: &crate::theme::ThemeSelection,
    scope: crate::theme::HtmlScope,
    config_styles: &[crate::config::CssOverride],
) -> Result<Option<crate::theme::HtmlEntry>> {
    let mut resolved_entry = crate::theme::resolve_html_entry(theme, scope)?;

    if let Some(entry) = resolved_entry.as_mut() {
        if !config_styles.is_empty() {
            entry.append_styles(config_styles.to_vec());
        }
        return Ok(Some(entry.clone()));
    }

    if !config_styles.is_empty() {
        Ok(Some(crate::theme::style_only_html_entry(
            config_styles.to_vec(),
        )))
    } else {
        Ok(None)
    }
}

pub(crate) fn postprocess_html_output(
    output: &Path,
    layout: &LayoutPaths,
    html_entry: Option<&crate::theme::HtmlEntry>,
    syntax_theme: &HtmlSyntaxTheme,
    site_context: Option<&SiteContextInput>,
    minify_html: bool,
) -> Result<()> {
    if !output.exists() {
        return Ok(());
    }

    apply_html_theme_file_with_site_context(
        output,
        html_entry,
        syntax_theme,
        &layout.root,
        site_context,
    )?;
    inline_html_images_file(output, &layout.root)?;
    if minify_html {
        minify_html_file(output)?;
    }

    Ok(())
}

pub(crate) fn postprocess_compiled_html_output(
    output: &Path,
    layout: &LayoutPaths,
    html_entry: Option<&crate::theme::HtmlEntry>,
    syntax_theme: &HtmlSyntaxTheme,
    site_context: Option<&SiteContextInput>,
    minify_html: bool,
) -> Result<()> {
    if !output.exists() {
        return Err(anyhow!(
            "missing HTML output after typst compile: {}",
            output.display()
        ));
    }
    postprocess_html_output(
        output,
        layout,
        html_entry,
        syntax_theme,
        site_context,
        minify_html,
    )
}

pub fn compile_with_typst(
    typst: &Path,
    layout: &LayoutPaths,
    options: CompileOptions<'_>,
) -> Result<()> {
    let output = resolve_render_output(layout, options.output.as_deref(), options.format);
    let output_path = &output.path;
    let html_entry = if output.is_html() {
        if let Some(entry) = options.html_entry.cloned() {
            let mut owned_entry = entry.clone();
            if !options.config_styles.is_empty() {
                owned_entry.append_styles(options.config_styles.to_vec());
            }
            Some(owned_entry)
        } else {
            resolve_html_entry_with_styles(
                options.theme,
                options.html_scope,
                options.config_styles,
            )?
        }
    } else {
        None
    };
    let html_entry = html_entry.as_ref();
    validate_forwarded_typst_args(options.typst_args, output.format)?;
    let args = typst_subcommand_args(
        "compile",
        layout,
        &output,
        options.typst_args,
        ReservedInputs {
            asset_base: None,
            pages: options.pages_input,
            current_href: options.current_href_input,
        },
    );
    assert_supported_typst(typst)?;
    let progress = Progress::spinner(
        format!(
            "[render] {} -> {}",
            layout.input_rel.display(),
            output_path
                .strip_prefix(&layout.root)
                .unwrap_or(output_path)
                .display()
        ),
        crate::cli::is_quiet() || !options.progress,
    );
    run_typst_status(typst, "run typst compile", &args, &layout.root, |stderr| {
        format!("typst compile failed:\n{stderr}")
    })?;
    if output.is_html() {
        progress.set_message(format!(
            "[html] {}",
            output_path
                .strip_prefix(&layout.root)
                .unwrap_or(output_path)
                .display()
        ));
        let builtin_syntax_theme;
        let syntax_theme = if let Some(theme) = options.html_syntax_theme {
            theme
        } else {
            builtin_syntax_theme = HtmlSyntaxTheme::builtin();
            &builtin_syntax_theme
        };
        postprocess_compiled_html_output(
            output_path,
            layout,
            html_entry,
            syntax_theme,
            options.site_context,
            options.minify_html,
        )?;
    }
    progress.finish(format!(
        "[done] {}",
        output_path
            .strip_prefix(&layout.root)
            .unwrap_or(output_path)
            .display()
    ));
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
    fn resolve_output_format_returns_typed_format() {
        assert_eq!(
            resolve_output_format(None, Some(Path::new("paper.HTML"))),
            Some(OutputFormat::Html)
        );
        assert_eq!(
            resolve_output_format(Some(OutputFormat::Svg), Some(Path::new("paper.pdf"))),
            Some(OutputFormat::Svg)
        );
    }

    #[test]
    fn rejects_forwarded_typst_owned_flags() {
        for args in [
            vec!["--format".to_string(), "html".to_string()],
            vec!["--format=html".to_string()],
            vec!["--root".to_string(), ".".to_string()],
            vec!["--root=.".to_string()],
        ] {
            let err = validate_forwarded_typst_args(&args, None)
                .unwrap_err()
                .to_string();
            assert!(err.contains("managed by Calepin"), "{err}");
        }
    }

    #[test]
    fn rejects_forwarded_features_for_html_output() {
        for args in [
            vec!["--features".to_string(), "foo".to_string()],
            vec!["--features=foo".to_string()],
        ] {
            let err = validate_forwarded_typst_args(&args, Some(OutputFormat::Html))
                .unwrap_err()
                .to_string();
            assert!(err.contains("features"), "{err}");
            assert!(err.contains("HTML"), "{err}");
        }

        validate_forwarded_typst_args(&["--features=foo".to_string()], Some(OutputFormat::Pdf))
            .unwrap();
    }

    #[test]
    fn compiled_html_postprocess_errors_when_output_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(dir.path())).unwrap();
        let missing = dir.path().join("paper.html");
        let syntax_theme = HtmlSyntaxTheme::builtin();

        postprocess_html_output(&missing, &layout, None, &syntax_theme, None, false).unwrap();
        let err =
            postprocess_compiled_html_output(&missing, &layout, None, &syntax_theme, None, false)
                .unwrap_err()
                .to_string();

        assert!(err.contains("missing HTML output"), "{err}");
    }

    #[test]
    fn html_format_enables_typst_html_feature() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(dir.path())).unwrap();

        let args = typst_compile_args(
            &layout,
            Some(&PathBuf::from("paper.html")),
            Some(OutputFormat::Html),
            &[],
            ReservedInputs::default(),
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
    fn compile_args_infers_format_from_output_extension() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(dir.path())).unwrap();

        let args = typst_compile_args(
            &layout,
            Some(&PathBuf::from("paper.html")),
            None,
            &[],
            ReservedInputs::default(),
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
    fn non_html_formats_do_not_enable_typst_html_feature() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(dir.path())).unwrap();

        let args = typst_compile_args(
            &layout,
            Some(&PathBuf::from("paper.pdf")),
            Some(OutputFormat::Pdf),
            &[],
            ReservedInputs::default(),
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
        let layout = resolve_layout(&input, Some(dir.path())).unwrap();

        assert_eq!(
            resolve_output_path(
                &layout,
                Some(Path::new("out/report.pdf")),
                Some(OutputFormat::Pdf)
            ),
            layout.root.join("out/report.pdf")
        );
        assert_eq!(
            resolve_output_path(&layout, None, Some(OutputFormat::Pdf)),
            layout.root.join("paper.pdf")
        );
        assert_eq!(
            resolve_output_path(&layout, None, Some(OutputFormat::Html)),
            layout.root.join("paper.html")
        );
        assert_eq!(
            resolve_output_path(&layout, None, None),
            layout.root.join("paper.pdf")
        );
    }

    #[test]
    fn compile_args_defaults_output_to_input_stem() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(dir.path())).unwrap();

        let args = typst_compile_args(
            &layout,
            None,
            Some(OutputFormat::Pdf),
            &[],
            ReservedInputs::default(),
        );
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
        let layout = resolve_layout(&input, Some(dir.path())).unwrap();

        let args = typst_watch_args(
            &layout,
            Some(&PathBuf::from("paper.pdf")),
            Some(OutputFormat::Pdf),
            &[],
            ReservedInputs::default(),
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
    fn watch_args_infers_format_from_output_extension() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(dir.path())).unwrap();

        let args = typst_watch_args(
            &layout,
            Some(&PathBuf::from("paper.html")),
            None,
            &[],
            ReservedInputs::default(),
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
    fn watch_args_default_output_to_input_stem() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(dir.path())).unwrap();

        let args = typst_watch_args(
            &layout,
            None,
            Some(OutputFormat::Pdf),
            &[],
            ReservedInputs::default(),
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
        let layout = resolve_layout(&input, Some(dir.path())).unwrap();

        let args = typst_watch_args(
            &layout,
            Some(&PathBuf::from("paper.html")),
            Some(OutputFormat::Html),
            &[],
            ReservedInputs {
                asset_base: Some("http://127.0.0.1:3002"),
                ..ReservedInputs::default()
            },
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
