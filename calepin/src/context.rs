//! Runtime project context: resolves project config, target, and theme for a render.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::{paths, project, theme_manifest};

/// Resolved project context: project metadata + target, shared by render and preview.
pub(crate) struct ProjectContext {
    pub project_root: Option<PathBuf>,
    pub project_metadata: Option<crate::metadata::Metadata>,
    pub target_name: String,
    pub target: project::Target,
    /// True when the target was explicitly set (CLI flag or front matter),
    /// false when it fell back to the default "html".
    pub explicit_target: bool,
    /// Active theme name, if any.
    pub theme_name: Option<String>,
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
    resolve_context_with_theme(input, cli_target, None)
}

/// Resolve project config, target, and theme from an input file and optional CLI flags.
pub(crate) fn resolve_context_with_theme(input: &Path, cli_target: Option<&str>, cli_theme: Option<&str>) -> Result<ProjectContext> {
    let input_dir = input.parent().unwrap_or(Path::new("."));
    let abs_input_dir = if input_dir.is_relative() {
        std::env::current_dir().unwrap_or_default().join(input_dir)
    } else {
        input_dir.to_path_buf()
    };

    // Project root is the directory containing the input file.
    // Load config and convert to Metadata immediately.
    let (project_root, project_metadata) = {
        let cfg_path = abs_input_dir.join("_calepin.toml");
        if cfg_path.exists() {
            match project::load_project_metadata(&cfg_path) {
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

    // Read front matter once (used for target and theme resolution)
    let front_meta = fs::read_to_string(input).ok()
        .and_then(|text| crate::metadata::split_frontmatter(&text).ok())
        .map(|(meta, _)| meta);

    // Target name: CLI flag -> front matter -> default from config
    let default_format = project::get_defaults().format.clone().unwrap_or_else(|| "html".to_string());
    let (target_name, explicit_target) = if let Some(name) = cli_target {
        (name.to_string(), true)
    } else {
        match front_meta.as_ref().and_then(|m| m.target.clone()) {
            Some(t) => (t, true),
            None => (default_format.clone(), false),
        }
    };

    let empty_targets = std::collections::HashMap::new();
    let user_targets = project_metadata.as_ref().map(|m| &m.targets).unwrap_or(&empty_targets);
    let target = project::resolve_target(&target_name, user_targets)?;

    let mut defaults = project::resolve_defaults(project_metadata.as_ref().map(|m| &m.defaults));
    if let Some(embed) = target.embed_resources {
        defaults.embed_resources = Some(embed);
    }
    project::set_active_defaults(defaults);

    // In document mode (no _calepin.toml), the project root is the
    // input file's parent directory so that all paths resolve relative to it.
    let effective_root = project_root.clone().unwrap_or_else(|| abs_input_dir.clone());

    // Warn when document mode root differs from cwd (e.g., `calepin render subdir/doc.qmd`)
    if project_root.is_none() && !crate::cli::is_quiet() {
        if let Ok(cwd) = std::env::current_dir() {
            if cwd != effective_root {
                eprintln!(
                    "Note: project root is {} (input file directory, no _calepin.toml found)",
                    effective_root.display()
                );
            }
        }
    }

    paths::set_project_root(Some(&effective_root));

    // Resolve theme: CLI flag -> front matter -> project config
    let theme_name = cli_theme.map(|s| s.to_string())
        .or_else(|| front_meta.as_ref().and_then(|m| m.theme.clone()));

    // If theme is active, set theme dir for template resolution
    if let Some(ref theme) = theme_name {
        if let Some(theme_dir) = theme_manifest::resolve_theme_dir(theme, &effective_root) {
            paths::set_theme_dir(Some(&theme_dir));
        }
    }

    Ok(ProjectContext {
        project_root: Some(effective_root),
        project_metadata,
        target_name,
        target,
        explicit_target,
        theme_name,
    })
}

/// Apply `--engine` override to a resolved project context.
///
/// Validates that the engine is allowed for the target:
///   - `pdf`: html, latex, typst, markdown
///   - `book`: latex, typst
///   - others: no override allowed (engine is fixed)
pub(crate) fn apply_engine_override(ctx: &mut ProjectContext, engine: Option<&str>) -> Result<()> {
    let Some(engine) = engine else { return Ok(()) };

    let allowed: &[&str] = match ctx.target_name.as_str() {
        "pdf" => &["html", "latex", "typst", "markdown"],
        "book" => &["latex", "typst"],
        other => anyhow::bail!(
            "--engine is only valid for pdf or book targets (got '{}')", other
        ),
    };

    if !allowed.contains(&engine) {
        anyhow::bail!(
            "--engine '{}' is not valid for target '{}'. Allowed: {}",
            engine, ctx.target_name, allowed.join(", ")
        );
    }

    ctx.target.engine = engine.to_string();

    // Update extension and fig-extension to match the new engine
    let builtin = project::builtin_metadata().targets.get(engine);
    if let Some(b) = builtin {
        ctx.target.extension = b.extension.clone();
        ctx.target.fig_extension = b.fig_extension.clone();
        ctx.target.compile = b.compile.clone();
        ctx.target.preview = b.preview.clone();
    }

    Ok(())
}
