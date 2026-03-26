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

    let compile = args.compile;

    // Collection mode: _calepin.toml config with [[contents]]
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
                    crate::apply_engine_override(&mut ctx, args.engine.as_deref())?;
                    render_one_with_context(&args.input[0], None, &ctx, &overrides, args.quiet, compile)?;
                }
                return Ok(());
            }
        }
        let mut ctx = crate::resolve_context(&args.input[0], args.format.as_deref())?;
        crate::apply_engine_override(&mut ctx, args.engine.as_deref())?;
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

    // Run compile step if --compile was passed and the format defines one
    if compile {
        if let Some(ref compile_cmd) = ctx.target.compile {
            run_compile_step(&output_path, compile_cmd, ctx.target.output_extension(), quiet)?;
        }
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
/// `compile_command` is the shell command template (e.g., "tectonic {input}").
/// `output_ext` is the final output extension (from target.extension).
/// If the rendered file is `.typ` and no command is given, uses the built-in Typst compiler.
pub fn run_compile_step(
    rendered_path: &Path,
    compile_command: &str,
    output_ext: &str,
    quiet: bool,
) -> Result<()> {
    let output_path = rendered_path.with_extension(output_ext);

    // Native Typst compilation when no explicit command is given.
    if compile_command.is_empty()
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
