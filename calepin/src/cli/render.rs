//! The `calepin render` command: render .qmd files to output formats.

use std::path::Path;
use anyhow::{Context, Result};
use crate::cli::RenderArgs;
use crate::render::pipeline;

pub fn handle_render(args: RenderArgs) -> Result<()> {
    crate::cli::set_quiet(args.quiet);
    let mut overrides = args.overrides;
    if args.no_highlight {
        overrides.push("highlight-style=none".to_string());
    }

    let compile = args.compile;

    // Directory: look for project config inside
    if args.input.len() == 1 && args.input[0].is_dir() {
        if let Some(config) = crate::cli::find_project_config(&args.input[0]) {
            if !args.quiet {
                eprintln!("Found collection config: {}", config.display());
            }
            let output = args.output.unwrap_or_else(|| std::path::PathBuf::from("output"));
            return crate::collection::build_collection(Some(config.as_path()), &output, args.clean, args.quiet, args.format.as_deref());
        }
    }

    // Collection mode: _calepin/config.toml or _calepin/config.toml config with [[contents]]
    if args.input.len() == 1 && crate::cli::is_collection_config(&args.input[0]) {
        let output = args.output.unwrap_or_else(|| std::path::PathBuf::from("output"));
        return crate::collection::build_collection(Some(args.input[0].as_path()), &output, args.clean, args.quiet, args.format.as_deref());
    }

    // Single file: may use -o as output file path
    if args.input.len() == 1 {
        // Multi-format: split comma-separated formats and render each
        if let Some(ref fmt_str) = args.format {
            if fmt_str.contains(',') {
                let formats: Vec<&str> = fmt_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
                for f in &formats {
                    let mut ctx = crate::resolve_context(&args.input[0], Some(f))?;
                    crate::apply_writer_override(&mut ctx, args.writer.as_deref())?;
                    render_one_with_context(&args.input[0], None, &ctx, &overrides, args.quiet, compile)?;
                }
                return Ok(());
            }
        }
        let mut ctx = crate::resolve_context(&args.input[0], args.format.as_deref())?;
        crate::apply_writer_override(&mut ctx, args.writer.as_deref())?;
        return render_one_with_context(&args.input[0], args.output.as_deref(), &ctx, &overrides, args.quiet, compile);
    }

    // Multiple files: render in parallel.
    // -o is an output directory (if given); format/overrides/quiet apply to all.
    if let Some(ref out_dir) = args.output {
        std::fs::create_dir_all(out_dir)
            .with_context(|| format!("Failed to create output directory: {}", out_dir.display()))?;
    }

    // Resolve project context once and share it across all files.
    let mut ctx = crate::resolve_context(&args.input[0], args.format.as_deref())?;
    crate::apply_writer_override(&mut ctx, args.writer.as_deref())?;

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
            match render_one_with_context(input, file_output.as_deref(), &ctx, &overrides, args.quiet, compile) {
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

/// Render a single .qmd file with a pre-resolved project context.
fn render_one_with_context(
    input: &Path,
    output: Option<&Path>,
    ctx: &crate::ProjectContext,
    overrides: &[String],
    quiet: bool,
    compile: bool,
) -> Result<()> {
    let (output_path, final_output, renderer) = pipeline::render_file(
        input,
        output,
        Some(&ctx.target_name),
        overrides,
        Some(&ctx.target),
        ctx.project_root.as_deref(),
        if ctx.explicit_target { ctx.output_dir() } else { None },
        ctx.project_metadata.as_ref(),
    )?;

    renderer.write_output(&final_output, &output_path)?;

    if !quiet {
        eprintln!("-> {}", output_path.display());
    }

    // Run compile step: either explicit --compile flag, explicit compile command on target,
    // or writer extension differs from output extension (e.g., typst -> pdf).
    let needs_compile = compile
        || ctx.target.compile.is_some()
        || crate::paths::resolve_extension(&ctx.target.writer) != ctx.target.output_extension();
    if needs_compile {
        let cmd = ctx.target.compile.as_deref().unwrap_or("");
        run_compile_step(&output_path, cmd, ctx.target.output_extension(), quiet)?;
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
/// `compile_command` is the shell command template (e.g., "typst compile {input}").
/// `output_ext` is the final output extension (from target.extension).
pub fn run_compile_step(
    rendered_path: &Path,
    compile_command: &str,
    output_ext: &str,
    quiet: bool,
) -> Result<()> {
    let output_path = rendered_path.with_extension(output_ext);

    let cmd = compile_command
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
