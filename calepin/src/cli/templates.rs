//! `calepin templates` subcommand: list, eject, show, diff, update, reset.
//!
//! All subcommands operate on a specific sidecar identified by the input .qmd file.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use xxhash_rust::xxh3::xxh3_64;

use crate::cli::TemplatesAction;
use crate::render::elements::{BUILTIN_TEMPLATES, resolve_builtin_template};

// ---------------------------------------------------------------------------
// Hash utilities
// ---------------------------------------------------------------------------

fn compute_hash(content: &[u8]) -> String {
    format!("{:016x}", xxh3_64(content))
}

const MARKER_PREFIX: &str = "{# calepin:xxh3:";
const MARKER_SUFFIX: &str = " #}";

fn parse_hash_marker(text: &str) -> Option<String> {
    let first_line = text.lines().next()?;
    let rest = first_line.strip_prefix(MARKER_PREFIX)?;
    let hash = rest.strip_suffix(MARKER_SUFFIX)?;
    Some(hash.to_string())
}

fn strip_hash_marker(text: &str) -> &str {
    if text.starts_with(MARKER_PREFIX) {
        text.find('\n').map(|i| &text[i + 1..]).unwrap_or("")
    } else {
        text
    }
}

enum TemplateState {
    Unmodified,
    Modified,
    NoMarker,
}

fn check_template_state(local_content: &str) -> TemplateState {
    match parse_hash_marker(local_content) {
        Some(marker_hash) => {
            let body = strip_hash_marker(local_content);
            let current_hash = compute_hash(body.as_bytes());
            if current_hash == marker_hash {
                TemplateState::Unmodified
            } else {
                TemplateState::Modified
            }
        }
        None => TemplateState::NoMarker,
    }
}

// ---------------------------------------------------------------------------
// Context: resolve sidecar + target from input .qmd
// ---------------------------------------------------------------------------

struct TemplateContext {
    /// The flat templates directory: {stem}_calepin/templates/{target}/
    tpl_dir: PathBuf,
    /// Sidecar root: {stem}_calepin/
    sidecar: PathBuf,
    target_name: String,
    writer: String,
    ext: String,
    chain: Vec<String>,
}

/// Resolve the sidecar and target from an input .qmd file.
fn resolve_context(input: &Path, target_override: Option<&str>) -> Result<TemplateContext> {
    if !input.exists() {
        bail!("File not found: {}", input.display());
    }
    let stem = input.file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("Cannot determine stem for: {}", input.display()))?;
    let parent = input.parent().unwrap_or(Path::new("."));
    let sidecar = parent.join(format!("{}_calepin", stem));

    // Read target from front matter if not overridden
    let target_name = if let Some(t) = target_override {
        t.to_string()
    } else {
        let text = std::fs::read_to_string(input)?;
        crate::config::split_frontmatter(&text)
            .ok()
            .and_then(|(fm, _)| fm.target)
            .unwrap_or_else(|| "html".to_string())
    };

    let empty = std::collections::HashMap::new();
    let target = crate::config::resolve_target(&target_name, &empty).ok();
    let writer = target.as_ref().map(|t| t.writer.as_str()).unwrap_or(&target_name).to_string();
    let ext = crate::paths::resolve_extension(&writer).to_string();
    let project_root = parent.to_path_buf();
    let chain = crate::config::extension::inheritance_chain(&project_root, &target_name, &empty);
    crate::paths::set_active_target_with_chain(Some(&target_name), chain.clone());

    let tpl_dir = sidecar.join("templates").join(&target_name);

    Ok(TemplateContext { tpl_dir, sidecar, target_name, writer, ext, chain })
}

/// Collect all template names from built-in templates for the given inheritance chain.
fn collect_builtin_names(chain: &[String]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for chain_target in chain {
        if let Some(dir) = BUILTIN_TEMPLATES.get_dir(chain_target.as_str()) {
            for file in dir.files() {
                if let Some(stem) = file.path().file_stem().and_then(|s| s.to_str()) {
                    if !names.contains(&stem.to_string()) {
                        names.push(stem.to_string());
                    }
                }
            }
        }
    }
    names.sort();
    names
}

// ---------------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------------

