//! `calepin theme` -- apply, list, or preview themes.

use std::path::Path;

use anyhow::Result;

use crate::config::paths::ProjectKind;
use crate::themes::{Theme, FileAction};

/// Handle `calepin theme list`.
pub fn handle_theme_list() -> Result<()> {
    let themes = Theme::list_builtin();
    eprintln!("Available themes:\n");
    for t in &themes {
        eprintln!("  {:<16} {}", t.name, t.description);
    }
    Ok(())
}

/// Handle `calepin theme show` (dry run).
pub fn handle_theme_show(name: &str, path: &Path) -> Result<()> {
    let theme = resolve_theme(name)?;
    let kind = ProjectKind::discover(path)?;
    let plan = theme.plan(&kind);

    print_plan(&theme, &kind, &plan);

    if !plan.has_changes() {
        eprintln!("Nothing to do.");
    }

    Ok(())
}

/// Handle `calepin theme apply`.
pub fn handle_theme_apply(name: &str, path: &Path, yes: bool) -> Result<()> {
    let theme = resolve_theme(name)?;
    let kind = ProjectKind::discover(path)?;

    // Show plan and confirm
    let plan = theme.plan(&kind);

    if !plan.has_changes() {
        eprintln!("Theme \"{}\" is already applied. Nothing to do.", theme.manifest.name);
        return Ok(());
    }

    print_plan(&theme, &kind, &plan);

    if !yes {
        eprint!("Proceed? [y/N] ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            eprintln!("Aborted.");
            return Ok(());
        }
    }

    let result = theme.apply(&kind)?;

    let created = result.created().len();
    let overwritten = result.overwritten().len();
    let backed_up = result.overwritten().len();

    eprintln!(
        "Applied theme \"{}\": {} created, {} overwritten{}.",
        theme.manifest.name,
        created,
        overwritten,
        if backed_up > 0 {
            format!(" ({} backed up as .bak)", backed_up)
        } else {
            String::new()
        },
    );

    Ok(())
}

/// Resolve a theme by name (built-in) or path (local directory).
fn resolve_theme(name: &str) -> Result<Theme> {
    // Try built-in first
    if let Some(theme) = Theme::builtin(name) {
        return Ok(theme);
    }

    // Try as a local path
    let path = Path::new(name);
    if path.is_dir() {
        return Theme::from_path(path);
    }

    anyhow::bail!(
        "Unknown theme: \"{}\". Run `calepin theme list` to see available themes.",
        name
    );
}

/// Print a human-readable plan summary.
fn print_plan(theme: &Theme, kind: &ProjectKind, plan: &crate::themes::ApplyPlan) {
    let target_desc = match kind {
        ProjectKind::Document { qmd, .. } => format!("document {}", qmd.display()),
        ProjectKind::Collection { root, .. } => format!("collection at {}", root.display()),
    };

    eprintln!("Applying theme \"{}\" to {}\n", theme.manifest.name, target_desc);

    let overwritten = plan.overwritten();
    if !overwritten.is_empty() {
        eprintln!("  Overwrite ({} file{}):", overwritten.len(), if overwritten.len() == 1 { "" } else { "s" });
        for p in &overwritten {
            eprintln!("    {}", p.display());
        }
        eprintln!();
    }

    let created = plan.created();
    if !created.is_empty() {
        eprintln!("  Create ({} file{}):", created.len(), if created.len() == 1 { "" } else { "s" });
        for p in &created {
            eprintln!("    {}", p.display());
        }
        eprintln!();
    }

    let unchanged = plan.unchanged_count();
    if unchanged > 0 {
        eprintln!("  Unchanged: {} file{}", unchanged, if unchanged == 1 { "" } else { "s" });
        eprintln!();
    }

    // Back up count
    let backup_count = plan.actions.iter()
        .filter(|(_, a, _)| matches!(a, FileAction::Overwrite))
        .count();
    if backup_count > 0 {
        eprintln!("  Backup: {} conflicting file{} will be saved as .bak", backup_count, if backup_count == 1 { "" } else { "s" });
        eprintln!();
    }

    if let Some(ref _frag) = plan.config_fragment {
        eprintln!("  Config: config.toml fragment will be merged");
        eprintln!();
    }
}
