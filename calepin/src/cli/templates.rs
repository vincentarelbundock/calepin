//! `calepin templates` subcommand: list, show, diff, reset.
//!
//! All subcommands operate on a specific sidecar identified by the input .qmd file.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::cli::TemplatesAction;
use crate::render::elements::{BUILTIN_TEMPLATES, resolve_builtin_template};

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

/// Collect all built-in template filenames for the given inheritance chain.
fn collect_builtin_files(chain: &[String], ext: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for chain_target in chain {
        if let Some(dir) = BUILTIN_TEMPLATES.get_dir(chain_target.as_str()) {
            let prefix = std::path::Path::new(chain_target.as_str());
            collect_dir_files(dir, prefix, &mut names);
        }
    }
    names.sort();
    names.dedup();
    names
}

fn collect_dir_files(dir: &include_dir::Dir<'static>, prefix: &Path, names: &mut Vec<String>) {
    for file in dir.files() {
        let rel = file.path().strip_prefix(prefix).unwrap_or(file.path());
        let name = rel.display().to_string();
        if !names.contains(&name) {
            names.push(name);
        }
    }
    for subdir in dir.dirs() {
        collect_dir_files(subdir, prefix, names);
    }
}

/// Get built-in template content by filename, walking the chain (child-first).
fn get_builtin_content(chain: &[String], filename: &str) -> Option<String> {
    for chain_target in chain {
        let path = format!("{}/{}", chain_target, filename);
        if let Some(file) = BUILTIN_TEMPLATES.get_file(&path) {
            if let Some(content) = file.contents_utf8() {
                return Some(content.to_string());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------------

pub fn handle_templates(action: TemplatesAction) -> Result<()> {
    match action {
        TemplatesAction::List { input, target } => handle_list(&input, target.as_deref()),
        TemplatesAction::Show { name, input, target } => handle_show(&name, &input, target.as_deref()),
        TemplatesAction::Diff { input, name, target } => handle_diff(&input, name.as_deref(), target.as_deref()),
        TemplatesAction::Reset { input, name, target, force } => handle_reset(&input, name.as_deref(), target.as_deref(), force),
    }
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

fn handle_list(input: &Path, target: Option<&str>) -> Result<()> {
    let ctx = resolve_context(input, target)?;
    let builtin_files = collect_builtin_files(&ctx.chain, &ctx.ext);

    println!("Templates for '{}' (target: {}):\n", input.display(), ctx.target_name);

    // Collect local files
    let mut local_files: Vec<String> = Vec::new();
    if ctx.tpl_dir.is_dir() {
        let pattern = ctx.tpl_dir.join("**").join("*.*");
        for entry in crate::util::safe_glob(&pattern.display().to_string()) {
            if let Ok(path) = entry {
                if path.is_file() {
                    let rel = path.strip_prefix(&ctx.tpl_dir).unwrap_or(&path);
                    local_files.push(rel.display().to_string());
                }
            }
        }
        local_files.sort();
    }

    // Show built-in templates with status
    for name in &builtin_files {
        let path = ctx.tpl_dir.join(name);
        if path.exists() {
            let local = std::fs::read_to_string(&path).unwrap_or_default();
            let builtin = get_builtin_content(&ctx.chain, name).unwrap_or_default();
            if local == builtin {
                println!("  {}  \x1b[36mdefault\x1b[0m", name);
            } else {
                println!("  {}  \x1b[33mmodified\x1b[0m", name);
            }
        } else {
            println!("  {}  \x1b[90mmissing\x1b[0m", name);
        }
    }

    // Show custom templates (in sidecar but not in built-in)
    for name in &local_files {
        if !builtin_files.contains(name) {
            println!("  {}  \x1b[32mcustom\x1b[0m", name);
        }
    }

    if ctx.tpl_dir.is_dir() {
        println!("\nTemplates: {}", ctx.tpl_dir.display());
    } else {
        println!("\nNo sidecar. Run `calepin init {}` to create one.", input.display());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// show
// ---------------------------------------------------------------------------

fn handle_show(name: &str, input: &Path, target: Option<&str>) -> Result<()> {
    let ctx = resolve_context(input, target)?;

    // Try local file first
    let specific = format!("{}.{}", name, ctx.ext);
    let path = ctx.tpl_dir.join(&specific);
    if path.exists() {
        print!("{}", std::fs::read_to_string(&path)?);
        return Ok(());
    }

    // Try exact filename (e.g., "base.html")
    let path = ctx.tpl_dir.join(name);
    if path.exists() {
        print!("{}", std::fs::read_to_string(&path)?);
        return Ok(());
    }

    // Fall back to built-in
    if let Some(content) = resolve_builtin_template(name, &ctx.writer) {
        print!("{}", content);
        return Ok(());
    }

    // Try by full filename in chain
    if let Some(content) = get_builtin_content(&ctx.chain, name) {
        print!("{}", content);
        return Ok(());
    }

    bail!("Template '{}' not found for target '{}'", name, ctx.target_name);
}

// ---------------------------------------------------------------------------
// diff
// ---------------------------------------------------------------------------

fn handle_diff(input: &Path, name: Option<&str>, target: Option<&str>) -> Result<()> {
    let ctx = resolve_context(input, target)?;

    if !ctx.tpl_dir.is_dir() {
        bail!("No sidecar templates at {}. Run `calepin init {}` first.", ctx.tpl_dir.display(), input.display());
    }

    let files: Vec<String> = if let Some(name) = name {
        // Single file: try exact name, then name.ext
        let path = ctx.tpl_dir.join(name);
        if path.exists() {
            vec![name.to_string()]
        } else {
            let specific = format!("{}.{}", name, ctx.ext);
            let path = ctx.tpl_dir.join(&specific);
            if path.exists() {
                vec![specific]
            } else {
                bail!("Template '{}' not found in {}", name, ctx.tpl_dir.display());
            }
        }
    } else {
        collect_builtin_files(&ctx.chain, &ctx.ext)
    };

    let mut found_diff = false;
    for filename in &files {
        let path = ctx.tpl_dir.join(filename);
        if !path.exists() { continue; }

        let local = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let builtin = match get_builtin_content(&ctx.chain, filename) {
            Some(b) => b,
            None => continue,
        };

        if local == builtin { continue; }

        found_diff = true;
        println!("\x1b[1m--- built-in: {}\x1b[0m", filename);
        println!("\x1b[1m+++ local:    {}\x1b[0m", filename);

        let builtin_lines: Vec<&str> = builtin.lines().collect();
        let local_lines: Vec<&str> = local.lines().collect();
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
// reset
// ---------------------------------------------------------------------------

fn handle_reset(input: &Path, name: Option<&str>, target: Option<&str>, force: bool) -> Result<()> {
    let ctx = resolve_context(input, target)?;

    if !ctx.tpl_dir.is_dir() {
        bail!("No sidecar templates at {}. Nothing to reset.", ctx.tpl_dir.display());
    }

    if let Some(name) = name {
        // Reset single template
        let path = ctx.tpl_dir.join(name);
        let specific = format!("{}.{}", name, ctx.ext);
        let path = if path.exists() { path } else { ctx.tpl_dir.join(&specific) };
        let filename = if ctx.tpl_dir.join(name).exists() { name.to_string() } else { specific };

        let builtin = get_builtin_content(&ctx.chain, &filename)
            .ok_or_else(|| anyhow::anyhow!("No built-in template '{}' for target '{}'", filename, ctx.target_name))?;

        std::fs::write(&path, &builtin)?;
        eprintln!("Reset {} to built-in.", filename);
    } else {
        // Reset all templates
        if !force {
            eprint!("Reset all templates in {} to built-in defaults? [y/N] ", ctx.tpl_dir.display());
            let mut answer = String::new();
            std::io::stdin().read_line(&mut answer)?;
            if !answer.trim().eq_ignore_ascii_case("y") {
                eprintln!("Aborted.");
                return Ok(());
            }
        }

        let files = collect_builtin_files(&ctx.chain, &ctx.ext);
        let mut count = 0;
        for filename in &files {
            if let Some(builtin) = get_builtin_content(&ctx.chain, filename) {
                let path = ctx.tpl_dir.join(filename);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, &builtin)?;
                count += 1;
            }
        }
        eprintln!("Reset {} template(s) to built-in defaults.", count);
    }

    Ok(())
}
