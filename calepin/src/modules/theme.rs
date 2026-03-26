//! Theme manifests: load and list themes from `_calepin/themes/`.

use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

/// A theme is a directory with a `theme.toml` manifest that provides
/// partials, snippets, and assets layered on top of a target's built-in
/// partials.
#[derive(Debug)]
pub struct ThemeManifest {
    pub name: String,
    pub target: String,
    pub description: Option<String>,
    #[allow(dead_code)]
    pub theme_dir: PathBuf,
}

impl ThemeManifest {
    /// Load a theme manifest from `theme.toml` in the given directory.
    pub fn load(dir: &Path) -> Result<Self> {
        let toml_path = dir.join("theme.toml");
        let content = std::fs::read_to_string(&toml_path)
            .with_context(|| format!("Failed to read {}", toml_path.display()))?;
        let parsed: toml::Value = content.parse()
            .with_context(|| format!("Failed to parse {}", toml_path.display()))?;

        let name = parsed.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| dir.file_name().unwrap_or_default().to_str().unwrap_or(""))
            .to_string();
        let target = parsed.get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("html")
            .to_string();
        let description = parsed.get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let theme_dir = dir.canonicalize()
            .unwrap_or_else(|_| dir.to_path_buf());

        Ok(ThemeManifest {
            name,
            target,
            description,
            theme_dir,
        })
    }
}

/// Resolve a theme directory by name from `_calepin/themes/`.
pub fn resolve_theme_dir(name: &str, project_root: &Path) -> Option<PathBuf> {
    let dir = crate::paths::themes_dir(project_root).join(name);
    if dir.join("theme.toml").exists() {
        Some(dir)
    } else {
        None
    }
}

/// List all available themes in `_calepin/themes/`.
pub fn list_themes(project_root: &Path) -> Vec<ThemeManifest> {
    let themes_dir = crate::paths::themes_dir(project_root);
    if !themes_dir.is_dir() {
        return Vec::new();
    }
    let mut themes = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&themes_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("theme.toml").exists() {
                if let Ok(manifest) = ThemeManifest::load(&path) {
                    themes.push(manifest);
                }
            }
        }
    }
    themes.sort_by(|a, b| a.name.cmp(&b.name));
    themes
}