pub fn handle_templates(action: TemplatesAction) -> Result<()> {
    match action {
        TemplatesAction::List { input, target } => handle_list(&input, target.as_deref()),
        TemplatesAction::Eject { input, target, force, dry_run } => handle_eject(&input, target.as_deref(), force, dry_run),
        TemplatesAction::Show { name, input, target } => handle_show(&name, &input, target.as_deref()),
        TemplatesAction::Diff { input, target } => handle_diff(&input, target.as_deref()),
        TemplatesAction::Update { input, target, force, dry_run } => handle_update(&input, target.as_deref(), force, dry_run),
        TemplatesAction::Reset { name, input, target } => handle_reset(&name, &input, target.as_deref()),
    }
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

fn handle_list(input: &Path, target: Option<&str>) -> Result<()> {
    let ctx = resolve_context(input, target)?;
    let names = collect_builtin_names(&ctx.chain);

    println!("Templates for '{}' (target: {}, writer: {}):\n",
        input.display(), ctx.target_name, ctx.writer);

    for name in &names {
        let specific = format!("{}.{}", name, ctx.ext);
        let path = ctx.tpl_dir.join(&specific);
        if path.exists() {
            let rel = path.strip_prefix(&ctx.sidecar).unwrap_or(&path);
            let state_label = match std::fs::read_to_string(&path) {
                Ok(content) => match check_template_state(&content) {
                    TemplateState::Unmodified => "\x1b[32mlocal\x1b[0m",
                    TemplateState::Modified => "\x1b[33mmodified\x1b[0m",
                    TemplateState::NoMarker => "\x1b[33mlocal (no marker)\x1b[0m",
                },
                Err(_) => "\x1b[31merror\x1b[0m",
            };
            println!("  {}  {}  {}", specific, state_label, rel.display());
        } else if resolve_builtin_template(name, &ctx.writer).is_some() {
            println!("  {}  \x1b[36mnot ejected\x1b[0m", specific);
        }
    }

    if ctx.tpl_dir.is_dir() {
        println!("\nTemplates: {}", ctx.tpl_dir.display());
    } else {
        println!("\nNo templates directory yet. Run `calepin templates eject` to create it.");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// eject
// ---------------------------------------------------------------------------

fn handle_eject(input: &Path, target: Option<&str>, force: bool, dry_run: bool) -> Result<()> {
    let ctx = resolve_context(input, target)?;

    let mut written = 0;
    // Child-first: child templates take priority, parents fill gaps
    for chain_target in &ctx.chain {
        if let Some(dir) = BUILTIN_TEMPLATES.get_dir(chain_target.as_str()) {
            written += eject_dir_flat(dir, chain_target, &ctx.tpl_dir, force, dry_run)?;
        }
    }

    if dry_run {
        eprintln!("Dry run: would write {} file(s) to {}", written, ctx.tpl_dir.display());
    } else if written > 0 {
        eprintln!("Ejected {} template(s) to {}", written, ctx.tpl_dir.display());
    } else {
        eprintln!("All templates already exist (use --force to overwrite).");
    }

    Ok(())
}

/// Eject files from a built-in directory into a flat destination.
fn eject_dir_flat(
    dir: &include_dir::Dir<'static>,
    prefix: &str,
    dest: &Path,
    force: bool,
    dry_run: bool,
) -> Result<usize> {
    let mut written = 0;
    let prefix_path = std::path::Path::new(prefix);

    for file in dir.files() {
        let rel = file.path().strip_prefix(prefix_path).unwrap_or(file.path());
        let out = dest.join(rel);
        if out.exists() && !force {
            continue;
        }
        if let Some(content) = file.contents_utf8() {
            if dry_run {
                eprintln!("  would write: {}", rel.display());
            } else {
                if let Some(parent) = out.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&out, crate::paths::prepend_hash_marker(content))?;
            }
            written += 1;
        }
    }

    for subdir in dir.dirs() {
        let sub_rel = subdir.path().strip_prefix(prefix_path).unwrap_or(subdir.path());
        written += eject_subdir_flat(subdir, &dest.join(sub_rel), force, dry_run)?;
    }

    Ok(written)
}

fn eject_subdir_flat(
    dir: &include_dir::Dir<'static>,
    dest: &Path,
    force: bool,
    dry_run: bool,
) -> Result<usize> {
    let mut written = 0;

    for file in dir.files() {
        let name = file.path().file_name().unwrap_or_default();
        let out = dest.join(name);
        if out.exists() && !force {
            continue;
        }
        if let Some(content) = file.contents_utf8() {
            if !dry_run {
                if let Some(parent) = out.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&out, crate::paths::prepend_hash_marker(content))?;
            }
            written += 1;
        }
    }

    for subdir in dir.dirs() {
        let sub_name = subdir.path().file_name().unwrap_or_default();
        written += eject_subdir_flat(subdir, &dest.join(sub_name), force, dry_run)?;
    }

    Ok(written)
}

// ---------------------------------------------------------------------------
// show
// ---------------------------------------------------------------------------

fn handle_show(name: &str, input: &Path, target: Option<&str>) -> Result<()> {
    let ctx = resolve_context(input, target)?;
    let specific = format!("{}.{}", name, ctx.ext);
    let path = ctx.tpl_dir.join(&specific);

    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        print!("{}", strip_hash_marker(&content));
        return Ok(());
    }

    // Show the built-in version if not ejected
    if let Some(content) = resolve_builtin_template(name, &ctx.writer) {
        print!("{}", content);
        return Ok(());
    }

    bail!("Template '{}' not found for target '{}'", name, ctx.target_name);
}

