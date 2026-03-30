//! `calepin templates` subcommand: list, eject, show, diff, update, reset.

use std::path::Path;

use anyhow::{bail, Result};
use xxhash_rust::xxh3::xxh3_64;

use crate::cli::TemplatesAction;
use crate::render::elements::{BUILTIN_TEMPLATES, resolve_builtin_template};

// ---------------------------------------------------------------------------
// Hash utilities
// ---------------------------------------------------------------------------

/// Compute xxh3_64 hash of content, formatted as hex.
fn compute_hash(content: &[u8]) -> String {
    format!("{:016x}", xxh3_64(content))
}

/// Format: `{# calepin:xxh3:HASH #}\n`
const MARKER_PREFIX: &str = "{# calepin:xxh3:";
const MARKER_SUFFIX: &str = " #}";

/// Prepend a hash marker to template content.
fn prepend_hash_marker(content: &str) -> String {
    let hash = compute_hash(content.as_bytes());
    format!("{}{}{}\n{}", MARKER_PREFIX, hash, MARKER_SUFFIX, content)
}

/// Parse the hash from the first line of a file, if it has a marker.
fn parse_hash_marker(text: &str) -> Option<String> {
    let first_line = text.lines().next()?;
    let rest = first_line.strip_prefix(MARKER_PREFIX)?;
    let hash = rest.strip_suffix(MARKER_SUFFIX)?;
    Some(hash.to_string())
}

/// Strip the hash marker line from file content.
fn strip_hash_marker(text: &str) -> &str {
    if text.starts_with(MARKER_PREFIX) {
        // Skip first line (marker) + newline
        text.find('\n').map(|i| &text[i + 1..]).unwrap_or("")
    } else {
        text
    }
}

/// Template modification state.
enum TemplateState {
    /// Local content matches the ejected built-in (hash matches).
    Unmodified,
    /// User has customized the template.
    Modified,
    /// No hash marker found (manually created or old-style override).
    NoMarker,
}

/// Check whether a local template file has been modified relative to its marker hash.
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
// Target resolution helpers
// ---------------------------------------------------------------------------

/// Resolve target, writer, extension, and inheritance chain for a target name.
fn resolve_target_info(target_name: &str) -> (Option<crate::config::Target>, String, String, Vec<String>) {
    let empty = std::collections::HashMap::new();
    let target = crate::config::resolve_target(target_name, &empty).ok();
    let writer = target.as_ref().map(|t| t.writer.as_str()).unwrap_or(target_name).to_string();
    let ext = crate::paths::resolve_extension(&writer).to_string();
    let project_root = crate::paths::get_project_dir();
    let empty_targets = std::collections::HashMap::new();
    let chain = crate::config::extension::inheritance_chain(&project_root, target_name, &empty_targets);
    crate::paths::set_active_target_with_chain(Some(target_name), chain.clone());
    (target, writer, ext, chain)
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
        TemplatesAction::List { target } => handle_list(&target),
        TemplatesAction::Eject { target, force, dry_run } => handle_eject(&target, force, dry_run),
        TemplatesAction::Show { name, target } => handle_show(&name, &target),
        TemplatesAction::Diff { target } => handle_diff(&target),
        TemplatesAction::Update { target, force, dry_run } => handle_update(&target, force, dry_run),
        TemplatesAction::Reset { name, target } => handle_reset(&name, &target),
    }
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

