//! `calepin init new` -- create a project or document from a scaffold.
//!
//! The scaffold kind ("collection" or "document") is declared in the extension
//! manifest's `[scaffold.*]` table. Collections create a directory project with
//! `_calepin/config.toml`; documents create a single `.qmd` with a sidecar.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::config::extension::ExtensionManifest;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Handle `calepin init new <path> [--scaffold NAME] [--extension PATH]`.
pub fn handle_init_new(
    path: &Path,
    scaffold_name: Option<&str>,
    extension: Option<&str>,
) -> Result<()> {
    // Infer scaffold name from path when not explicit.
    let is_qmd = path.extension().and_then(|e| e.to_str()) == Some("qmd");
    let scaffold_name = scaffold_name.unwrap_or(if is_qmd { "document" } else { "website" });

    let resolved = resolve_scaffold(scaffold_name, extension)?;
    let info = resolved.manifest.get_scaffold(scaffold_name)
        .ok_or_else(|| anyhow::anyhow!("Scaffold '{}' not found in extension '{}'", scaffold_name, resolved.manifest.name))?;

    let is_document = info.kind == "document";

    if is_document {
        scaffold_document(path, &resolved, scaffold_name)
    } else {
        scaffold_collection(path, &resolved, scaffold_name)
    }
}

// ---------------------------------------------------------------------------
// Collection scaffold (website, book, ...)
// ---------------------------------------------------------------------------

fn scaffold_collection(dir: &Path, scaffold: &ResolvedScaffold, scaffold_name: &str) -> Result<()> {
    if dir.exists() {
        bail!("{} already exists", dir.display());
    }
    std::fs::create_dir_all(dir)
        .with_context(|| format!("Failed to create directory: {}", dir.display()))?;

    copy_scaffold(scaffold, dir)?;
    install_extension(scaffold, dir)?;

    // Compose config.toml: scaffold project config + shared defaults + collection defaults
    let calepin_dir = crate::paths::calepin_dir(dir, &[]);
    std::fs::create_dir_all(&calepin_dir)?;
    let project_config = scaffold_file(scaffold, "_calepin/config.toml");
    let composed = format!(
        "{}\n\n# === Built-in defaults ===\n\n{}\n{}",
        project_config.trim(),
        crate::config::SHARED_TOML,
        crate::config::COLLECTION_TOML,
    );
    std::fs::write(calepin_dir.join("config.toml"), composed)?;

    // Copy built-in assets (CSS, fonts, icons)
    let kind = crate::paths::ProjectKind::Collection {
        root: dir.to_path_buf(),
        config: calepin_dir.join("config.toml"),
    };
    crate::themes::copy_builtin_assets(&kind)?;

    if scaffold.ext_dir.is_some() {
        eprintln!("Created {} project in {}/ (using extension '{}')", scaffold_name, dir.display(), scaffold.manifest.name);
    } else {
        eprintln!("Created {} project in {}/", scaffold_name, dir.display());
    }
    eprintln!();
    super::tree::print_tree(dir, 2);
    eprintln!();
    eprintln!("To preview:  calepin preview {}", dir.display());

    Ok(())
}

// ---------------------------------------------------------------------------
// Document scaffold (single .qmd)
// ---------------------------------------------------------------------------

fn scaffold_document(path: &Path, scaffold: &ResolvedScaffold, _scaffold_name: &str) -> Result<()> {
    let path = if path.extension().is_some() {
        path.to_path_buf()
    } else {
        path.with_extension("qmd")
    };

    if path.exists() {
        bail!("{} already exists", path.display());
    }

    let stem = path.file_stem().unwrap().to_string_lossy();
    let sidecar = path.parent().unwrap_or(Path::new(".")).join(format!("{}_calepin", stem));
    if sidecar.exists() {
        bail!("Sidecar directory {} already exists", sidecar.display());
    }

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let parent = path.parent().unwrap_or(Path::new("."));
    copy_scaffold(scaffold, parent)?;

    // Rename the scaffold's .qmd to the requested filename.
    // Look up the .qmd name from the scaffold source itself (not the filesystem).
    if let Some(scaffold_qmd_name) = scaffold_qmd_name(scaffold) {
        let copied = parent.join(&scaffold_qmd_name);
        if copied != path && copied.exists() {
            std::fs::rename(&copied, &path)?;
        }
    }

    // Install external extension into sidecar
    install_extension_into_sidecar(scaffold, &sidecar)?;

    // Copy built-in assets
    let kind = crate::paths::ProjectKind::Document {
        qmd: path.clone(),
        sidecar: sidecar.clone(),
    };
    crate::themes::copy_builtin_assets(&kind)?;

    if scaffold.ext_dir.is_some() {
        eprintln!("Created {} (using extension '{}')", path.display(), scaffold.manifest.name);
    } else {
        eprintln!("Created {}", path.display());
    }

    Ok(())
}