// ---------------------------------------------------------------------------
// diff
// ---------------------------------------------------------------------------

fn handle_diff(input: &Path, target: Option<&str>) -> Result<()> {
    let ctx = resolve_context(input, target)?;
    let names = collect_builtin_names(&ctx.chain);
    let mut found_diff = false;

    for name in &names {
        let specific = format!("{}.{}", name, ctx.ext);
        let path = ctx.tpl_dir.join(&specific);
        if !path.exists() { continue; }

        let local_content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let local_body = strip_hash_marker(&local_content);

        let builtin = match resolve_builtin_template(name, &ctx.writer) {
            Some(b) => b,
            None => continue,
        };

        if local_body == builtin { continue; }

        found_diff = true;
        println!("\x1b[1m--- built-in: {}\x1b[0m", specific);
        println!("\x1b[1m+++ local:    {}\x1b[0m", specific);

        let builtin_lines: Vec<&str> = builtin.lines().collect();
        let local_lines: Vec<&str> = local_body.lines().collect();
        let max = builtin_lines.len().max(local_lines.len());

        for i in 0..max {
            match (builtin_lines.get(i), local_lines.get(i)) {
                (Some(b), Some(l)) if b == l => println!(" {}", b),
                (Some(b), Some(l)) => {
                    println!("\x1b[31m-{}\x1b[0m", b);
                    println!("\x1b[32m+{}\x1b[0m", l);
                }
                (Some(b), None) => println!("\x1b[31m-{}\x1b[0m", b),
                (None, Some(l)) => println!("\x1b[32m+{}\x1b[0m", l),
                (None, None) => {}
            }
        }
        println!();
    }

    if !found_diff {
        eprintln!("No differences found.");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// update
// ---------------------------------------------------------------------------

fn handle_update(input: &Path, target: Option<&str>, force: bool, dry_run: bool) -> Result<()> {
    let ctx = resolve_context(input, target)?;
    let names = collect_builtin_names(&ctx.chain);
    let mut updated = 0;
    let mut skipped = 0;

    for name in &names {
        let specific = format!("{}.{}", name, ctx.ext);
        let path = ctx.tpl_dir.join(&specific);
        if !path.exists() { continue; }

        let local_content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let builtin = match resolve_builtin_template(name, &ctx.writer) {
            Some(b) => b,
            None => continue,
        };

        let local_body = strip_hash_marker(&local_content);
        if local_body == builtin { continue; }

        let state = check_template_state(&local_content);
        match state {
            TemplateState::Unmodified => {
                if dry_run {
                    eprintln!("  would update: {}", specific);
                } else {
                    std::fs::write(&path, crate::paths::prepend_hash_marker(builtin))?;
                    eprintln!("  updated: {}", specific);
                }
                updated += 1;
            }
            TemplateState::Modified | TemplateState::NoMarker => {
                if force {
                    if dry_run {
                        eprintln!("  would overwrite (modified): {}", specific);
                    } else {
                        std::fs::write(&path, crate::paths::prepend_hash_marker(builtin))?;
                        eprintln!("  overwritten: {}", specific);
                    }
                    updated += 1;
                } else {
                    eprintln!("  \x1b[33mskipped (modified):\x1b[0m {}", specific);
                    skipped += 1;
                }
            }
        }
    }

    if updated == 0 && skipped == 0 {
        eprintln!("All templates are up to date.");
    } else {
        if updated > 0 {
            let verb = if dry_run { "would update" } else { "updated" };
            eprintln!("\n{} {} template(s).", verb, updated);
        }
        if skipped > 0 {
            eprintln!("{} modified template(s) skipped (use --force to overwrite).", skipped);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// reset
// ---------------------------------------------------------------------------

fn handle_reset(name: &str, input: &Path, target: Option<&str>) -> Result<()> {
    let ctx = resolve_context(input, target)?;
    let specific = format!("{}.{}", name, ctx.ext);
    let path = ctx.tpl_dir.join(&specific);

    if !path.exists() {
        bail!("No local template '{}' found in {}", specific, ctx.tpl_dir.display());
    }

    // Verify there's a built-in to replace it with
    let builtin = resolve_builtin_template(name, &ctx.writer)
        .ok_or_else(|| anyhow::anyhow!("No built-in template '{}' for writer '{}'", name, ctx.writer))?;

    // Replace with the built-in version (with hash marker)
    std::fs::write(&path, crate::paths::prepend_hash_marker(builtin))?;
    eprintln!("Reset {} to built-in default.", specific);

    Ok(())
}