fn handle_list(target_name: &str) -> Result<()> {
    let (target, writer, ext, chain) = resolve_target_info(target_name);
    if target.is_none() {
        eprintln!("Note: target '{}' not found, showing writer-level templates", target_name);
    }

    let names = collect_builtin_names(&chain);
    let project_root = crate::paths::get_project_dir();

    println!("Template resolution for target '{}' (writer: {}):\n", target_name, writer);

    for name in &names {
        if let Some(path) = crate::paths::resolve_template(name, &writer) {
            let rel = path.strip_prefix(&project_root).unwrap_or(&path);
            // Check modification state
            let state_label = match std::fs::read_to_string(&path) {
                Ok(content) => match check_template_state(&content) {
                    TemplateState::Unmodified => "\x1b[32mlocal\x1b[0m",
                    TemplateState::Modified => "\x1b[33mmodified\x1b[0m",
                    TemplateState::NoMarker => "\x1b[33mlocal (no marker)\x1b[0m",
                },
                Err(_) => "\x1b[31merror\x1b[0m",
            };
            println!("  {}.{}  {}  {}", name, ext, state_label, rel.display());
        } else if resolve_builtin_template(name, &writer).is_some() {
            println!("  {}.{}  \x1b[36mbuilt-in\x1b[0m", name, ext);
        } else {
            println!("  {}.{}  \x1b[31mnot found\x1b[0m", name, ext);
        }
    }

    let templates_dir = crate::paths::templates_dir(&project_root);
    if templates_dir.is_dir() {
        println!("\nUser templates: {}", templates_dir.display());
    } else {
        println!("\nNo user templates directory.");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// eject
// ---------------------------------------------------------------------------

/// Eject built-in templates for a target's inheritance chain into `_calepin/templates/`.
fn handle_eject(target_name: &str, force: bool, dry_run: bool) -> Result<()> {
    let (_target, _writer, _ext, chain) = resolve_target_info(target_name);
    let project_root = crate::paths::get_project_dir();
    let dest = crate::paths::templates_dir(&project_root);

    let written = eject_templates(&dest, &chain, force, dry_run)?;

    if dry_run {
        eprintln!("Dry run: would write {} file(s) to {}", written, dest.display());
    } else if written > 0 {
        eprintln!("Ejected {} template(s) to {}", written, dest.display());
    } else {
        eprintln!("All templates already exist (use --force to overwrite).");
    }

    Ok(())
}

/// Core eject logic, shared by `handle_eject` and init integration.
pub fn eject_templates(dest: &Path, chain: &[String], force: bool, dry_run: bool) -> Result<usize> {
    let mut written = 0;

    for chain_target in chain {
        if let Some(dir) = BUILTIN_TEMPLATES.get_dir(chain_target.as_str()) {
            let target_dest = dest.join(chain_target);
            written += eject_dir_recursive(dir, chain_target, &target_dest, force, dry_run)?;
        }
    }

    // Write README
    if !dry_run && written > 0 {
        let readme_path = dest.join("README.md");
        if !readme_path.exists() || force {
            write_readme(&readme_path, chain)?;
        }
    }

    Ok(written)
}

/// Recursively eject files from a built-in directory.
fn eject_dir_recursive(
    dir: &include_dir::Dir<'static>,
    chain_target: &str,
    dest: &Path,
    force: bool,
    dry_run: bool,
) -> Result<usize> {
    let mut written = 0;
    let prefix = std::path::Path::new(chain_target);

    for file in dir.files() {
        let rel = file.path().strip_prefix(prefix).unwrap_or(file.path());
        let out = dest.join(rel);
        if out.exists() && !force {
            continue;
        }
        if let Some(content) = file.contents_utf8() {
            if dry_run {
                let rel_display = dest.file_name().unwrap_or_default().to_string_lossy();
                eprintln!("  would write: {}/{}", rel_display, rel.display());
            } else {
                if let Some(parent) = out.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let marked = prepend_hash_marker(content);
                std::fs::write(&out, &marked)?;
            }
            written += 1;
        }
    }

    for subdir in dir.dirs() {
        let sub_dest = dest.join(
            subdir.path().strip_prefix(prefix).unwrap_or(subdir.path())
        );
        written += eject_dir_recursive_inner(subdir, &sub_dest, force, dry_run)?;
    }

    Ok(written)
}

/// Inner recursive helper (no prefix stripping needed).
fn eject_dir_recursive_inner(
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
                let marked = prepend_hash_marker(content);
                std::fs::write(&out, &marked)?;
            }
            written += 1;
        }
    }

    for subdir in dir.dirs() {
        let sub_name = subdir.path().file_name().unwrap_or_default();
        written += eject_dir_recursive_inner(subdir, &dest.join(sub_name), force, dry_run)?;
    }

    Ok(written)
}

// ---------------------------------------------------------------------------
// show
// ---------------------------------------------------------------------------

fn handle_show(name: &str, target_name: &str) -> Result<()> {
    let (_target, writer, _ext, _chain) = resolve_target_info(target_name);

    // Try filesystem first
    if let Some(path) = crate::paths::resolve_template(name, &writer) {
        let content = std::fs::read_to_string(&path)?;
        print!("{}", strip_hash_marker(&content));
        return Ok(());
    }

    // Fall back to built-in
    if let Some(content) = resolve_builtin_template(name, &writer) {
        print!("{}", content);
        return Ok(());
    }

    bail!("Template '{}' not found for target '{}'", name, target_name);
}

// ---------------------------------------------------------------------------
// diff
// ---------------------------------------------------------------------------

