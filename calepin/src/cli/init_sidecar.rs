//! `calepin init sidecar` -- extract a sidecar directory from an existing .qmd document.
//!
//! Splits TOML front matter into identity fields (kept in the .qmd) and rendering
//! fields (moved to `{stem}_calepin/config.toml`). Backs up the original file
//! before rewriting.

use std::path::Path;
use anyhow::{bail, Result};
use toml_edit::DocumentMut;

/// Top-level TOML keys that describe document identity and stay in the front matter.
const IDENTITY_KEYS: &[&str] = &[
    "title",
    "subtitle",
    "author",
    "authors",
    "date",
    "abstract",
    "keywords",
    "copyright",
    "license",
    "citation",
    "funding",
    "appendix-style",
    "appendix_style",
    "var",
];

/// Extract the raw TOML front matter string and the body from a .qmd file.
/// Returns `(front_matter_string, body_string, closing_delimiter)`.
fn split_raw(text: &str) -> Option<(String, String, &'static str)> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() || lines[0].trim() != "---" {
        return None;
    }
    let mut end = None;
    let mut closer = "---";
    for (i, line) in lines.iter().enumerate().skip(1) {
        let trimmed = line.trim_end();
        if trimmed == "---" || trimmed == "..." {
            end = Some(i);
            if trimmed == "..." {
                closer = "...";
            }
            break;
        }
    }
    let end = end?;
    let raw = lines[1..end].join("\n");
    let body = lines[end + 1..].join("\n");
    if raw.trim().is_empty() {
        return None;
    }
    Some((raw, body, closer))
}

pub fn handle_init_sidecar(
    path: &Path,
    force: bool,
    write_templates: bool,
    target_name: Option<&str>,
    dry_run: bool,
    no_backup: bool,
) -> Result<()> {
    if !path.exists() {
        bail!("File not found: {}", path.display());
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext != "qmd" {
        bail!("Expected a .qmd file, got: {}", path.display());
    }

    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("doc");
    let parent = path.parent().unwrap_or(Path::new("."));
    let sidecar_dir = parent.join(format!("{}_calepin", stem));

    if sidecar_dir.exists() && !force {
        bail!(
            "Sidecar directory already exists: {}\nUse --force to overwrite.",
            sidecar_dir.display()
        );
    }

    let text = std::fs::read_to_string(path)?;
    let (raw_toml, body, closer) = match split_raw(&text) {
        Some(parts) => parts,
        None => {
            if dry_run {
                println!("No TOML front matter found. Would create sidecar with defaults at: {}", sidecar_dir.display());
                return Ok(());
            }
            println!("No TOML front matter found. Creating sidecar with defaults.");
            crate::paths::create_sidecar(&sidecar_dir);
            if write_templates {
                crate::paths::write_builtin_templates(&sidecar_dir.join("templates"));
            }
            if target_name.is_some() {
                apply_target_assets(path, &sidecar_dir)?;
            }
            println!("Created: {}", sidecar_dir.display());
            return Ok(());
        }
    };

    let doc: DocumentMut = match raw_toml.parse() {
        Ok(d) => d,
        Err(_) => {
            if dry_run {
                println!("Front matter is not valid TOML. Would create sidecar with defaults at: {}", sidecar_dir.display());
                return Ok(());
            }
            println!("Front matter is not valid TOML. Creating sidecar with defaults.");
            crate::paths::create_sidecar(&sidecar_dir);
            if write_templates {
                crate::paths::write_builtin_templates(&sidecar_dir.join("templates"));
            }
            if target_name.is_some() {
                apply_target_assets(path, &sidecar_dir)?;
            }
            println!("Created: {}", sidecar_dir.display());
            return Ok(());
        }
    };

    // Partition keys into identity (stays) and rendering (moves to sidecar)
    let mut identity_doc = DocumentMut::new();
    let mut rendering_doc = DocumentMut::new();
    let mut moved_keys: Vec<String> = Vec::new();
    let mut kept_keys: Vec<String> = Vec::new();

    for (key, item) in doc.iter() {
        if is_identity_key(key) {
            identity_doc[key] = item.clone();
            kept_keys.push(key.to_string());
        } else {
            rendering_doc[key] = item.clone();
            moved_keys.push(key.to_string());
        }
    }

    let identity_toml = identity_doc.to_string();
    let rendering_toml = rendering_doc.to_string();
    let has_rendering = !rendering_toml.trim().is_empty();
    let has_identity = !identity_toml.trim().is_empty();

    // Build the new .qmd content
    let new_qmd = if has_identity {
        format!("---\n{}\n{}\n{}", identity_toml.trim_end(), closer, body)
    } else {
        body.clone()
    };

    if dry_run {
        println!("Sidecar directory: {}", sidecar_dir.display());
        if has_rendering {
            println!("\nRendering keys to move to config.toml:");
            for key in &moved_keys {
                println!("  {}", key);
            }
        } else {
            println!("\nNo rendering keys found in front matter.");
        }
        if has_identity {
            println!("\nIdentity keys staying in front matter:");
            for key in &kept_keys {
                println!("  {}", key);
            }
        }
        if has_rendering && !no_backup {
            println!("\nWould back up {} to {}.bak", path.display(), path.display());
        }
        return Ok(());
    }

    // Create sidecar directory
    let sidecar_existed = sidecar_dir.exists();
    std::fs::create_dir_all(&sidecar_dir)?;

    // Write config.toml
    let config_path = sidecar_dir.join("config.toml");
    if has_rendering {
        std::fs::write(&config_path, rendering_toml.trim_end().to_string() + "\n")?;
        println!("Wrote rendering config to: {}", config_path.display());
    } else {
        let defaults = format!("{}\n{}", crate::config::SHARED_TOML, crate::config::DOCUMENT_TOML);
        std::fs::write(&config_path, &defaults)?;
        println!("No rendering keys in front matter. Wrote defaults to: {}", config_path.display());
    }

    if write_templates {
        crate::paths::write_builtin_templates(&sidecar_dir.join("templates"));
        println!("Wrote built-in templates.");
    }

    if let Some(target) = target_name {
        apply_target_assets(path, &sidecar_dir)?;
        println!("Applied target assets: {}", target);
    }

    // Back up and rewrite the .qmd only on first run (sidecar didn't exist before).
    // With --force on an existing sidecar, only update config/templates, not the .qmd.
    let is_fresh = !sidecar_existed;
    if has_rendering && is_fresh {
        if !no_backup {
            let backup = parent.join(format!("{}.qmd.bak", stem));
            std::fs::copy(path, &backup)?;
            println!("Backed up original to: {}", backup.display());
        }
        std::fs::write(path, &new_qmd)?;
        println!("Rewrote front matter in: {}", path.display());
    }

    println!("Done.");
    Ok(())
}

fn is_identity_key(key: &str) -> bool {
    let normalized = key.replace('-', "_");
    IDENTITY_KEYS.iter().any(|k| k.replace('-', "_") == normalized)
}

fn apply_target_assets(qmd_path: &Path, sidecar: &Path) -> Result<()> {
    use crate::paths::ProjectKind;
    // Apply assets only (partials come from built-in via layered resolution)
    let kind = ProjectKind::Document {
        qmd: qmd_path.to_path_buf(),
        sidecar: sidecar.to_path_buf(),
    };
    crate::themes::copy_builtin_assets(&kind)
}
