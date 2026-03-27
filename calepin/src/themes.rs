//! Theme system: bundled sets of partials + assets + config.
//!
//! A theme packages partials, assets, and an optional config fragment that
//! can be applied to a document sidecar or collection `_calepin/` directory.
//! The `default` theme corresponds to the built-in partials and assets that
//! `calepin new` has always written.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use include_dir::Dir;

use crate::config::paths::ProjectKind;

// ---------------------------------------------------------------------------
// Built-in partials and assets (embedded at compile time)
// ---------------------------------------------------------------------------

pub use crate::render::elements::BUILTIN_PARTIALS;
pub use crate::render::elements::BUILTIN_ASSETS;

// ---------------------------------------------------------------------------
// ThemeManifest
// ---------------------------------------------------------------------------

/// Metadata about a theme.
#[derive(Debug, Clone)]
pub struct ThemeManifest {
    pub name: String,
    pub description: String,
}

// ---------------------------------------------------------------------------
// FileAction: what apply() would do
// ---------------------------------------------------------------------------

/// Describes what will happen to a single file when a theme is applied.
#[derive(Debug, Clone)]
pub enum FileAction {
    /// File does not exist yet; will be created.
    Create,
    /// File exists with different content; will be overwritten (old content backed up).
    Overwrite,
    /// File exists with identical content; no change needed.
    Unchanged,
}

/// The result of computing a theme diff against a target directory.
#[derive(Debug)]
pub struct ApplyPlan {
    /// (relative path, action, new content)
    pub actions: Vec<(PathBuf, FileAction, Vec<u8>)>,
    /// Optional config fragment to merge.
    pub config_fragment: Option<String>,
}

impl ApplyPlan {
    pub fn created(&self) -> Vec<&Path> {
        self.actions.iter()
            .filter(|(_, a, _)| matches!(a, FileAction::Create))
            .map(|(p, _, _)| p.as_path())
            .collect()
    }

    pub fn overwritten(&self) -> Vec<&Path> {
        self.actions.iter()
            .filter(|(_, a, _)| matches!(a, FileAction::Overwrite))
            .map(|(p, _, _)| p.as_path())
            .collect()
    }

    pub fn unchanged_count(&self) -> usize {
        self.actions.iter()
            .filter(|(_, a, _)| matches!(a, FileAction::Unchanged))
            .count()
    }

    pub fn has_changes(&self) -> bool {
        self.actions.iter().any(|(_,a,_)| !matches!(a, FileAction::Unchanged))
            || self.config_fragment.is_some()
    }
}

// ---------------------------------------------------------------------------
// Theme
// ---------------------------------------------------------------------------

/// A theme that can be applied to a project.
pub struct Theme {
    pub manifest: ThemeManifest,
    /// Collected files: relative path -> contents.
    files: BTreeMap<PathBuf, Vec<u8>>,
    /// Optional config.toml fragment to merge.
    config_fragment: Option<String>,
}

impl Theme {
    /// Load the built-in default theme.
    pub fn builtin_default() -> Self {
        let mut files = BTreeMap::new();

        // Partials
        collect_embedded_files(&BUILTIN_PARTIALS, Path::new("partials"), &mut files);

        // Assets (the "assets" subdirectory inside BUILTIN_ASSETS)
        if let Some(assets_dir) = BUILTIN_ASSETS.get_dir("assets") {
            collect_embedded_dir_stripped(assets_dir, Path::new("assets"), Path::new("assets"), &mut files);
        }

        Self {
            manifest: ThemeManifest {
                name: "default".to_string(),
                description: "Default Calepin theme".to_string(),
            },
            files,
            config_fragment: None,
        }
    }

    /// Load a built-in theme by name.
    pub fn builtin(name: &str) -> Option<Self> {
        match name {
            "default" => Some(Self::builtin_default()),
            _ => None,
        }
    }

