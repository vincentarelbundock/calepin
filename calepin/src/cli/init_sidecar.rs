//! `calepin init` -- create documents, websites, books, and extensions.
//!
//! Usage:
//!   calepin init paper.qmd              # new document (html target)
//!   calepin init paper.qmd -t latex     # new document (latex target)
//!   calepin init mysite -t website      # new website project
//!   calepin init mybook -t book         # new book project
//!   calepin init extension myext        # new extension (inherits html)

use std::path::Path;
use anyhow::{bail, Result};

use crate::cli::InitArgs;

pub fn handle_init(args: InitArgs) -> Result<()> {
    let path = &args.path;
    let path_str = path.to_string_lossy();

    // Special case: `calepin init extension <name>`
    if path_str == "extension" {
        // The target field doubles as the extension name for this case.
        // Usage: calepin init extension -t html  (but really we need a name)
        // Better: detect next positional somehow. For now, require -t as inherits.
        bail!("Usage: calepin init extension <name> [--inherits TARGET]\n\
               Use `calepin extra` subcommands for other utilities.");
    }

    let is_qmd = path.extension().and_then(|e| e.to_str()) == Some("qmd");
    let target = args.target.as_deref().unwrap_or(if is_qmd { "html" } else { "website" });

    // Determine if this is a collection target
    let is_collection = crate::config::extension::is_collection_target(target);

    if is_collection {
        // Collection: delegate to scaffold (scaffold name matches target name)
        crate::cli::scaffold::handle_init_new(path, Some(target), None)
    } else if is_qmd {
        // Document: create .qmd (if needed) + sidecar with templates
        init_document(path, target, args.force)
    } else {
        // Bare directory name without collection target -- assume website
        crate::cli::scaffold::handle_init_new(path, Some("website"), None)
    }
}

/// Initialize a document: create the .qmd file if it doesn't exist,
/// then create the sidecar with config.toml and all templates for the target.
fn init_document(path: &Path, target: &str, force: bool) -> Result<()> {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("doc");
    let parent = path.parent().unwrap_or(Path::new("."));
    let sidecar_dir = parent.join(format!("{}_calepin", stem));

    if sidecar_dir.exists() && !force {
        bail!(
            "Sidecar directory already exists: {}\nUse --force to overwrite.",
            sidecar_dir.display()
        );
    }

    // Create the .qmd file if it doesn't exist
    if !path.exists() {
        let content = format!("---\ntarget = \"{}\"\ntitle = \"{}\"\n---\n\n", target, stem);
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(path, &content)?;
        eprintln!("  Created: {}", path.display());
    }

    // Create sidecar directory
    std::fs::create_dir_all(&sidecar_dir)?;

    // Write config.toml with target and defaults
    let config_path = sidecar_dir.join("config.toml");
    let config = format!("target = \"{}\"\n\n{}\n{}", target, crate::config::SHARED_TOML, crate::config::DOCUMENT_TOML);
    std::fs::write(&config_path, &config)?;
    eprintln!("  Wrote: {}", config_path.display());

    // Write all templates for the target
    eject_templates(&sidecar_dir, target);
    eprintln!("  Wrote templates to: {}/templates/{}/", sidecar_dir.display(), target);

    // Copy built-in assets
    let kind = crate::paths::ProjectKind::Document {
        qmd: path.to_path_buf(),
        sidecar: sidecar_dir.clone(),
    };
    crate::themes::copy_builtin_assets(&kind)?;

    eprintln!("  Done: {}", sidecar_dir.display());
    Ok(())
}

/// Write all built-in templates for a target into the sidecar's templates/ directory.
fn eject_templates(sidecar: &Path, target: &str) {
    use crate::render::elements::BUILTIN_EXTENSIONS;
    let empty = std::collections::HashMap::new();
    let project_root = sidecar.parent().unwrap_or(Path::new("."));
    let chain = crate::config::extension::inheritance_chain(project_root, target, &empty);
    crate::paths::set_active_target_with_chain(Some(target), chain.clone());
    let dest = sidecar.join("templates").join(target);
    // Parent-first so child overrides parent
    for chain_target in chain.iter().rev() {
        let tpl_path = format!("{}/templates", chain_target);
        if let Some(dir) = BUILTIN_EXTENSIONS.get_dir(&tpl_path) {
            write_builtin_dir(dir, &tpl_path, &dest);
        }
    }
}

/// Recursively write files from a built-in directory, stripping the prefix.
fn write_builtin_dir(dir: &include_dir::Dir<'static>, prefix: &str, dest: &std::path::Path) {
    let prefix_path = std::path::Path::new(prefix);
    for file in dir.files() {
        let rel = file.path().strip_prefix(prefix_path).unwrap_or(file.path());
        let out = dest.join(rel);
        if let Some(content) = file.contents_utf8() {
            if let Some(parent) = out.parent() { let _ = std::fs::create_dir_all(parent); }
            let _ = std::fs::write(&out, content);
        } else if !out.exists() {
            // Binary files (icons, etc.)
            if let Some(parent) = out.parent() { let _ = std::fs::create_dir_all(parent); }
            let _ = std::fs::write(&out, file.contents());
        }
    }
    for subdir in dir.dirs() {
        write_builtin_dir(subdir, &subdir.path().display().to_string(), dest);
    }
}