fn handle_diff(target_name: &str) -> Result<()> {
    let (_target, writer, ext, chain) = resolve_target_info(target_name);
    let names = collect_builtin_names(&chain);
    let mut found_diff = false;

    for name in &names {
        if let Some(path) = crate::paths::resolve_template(name, &writer) {
            let local_content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let local_body = strip_hash_marker(&local_content);

            let builtin = match resolve_builtin_template(name, &writer) {
                Some(b) => b,
                None => continue,
            };

            if local_body == builtin {
                continue;
            }

            found_diff = true;
            println!("\x1b[1m--- built-in: {}.{}\x1b[0m", name, ext);
            println!("\x1b[1m+++ local:    {}.{}\x1b[0m", name, ext);

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
    }

    if !found_diff {
        eprintln!("No differences found. All local templates match their built-in defaults.");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// update
// ---------------------------------------------------------------------------

fn handle_update(target_name: &str, force: bool, dry_run: bool) -> Result<()> {
    let (_target, writer, _ext, chain) = resolve_target_info(target_name);
    let names = collect_builtin_names(&chain);
    let project_root = crate::paths::get_project_dir();
    let mut updated = 0;
    let mut skipped = 0;

    for name in &names {
        if let Some(path) = crate::paths::resolve_template(name, &writer) {
            let local_content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let builtin = match resolve_builtin_template(name, &writer) {
                Some(b) => b,
                None => continue,
            };

            let local_body = strip_hash_marker(&local_content);
            if local_body == builtin {
                // Already up to date
                continue;
            }

            let state = check_template_state(&local_content);
            match state {
                TemplateState::Unmodified => {
                    // Safe to update: user hasn't touched it
                    let rel = path.strip_prefix(&project_root).unwrap_or(&path);
                    if dry_run {
                        eprintln!("  would update: {}", rel.display());
                    } else {
                        let marked = prepend_hash_marker(builtin);
                        std::fs::write(&path, &marked)?;
                        eprintln!("  updated: {}", rel.display());
                    }
                    updated += 1;
                }
                TemplateState::Modified | TemplateState::NoMarker => {
                    if force {
                        let rel = path.strip_prefix(&project_root).unwrap_or(&path);
                        if dry_run {
                            eprintln!("  would overwrite (modified): {}", rel.display());
                        } else {
                            let marked = prepend_hash_marker(builtin);
                            std::fs::write(&path, &marked)?;
                            eprintln!("  overwritten: {}", rel.display());
                        }
                        updated += 1;
                    } else {
                        let rel = path.strip_prefix(&project_root).unwrap_or(&path);
                        eprintln!("  \x1b[33mskipped (modified):\x1b[0m {}", rel.display());
                        skipped += 1;
                    }
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

fn handle_reset(name: &str, target_name: &str) -> Result<()> {
    let (_target, writer, _ext, _chain) = resolve_target_info(target_name);

    let path = match crate::paths::resolve_template(name, &writer) {
        Some(p) => p,
        None => bail!("No local override found for template '{}' (target: '{}')", name, target_name),
    };

    // Verify there's a built-in to fall back to
    if resolve_builtin_template(name, &writer).is_none() {
        bail!("No built-in template '{}' exists for writer '{}'. Cannot reset.", name, writer);
    }

    std::fs::remove_file(&path)?;
    let project_root = crate::paths::get_project_dir();
    let rel = path.strip_prefix(&project_root).unwrap_or(&path);
    eprintln!("Removed local override: {}", rel.display());
    eprintln!("Template '{}' now resolves from built-in.", name);

    // Clean up empty parent directories
    if let Some(parent) = path.parent() {
        remove_empty_dirs(parent, &project_root);
    }

    Ok(())
}

fn remove_empty_dirs(dir: &Path, stop_at: &Path) {
    let mut current = dir.to_path_buf();
    while current.starts_with(stop_at) && current != *stop_at {
        if std::fs::read_dir(&current).map(|mut d| d.next().is_none()).unwrap_or(true) {
            let _ = std::fs::remove_dir(&current);
            current = match current.parent() {
                Some(p) => p.to_path_buf(),
                None => break,
            };
        } else {
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// README generation
// ---------------------------------------------------------------------------

fn write_readme(path: &Path, chain: &[String]) -> Result<()> {
    let chain_display = chain.join(" -> ");
    let content = format!(
r#"# Calepin Templates

These templates control how Calepin renders your documents. They were
ejected from Calepin's built-in defaults and can be customized freely.

Inheritance chain: {chain_display}

## Directory structure

Each subdirectory corresponds to a target or writer in the inheritance
chain. Templates in more specific directories (e.g., `website/`)
override those in parent directories (e.g., `html/`).

## Resolution order

1. `_calepin/templates/{{target}}/` (most specific)
2. `_calepin/templates/{{parent}}/` (inherited)
3. Built-in defaults (compiled into the binary)

## Template variables

Templates use Jinja syntax. Two namespaces are available:
- `config.*` -- user-authored values (front matter, attributes, labels)
- `calepin.*` -- engine-computed values (rendered content, format, paths)

## Hash tracking

Each file's first line contains a hash marker like:

    {{{{# calepin:xxh3:abc123... #}}}}

This lets `calepin templates update` detect whether you have customized
a template. Do not remove or edit this line manually.

## Common tasks

    calepin templates list [target]     Show resolution status
    calepin templates diff [target]     Compare local vs built-in
    calepin templates update [target]   Refresh unmodified templates
    calepin templates reset NAME        Remove a local override
    calepin templates show NAME         Print a template's content
"#);
    std::fs::write(path, &content)?;
    Ok(())
}