    /// Load a theme from a local directory.
    ///
    /// Expected layout:
    /// ```text
    /// theme.toml          # name, description
    /// partials/            # mirrors _calepin/partials/
    /// assets/              # mirrors _calepin/assets/
    /// config.toml          # optional config fragment
    /// ```
    pub fn from_path(path: &Path) -> Result<Self> {
        if !path.is_dir() {
            bail!("Theme path is not a directory: {}", path.display());
        }

        // Parse theme.toml
        let manifest_path = path.join("theme.toml");
        let manifest = if manifest_path.exists() {
            let text = std::fs::read_to_string(&manifest_path)
                .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
            let table: toml::Table = toml::from_str(&text)
                .with_context(|| format!("Failed to parse {}", manifest_path.display()))?;
            ThemeManifest {
                name: table.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                description: table.get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            }
        } else {
            ThemeManifest {
                name: path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                description: String::new(),
            }
        };

        // Collect files from partials/ and assets/
        let mut files = BTreeMap::new();
        for subdir in &["partials", "assets"] {
            let dir = path.join(subdir);
            if dir.is_dir() {
                collect_fs_files(&dir, &Path::new(subdir), &mut files)?;
            }
        }

        // Optional config fragment
        let config_path = path.join("config.toml");
        let config_fragment = if config_path.exists() {
            Some(std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read {}", config_path.display()))?)
        } else {
            None
        };

        Ok(Self { manifest, files, config_fragment })
    }

    /// List available built-in themes.
    pub fn list_builtin() -> Vec<ThemeManifest> {
        vec![
            ThemeManifest {
                name: "default".to_string(),
                description: "Default Calepin theme".to_string(),
            },
        ]
    }

    /// Compute what applying this theme to a project would do.
    pub fn plan(&self, kind: &ProjectKind) -> ApplyPlan {
        let base = kind.calepin_dir();
        let mut actions = Vec::new();

        for (rel, content) in &self.files {
            let target = base.join(rel);
            let action = if target.exists() {
                if let Ok(existing) = std::fs::read(&target) {
                    if existing == *content {
                        FileAction::Unchanged
                    } else {
                        FileAction::Overwrite
                    }
                } else {
                    FileAction::Create
                }
            } else {
                FileAction::Create
            };
            actions.push((rel.clone(), action, content.clone()));
        }

        ApplyPlan {
            actions,
            config_fragment: self.config_fragment.clone(),
        }
    }

    /// Apply the theme: write files, back up conflicts with `.bak`.
    pub fn apply(&self, kind: &ProjectKind) -> Result<ApplyPlan> {
        let base = kind.calepin_dir();
        let plan = self.plan(kind);

        for (rel, action, content) in &plan.actions {
            let target = base.join(rel);

            // Back up conflicting files
            if matches!(action, FileAction::Overwrite) {
                let bak = target.with_extension(
                    format!("{}.bak",
                        target.extension().and_then(|e| e.to_str()).unwrap_or(""))
                );
                if !bak.exists() {
                    let _ = std::fs::copy(&target, &bak);
                }
            }

            if matches!(action, FileAction::Create | FileAction::Overwrite) {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("Failed to create {}", parent.display()))?;
                }
                std::fs::write(&target, content)
                    .with_context(|| format!("Failed to write {}", target.display()))?;
            }
        }

        Ok(plan)
    }

    /// Apply silently (no plan needed, used by `calepin new`).
    pub fn apply_quiet(&self, kind: &ProjectKind) -> Result<()> {
        let base = kind.calepin_dir();
        for (rel, content) in &self.files {
            let target = base.join(rel);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&target, content)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers: collect files from embedded dirs and filesystem
// ---------------------------------------------------------------------------

/// Collect files from an embedded `include_dir::Dir`, prefixing relative paths.
fn collect_embedded_files(dir: &Dir<'static>, prefix: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
    for file in dir.files() {
        let rel = prefix.join(file.path());
        out.insert(rel, file.contents().to_vec());
    }
    for subdir in dir.dirs() {
        collect_embedded_files(subdir, prefix, out);
    }
}

/// Collect files from an embedded dir, stripping a prefix from paths and
/// replacing it with a new prefix.
fn collect_embedded_dir_stripped(
    dir: &Dir<'static>,
    new_prefix: &Path,
    strip: &Path,
    out: &mut BTreeMap<PathBuf, Vec<u8>>,
) {
    for file in dir.files() {
        let rel = file.path().strip_prefix(strip).unwrap_or(file.path());
        out.insert(new_prefix.join(rel), file.contents().to_vec());
    }
    for subdir in dir.dirs() {
        collect_embedded_dir_stripped(subdir, new_prefix, strip, out);
    }
}

/// Collect files from the filesystem recursively.
fn collect_fs_files(dir: &Path, prefix: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) -> Result<()> {
    for entry in walkdir::WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(dir)?;
            let content = std::fs::read(entry.path())
                .with_context(|| format!("Failed to read {}", entry.path().display()))?;
            out.insert(prefix.join(rel), content);
        }
    }
    Ok(())
}
