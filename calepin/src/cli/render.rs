//! The `calepin render` command: render .qmd files to output formats.

use std::path::Path;
use anyhow::{Context, Result};
use crate::cli::RenderArgs;
use crate::render::pipeline;

pub fn handle_render(args: RenderArgs) -> Result<()> {
    crate::cli::set_quiet(args.quiet);
    crate::cli::set_debug_templates(args.debug_templates);
    let overrides = args.overrides;

    // Single input: discover project kind
    if args.input.len() == 1 {
        use crate::paths::ProjectKind;
        let input = &args.input[0];

        match ProjectKind::discover(input)? {
            ProjectKind::Collection { config, project_dir, .. } => {
                let output = args.output.unwrap_or_else(|| {
                    let meta = crate::config::load_project_metadata(&config).ok();
                    crate::paths::output_dir(&project_dir, meta.as_ref().and_then(|m| m.output.as_deref()), "index")
                });
                return crate::collection::build_collection(Some(config.as_path()), &output, args.clean, args.quiet, args.target.as_deref());
            }
            ProjectKind::Document { qmd, .. } => {
                // Multi-format: split comma-separated formats and render each
                if let Some(ref fmt_str) = args.target {
                    if fmt_str.contains(',') {
                        let formats: Vec<&str> = fmt_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
                        for f in &formats {
                            let mut ctx = crate::resolve_context(&qmd, Some(f))?;
                            crate::apply_writer_override(&mut ctx, args.writer.as_deref())?;
                            render_one_with_context(&qmd, None, &ctx, &overrides, args.quiet)?;
                        }
                        return Ok(());
                    }
                }
                let mut ctx = crate::resolve_context(&qmd, args.target.as_deref())?;
                crate::apply_writer_override(&mut ctx, args.writer.as_deref())?;
                return render_one_with_context(&qmd, args.output.as_deref(), &ctx, &overrides, args.quiet);
            }
        }
    }

    // Multiple files: render in parallel.
    // -o is an output directory (if given); format/overrides/quiet apply to all.
    if let Some(ref out_dir) = args.output {
        std::fs::create_dir_all(out_dir)
            .with_context(|| format!("Failed to create output directory: {}", out_dir.display()))?;
    }

    // Resolve project context once and share it across all files.
    let mut ctx = crate::resolve_context(&args.input[0], args.target.as_deref())?;
    crate::apply_writer_override(&mut ctx, args.writer.as_deref())?;

    let output_ext = args.output.as_ref().map(|dir| {
        (dir.clone(), ctx.target.output_extension().to_string())
    });

    use rayon::prelude::*;

    let errors: Vec<String> = args.input
        .par_iter()
        .filter_map(|input| {
            crate::paths::set_project_dir(ctx.project_root.as_deref());
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

/// Render a single .qmd file with a pre-resolved project context.
///
/// Uses the same render pipeline as collections: constructs a one-element
/// page list and renders through `render_page` + project module dispatch.
fn render_one_with_context(
    input: &Path,
    output: Option<&Path>,
    ctx: &crate::ProjectContext,
    overrides: &[String],
    quiet: bool,
) -> Result<()> {
    // Resolve output path: when the writer extension differs from the target
    // extension (e.g., typst writer -> .typ but target wants .pdf), render to
    // the writer's native extension and let post commands handle conversion.
    let writer = &ctx.target.writer;
    let ext = {
        let writer_ext = crate::paths::resolve_extension(writer);
        if writer_ext != ctx.target.output_extension() {
            writer_ext
        } else {
            ctx.target.output_extension()
        }
    };
    let (output_path, final_path) = if let Some(o) = output {
        let writer_ext = crate::paths::resolve_extension(writer);
        let o_ext = o.extension().and_then(|e| e.to_str()).unwrap_or("");
        if o_ext != writer_ext && !ctx.target.post.is_empty() {
            // User wants e.g. .pdf but writer produces .typ/.tex.
            // Write to the native extension; post commands will produce the final file.
            (o.with_extension(writer_ext), Some(o.to_path_buf()))
        } else {
            (o.to_path_buf(), None)
        }
    } else if ctx.explicit_target {
        (crate::config::resolve_target_output_path(
            input, &ctx.target_name, ext,
            ctx.project_root.as_deref(), ctx.output_dir(),
        ), None)
    } else {
        (input.with_extension(ext), None)
    };

    // Set active target and inheritance chain for template/extension resolution
    let empty_targets = std::collections::HashMap::new();
    let user_targets = ctx.project_metadata.as_ref().map(|m| &m.targets).unwrap_or(&empty_targets);
    let chain = crate::config::extension::inheritance_chain(
        ctx.project_root.as_deref().unwrap_or(std::path::Path::new(".")),
        &ctx.target_name,
        user_targets,
    );
    crate::paths::set_active_target_with_chain(Some(&ctx.target_name), chain);

    let mut all_overrides: Vec<String> = overrides.to_vec();
    all_overrides.extend(crate::collection::render::build_overrides(Some(&ctx.target)));

    // Render through the shared pipeline (render_core + assemble_page + transform_document)
    let (page, _result) = pipeline::render_page(
        input, &output_path, writer,
        &all_overrides,
        ctx.project_root.as_deref(),
        &pipeline::RenderCoreOptions::default(),
        ctx.project_metadata.as_ref(),
        Some(&ctx.target),
    )?;

    // Run project-level modules from extensions
    let mut final_body = page.body;
    {
        let project_root = ctx.project_root.as_deref().unwrap_or(Path::new("."));
        let registry = crate::registry::ModuleRegistry::load(&ctx.target.modules, project_root);
        let project_ctx = crate::registry::ProjectTransformContext {
            base_dir: project_root.to_path_buf(),
            output_dir: output_path.parent().unwrap_or(Path::new(".")).to_path_buf(),
            target_name: ctx.target_name.clone(),
        };
        let meta = ctx.project_metadata.clone().unwrap_or_default();
        let mut pages = vec![crate::registry::RenderedPage { body: final_body, ..page }];
        if let Err(e) = registry.run_project_transforms(&mut pages, &meta, writer, &project_ctx, &ctx.target.modules) {
            cwarn!("project module error: {}", e);
        }
        final_body = pages.into_iter().next().map(|p| p.body).unwrap_or_default();
    }

    // Write output
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output_path, &final_body)?;

    if !quiet {
        eprintln!("-> {}", output_path.display());
    }

    // Run target-level post-processing commands
    if !ctx.target.post.is_empty() {
        // Copy the figures directory next to the output so LaTeX/Typst can find it
        let input_stem = input.file_stem().unwrap_or_default().to_string_lossy();
        let files_dir_name = format!("{}_files", input_stem);
        let src_files = input.parent().unwrap_or(Path::new(".")).join(&files_dir_name);
        let dst_files = output_path.parent().unwrap_or(Path::new(".")).join(&files_dir_name);
        let copied_files = if src_files.is_dir() && !dst_files.exists() {
            crate::paths::copy_dir_recursive(&src_files, &dst_files).ok();
            true
        } else {
            false
        };

        let root = ctx.project_root.as_deref()
            .unwrap_or_else(|| output_path.parent().unwrap_or(Path::new(".")));
        run_target_post_commands(&ctx.target.post, &output_path, root, quiet)?;

        // Clean up copied figures directory
        if copied_files {
            std::fs::remove_dir_all(&dst_files).ok();
        }
    }

    // When -o requested a different extension (e.g. .pdf) than the writer
    // produced (e.g. .typ), the post command should have created the final
    // file next to the intermediate. Move it to the requested -o path.
    if let Some(final_path) = final_path {
        let produced = output_path.with_extension(
            final_path.extension().and_then(|e| e.to_str()).unwrap_or(""),
        );
        if produced.exists() {
            if produced != final_path {
                std::fs::rename(&produced, &final_path)?;
            }
            // Clean up the intermediate file only on success
            let _ = std::fs::remove_file(&output_path);
        }
    }

    Ok(())
}

/// Run target-level post-processing commands.
///
/// Each command supports these placeholders:
/// - `{input}` -- the rendered file path (e.g., `file.typ`)
/// - `{output}` -- alias for `{input}` (for backward compatibility)
/// - `{dir}` -- parent directory of the rendered file
/// - `{root}` -- the project root directory
pub fn run_target_post_commands(
    commands: &[String],
    output: &Path,
    project_root: &Path,
    quiet: bool,
) -> Result<()> {
    // Use absolute path so post commands work regardless of working_dir
    let output_abs = if output.is_absolute() {
        output.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(output)
    };
    let working_dir = output_abs.parent().unwrap_or(project_root);
    let dir = working_dir.display().to_string();
    for command in commands {
        let cmd = command
            .replace("{input}", &output_abs.display().to_string())
            .replace("{output}", &output_abs.display().to_string())
            .replace("{dir}", &dir)
            .replace("{root}", &project_root.display().to_string());
        run_shell_command(&cmd, working_dir, quiet);
    }
    Ok(())
}

/// Run a single shell command, printing status and warnings.
pub fn run_shell_command(cmd: &str, working_dir: &Path, quiet: bool) {
    if !quiet {
        eprintln!("  \x1b[36mpost:\x1b[0m {}", cmd);
    }
    match std::process::Command::new("sh").arg("-c").arg(cmd).current_dir(working_dir).output() {
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
