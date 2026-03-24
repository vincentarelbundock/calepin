//! The `calepin render` command: render .qmd files to output formats.

use std::path::Path;
use anyhow::{Context, Result};
use crate::cli::RenderArgs;
use crate::pipeline;

pub fn handle_render(args: RenderArgs) -> Result<()> {
    crate::cli::set_quiet(args.quiet);
    let mut overrides = args.overrides;
    if args.no_highlight {
        overrides.push("highlight-style=none".to_string());
    }

    // Collection mode: single .toml config with [[contents]], or legacy .yaml manifest
    if args.input.len() == 1 && crate::cli::is_collection_config(&args.input[0]) {
        let output = args.output.unwrap_or_else(|| std::path::PathBuf::from("output"));
        return crate::collection::build_collection(Some(args.input[0].as_path()), &output, args.clean, args.quiet, args.target.as_deref());
    }

    // Single file: may use -o as output file path
    if args.input.len() == 1 {
        // Multi-target: split comma-separated targets and render each
        if let Some(ref target_str) = args.target {
            if target_str.contains(',') {
                let targets: Vec<&str> = target_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
                for t in &targets {
                    render_one(&args.input[0], None, Some(t), &overrides, args.quiet, args.engine.as_deref())?;
                }
                return Ok(());
            }
        }
        return render_one(&args.input[0], args.output.as_deref(), args.target.as_deref(), &overrides, args.quiet, args.engine.as_deref());
    }

    // Multiple files: render in parallel.
    // -o is an output directory (if given); target/overrides/quiet apply to all.
    if let Some(ref out_dir) = args.output {
        std::fs::create_dir_all(out_dir)
            .with_context(|| format!("Failed to create output directory: {}", out_dir.display()))?;
    }

    // Resolve project context once and share it across all files.
    let mut ctx = crate::resolve_context(&args.input[0], args.target.as_deref())?;
    crate::apply_engine_override(&mut ctx, args.engine.as_deref())?;

    let output_ext = args.output.as_ref().map(|dir| {
        (dir.clone(), ctx.target.output_extension().to_string())
    });

    use rayon::prelude::*;

    let errors: Vec<String> = args.input
        .par_iter()
        .filter_map(|input| {
            crate::paths::set_project_root(ctx.project_root.as_deref());
            let file_output = output_ext.as_ref().map(|(dir, ext)| {
                dir.join(input.file_name().unwrap()).with_extension(ext)
            });
            match render_one_with_context(input, file_output.as_deref(), &ctx, &overrides, args.quiet) {
                Ok(()) => None,
                Err(e) => Some(format!("{:#}", e)),
            }
        })
        .collect();

    if !errors.is_empty() {
        for e in &errors {
            eprintln!("\x1b[31mError:\x1b[0m {}", e);
        }
        anyhow::bail!("{} of {} files failed to render", errors.len(), args.input.len());
    }

    Ok(())
}

/// Render a single .qmd file.
fn render_one(
    input: &Path,
    output: Option<&Path>,
    target: Option<&str>,
    overrides: &[String],
    quiet: bool,
    engine_override: Option<&str>,
) -> Result<()> {
    let mut ctx = crate::resolve_context(input, target)?;
    crate::apply_engine_override(&mut ctx, engine_override)?;
    render_one_with_context(input, output, &ctx, overrides, quiet)
}

/// Render a single .qmd file with a pre-resolved project context.
fn render_one_with_context(
    input: &Path,
    output: Option<&Path>,
    ctx: &crate::ProjectContext,
    overrides: &[String],
    quiet: bool,
) -> Result<()> {
    let (output_path, final_output, renderer) = pipeline::render_file(
        input,
        output,
        Some(&ctx.target_name),
        overrides,
        Some(&ctx.target),
        ctx.project_root.as_deref(),
        ctx.project_var(),
        if ctx.explicit_target { ctx.output_dir() } else { None },
    )?;

    renderer.write_output(&final_output, &output_path)?;

    if !quiet {
        eprintln!("-> {}", output_path.display());
    }

    // Run compile step if the target defines one
    if let Some(ref compile_cfg) = ctx.target.compile {
        run_compile_step(&output_path, compile_cfg, quiet)?;
    }

    // Run target-level post-processing commands
    if !ctx.target.post.is_empty() {
        let root = ctx.project_root.as_deref()
            .unwrap_or_else(|| output_path.parent().unwrap_or(Path::new(".")));
        run_target_post_commands(&ctx.target.post, &output_path, root, quiet)?;
    }

    Ok(())
}

/// Run a target's compile step.
///
/// If no command is specified and the input is a `.typ` file, uses the
/// built-in Typst compiler (no external binary needed). Otherwise shells
/// out to the configured command.
pub fn run_compile_step(
    rendered_path: &Path,
    compile_cfg: &crate::project::CompileConfig,
    quiet: bool,
) -> Result<()> {
    let compile_ext = compile_cfg.extension.as_deref()
        .ok_or_else(|| anyhow::anyhow!("Target compile section has no extension"))?;
    let output_path = rendered_path.with_extension(compile_ext);

    // Native Typst compilation when no command override is set.
    if compile_cfg.command.is_none()
        && rendered_path.extension().is_some_and(|e| e == "typ")
    {
        if !quiet {
            eprintln!("  compiling: {} -> {}", rendered_path.display(), output_path.display());
        }
        crate::typst_compile::compile_typst_to_pdf(rendered_path, &output_path)?;
        if !quiet {
            eprintln!("-> {}", output_path.display());
        }
        return Ok(());
    }

    let command = compile_cfg.command.as_deref()
        .ok_or_else(|| anyhow::anyhow!("Target compile section has no command"))?;

    let cmd = command
        .replace("{input}", &rendered_path.to_string_lossy())
        .replace("{output}", &output_path.to_string_lossy());

    if !quiet {
        eprintln!("  compiling: {}", cmd);
    }

    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .status()
        .with_context(|| format!("Failed to run compile command: {}", cmd))?;

    if !status.success() {
        anyhow::bail!("Compile command failed: {}", cmd);
    }

    if !quiet {
        eprintln!("-> {}", output_path.display());
    }

    Ok(())
}

/// Run target-level post-processing commands.
///
/// Each command supports `{output}` (rendered file path) and `{root}` (project root).
fn run_target_post_commands(
    commands: &[String],
    output: &Path,
    project_root: &Path,
    quiet: bool,
) -> Result<()> {
    for command in commands {
        let cmd = command
            .replace("{output}", &output.display().to_string())
            .replace("{root}", &project_root.display().to_string());

        if !quiet {
            eprintln!("  post: {}", cmd);
        }

        let result = std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .current_dir(project_root)
            .output();

        match result {
            Ok(out) => {
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    cwarn!("post command failed: {}", cmd);
                    if !stderr.trim().is_empty() {
                        eprintln!("  {}", stderr.trim());
                    }
                }
            }
            Err(e) => {
                cwarn!("failed to run post command: {}: {}", cmd, e);
            }
        }
    }
    Ok(())
}
