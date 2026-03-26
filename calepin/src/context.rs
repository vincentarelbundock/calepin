//! Runtime project context: resolves project config and target for a render.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::paths;
use crate::config;

/// Resolved project context: project metadata + target, shared by render and preview.
pub(crate) struct ProjectContext {
    pub project_root: Option<PathBuf>,
    pub project_metadata: Option<crate::config::Metadata>,
    pub target_name: String,
    pub target: config::Target,
    /// True when the target was explicitly set (CLI flag or front matter),
    /// false when it fell back to the default "html".
    pub explicit_target: bool,
}

impl ProjectContext {
    /// Get the configured output directory, if any.
    pub fn output_dir(&self) -> Option<&str> {
        self.project_metadata.as_ref().and_then(|m| m.output.as_deref())
    }
}

/// Resolve project config and target from an input file and optional CLI flags.
/// Falls back to front matter `target:`, then "html".
pub(crate) fn resolve_context(input: &Path, cli_target: Option<&str>) -> Result<ProjectContext> {
    let input_dir = input.parent().unwrap_or(Path::new("."));
    let abs_input_dir = if input_dir.is_relative() {
        std::env::current_dir().unwrap_or_default().join(input_dir)
    } else {
        input_dir.to_path_buf()
    };

    // Project root is the directory containing the input file.
    // Load config and convert to Metadata immediately.
    let (project_root, project_metadata) = {
        if let Some(cfg_path) = crate::cli::find_project_config(&abs_input_dir) {
            match config::load_project_metadata(&cfg_path) {
                Ok(meta) => (Some(abs_input_dir.clone()), Some(meta)),
                Err(e) => {
                    eprintln!("Warning: failed to load {}: {}", cfg_path.display(), e);
                    (Some(abs_input_dir.clone()), None)
                }
            }
        } else {
            (None, None)
        }
    };

    // Target name: CLI flag -> default from config
    let default_format = project_metadata.as_ref()
        .and_then(|m| m.target.clone())
        .unwrap_or_else(|| "html".to_string());
    let (target_name, explicit_target) = if let Some(name) = cli_target {
        (name.to_string(), true)
    } else {
        (default_format.clone(), false)
    };

    let empty_targets = std::collections::HashMap::new();
    let user_targets = project_metadata.as_ref().map(|m| &m.targets).unwrap_or(&empty_targets);
    let target = config::resolve_target(&target_name, user_targets)?;

    // In document mode (no _calepin/config.toml), the project root is the
    // input file's parent directory so that all paths resolve relative to it.
    let effective_root = project_root.clone().unwrap_or_else(|| abs_input_dir.clone());

    // Warn when document mode root differs from cwd (e.g., `calepin render subdir/doc.qmd`)
    if project_root.is_none() && !crate::cli::is_quiet() {
        if let Ok(cwd) = std::env::current_dir() {
            if cwd != effective_root {
                eprintln!(
                    "Note: project root is {} (input file directory, no _calepin/config.toml found)",
                    effective_root.display()
                );
            }
        }
    }

    paths::set_project_root(Some(&effective_root));

    Ok(ProjectContext {
        project_root: Some(effective_root),
        project_metadata,
        target_name,
        target,
        explicit_target,
    })
}

/// Apply `--writer` override to a resolved project context.
///
/// Validates that the writer is allowed for the target:
///   - `pdf`: html, latex, typst, markdown
///   - `book`: latex, typst
///   - others: no override allowed (writer is fixed)
pub(crate) fn apply_writer_override(ctx: &mut ProjectContext, writer: Option<&str>) -> Result<()> {
    let Some(writer) = writer else { return Ok(()) };

    let allowed: &[&str] = match ctx.target_name.as_str() {
        "pdf" => &["html", "latex", "typst", "markdown"],
        "book" => &["latex", "typst"],
        other => anyhow::bail!(
            "--writer is only valid for pdf or book targets (got '{}')", other
        ),
    };

    if !allowed.contains(&writer) {
        anyhow::bail!(
            "--writer '{}' is not valid for target '{}'. Allowed: {}",
            writer, ctx.target_name, allowed.join(", ")
        );
    }

    ctx.target.writer = writer.to_string();

    // Update extension and fig-extension to match the new writer
    let builtin = config::builtin_metadata().targets.get(writer);
    if let Some(b) = builtin {
        ctx.target.extension = b.extension.clone();
        ctx.target.fig_extension = b.fig_extension.clone();
        ctx.target.compile = b.compile.clone();
        ctx.target.preview = b.preview.clone();
    }

    Ok(())
}