/// Find the name of the .qmd file inside a scaffold source.
fn scaffold_qmd_name(scaffold: &ResolvedScaffold) -> Option<String> {
    match &scaffold.source {
        ScaffoldSource::Builtin(dir) => {
            dir.files().find_map(|f| {
                let name = f.path().file_name()?.to_str()?;
                if name.ends_with(".qmd") { Some(name.to_string()) } else { None }
            })
        }
        ScaffoldSource::External(scaffold_dir) => {
            std::fs::read_dir(scaffold_dir).ok()?.flatten().find_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if name.ends_with(".qmd") { Some(name) } else { None }
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Scaffold resolution
// ---------------------------------------------------------------------------

/// The source of a scaffold: either a built-in embedded directory or an external path.
pub enum ScaffoldSource {
    Builtin(&'static include_dir::Dir<'static>),
    External(PathBuf),
}

/// Resolved scaffold ready to be copied.
pub struct ResolvedScaffold {
    pub source: ScaffoldSource,
    pub manifest: ExtensionManifest,
    /// Path to the extension directory on disk (None for built-in).
    pub ext_dir: Option<PathBuf>,
}

/// Resolve a scaffold by name, optionally from a specific extension.
fn resolve_scaffold(scaffold_name: &str, extension: Option<&str>) -> Result<ResolvedScaffold> {
    if let Some(ext_arg) = extension {
        let (ext_dir, manifest) = resolve_extension(ext_arg)?;
        if !manifest.has_scaffold(scaffold_name) {
            let available: Vec<&str> = manifest.scaffold.keys().map(|k| k.as_str()).collect();
            if available.is_empty() {
                bail!("Extension '{}' does not declare any scaffolds", manifest.name);
            }
            bail!(
                "Extension '{}' does not declare [scaffold.{}]. Available: {}",
                manifest.name, scaffold_name, available.join(", ")
            );
        }
        let scaffold_dir = ext_dir.join("scaffold").join(scaffold_name);
        if !scaffold_dir.is_dir() {
            bail!(
                "Extension '{}' declares [scaffold.{}] but has no scaffold/{} directory",
                manifest.name, scaffold_name, scaffold_name
            );
        }
        Ok(ResolvedScaffold {
            source: ScaffoldSource::External(scaffold_dir),
            manifest,
            ext_dir: Some(ext_dir),
        })
    } else {
        let (ext_name, dir) = crate::config::extension::builtin_scaffold(scaffold_name)
            .ok_or_else(|| anyhow::anyhow!("No built-in scaffold named '{}'. Available: website, book, document", scaffold_name))?;
        let manifest = crate::config::extension::builtin_extension(ext_name)
            .ok_or_else(|| anyhow::anyhow!("Built-in extension '{}' not found", ext_name))?;
        Ok(ResolvedScaffold {
            source: ScaffoldSource::Builtin(dir),
            manifest,
            ext_dir: None,
        })
    }
}

fn resolve_extension(extension: &str) -> Result<(PathBuf, ExtensionManifest)> {
    let ext_dir = PathBuf::from(extension);
    let ext_dir = if ext_dir.is_relative() {
        std::env::current_dir()?.join(&ext_dir)
    } else {
        ext_dir
    };
    if !ext_dir.join("extension.toml").exists() {
        bail!(
            "No extension.toml found in {}. Provide a path to an extension directory.",
            ext_dir.display()
        );
    }
    let manifest = ExtensionManifest::load(&ext_dir)
        .with_context(|| format!("Failed to load extension from {}", ext_dir.display()))?;
    Ok((ext_dir, manifest))
}

// ---------------------------------------------------------------------------
// Scaffold file operations
// ---------------------------------------------------------------------------

fn copy_scaffold(scaffold: &ResolvedScaffold, dest: &Path) -> Result<()> {
    match &scaffold.source {
        ScaffoldSource::Builtin(dir) => {
            crate::paths::write_embedded_dir(dir, dest);
            Ok(())
        }
        ScaffoldSource::External(scaffold_dir) => {
            crate::paths::copy_dir_recursive(scaffold_dir, dest)
                .with_context(|| format!("Failed to copy scaffold from {}", scaffold_dir.display()))
        }
    }
}

fn scaffold_file(scaffold: &ResolvedScaffold, rel_path: &str) -> String {
    match &scaffold.source {
        ScaffoldSource::Builtin(dir) => {
            dir.get_file(rel_path)
                .and_then(|f| f.contents_utf8())
                .unwrap_or("")
                .to_string()
        }
        ScaffoldSource::External(scaffold_dir) => {
            std::fs::read_to_string(scaffold_dir.join(rel_path)).unwrap_or_default()
        }
    }
}

/// Install an external extension into `_calepin/extensions/`. No-op for built-in.
fn install_extension(scaffold: &ResolvedScaffold, project_dir: &Path) -> Result<()> {
    let ext_dir = match &scaffold.ext_dir {
        Some(dir) => dir,
        None => return Ok(()),
    };
    let target = project_dir
        .join("_calepin")
        .join("extensions")
        .join(&scaffold.manifest.name);
    std::fs::create_dir_all(&target)?;
    copy_dir_filtered(ext_dir, &target)
        .with_context(|| format!("Failed to install extension '{}' into project", scaffold.manifest.name))
}

/// Install an external extension into a sidecar's `extensions/`. No-op for built-in.
fn install_extension_into_sidecar(scaffold: &ResolvedScaffold, sidecar_dir: &Path) -> Result<()> {
    let ext_dir = match &scaffold.ext_dir {
        Some(dir) => dir,
        None => return Ok(()),
    };
    let target = sidecar_dir
        .join("extensions")
        .join(&scaffold.manifest.name);
    std::fs::create_dir_all(&target)?;
    copy_dir_filtered(ext_dir, &target)
        .with_context(|| format!("Failed to install extension '{}' into sidecar", scaffold.manifest.name))
}

/// Copy a directory tree, skipping the `scaffold/` subdirectory.
fn copy_dir_filtered(src: &Path, dst: &Path) -> Result<()> {
    use walkdir::WalkDir;
    for entry in WalkDir::new(src) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src)?;
        if rel.starts_with("scaffold") {
            continue;
        }
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
