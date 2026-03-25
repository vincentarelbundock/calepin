//! Theme manifest parsing.
//!
//! A theme is a directory under `_calepin/themes/{name}/` that provides
//! templates, snippets, and assets layered on top of a target's built-in
//! templates.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Parsed theme manifest (`theme.toml`).
pub struct ThemeManifest {
    /// Theme name. Must match directory name.
    pub name: String,
    /// Which target this theme is designed for.
    pub target: String,
    /// Shown in `calepin info themes`.
    pub description: Option<String>,
    /// Default settings when this theme is active.
    pub defaults: Option<toml::Value>,
    /// Default template variables.
    pub vars: Option<toml::Value>,
    /// Absolute path to the theme directory.
    pub theme_dir: PathBuf,
}

impl ThemeManifest {
    /// Load a theme manifest from `theme.toml` in the given directory.
    pub fn load(dir: &Path) -> Result<Self> {
        let toml_path = dir.join("theme.toml");
        let content = std::fs::read_to_string(&toml_path)
            .with_context(|| format!("Failed to read {}", toml_path.display()))?;
        let config: toml::Value = toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", toml_path.display()))?;

        let name = config.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Theme manifest missing 'name' field: {}", toml_path.display()))?
            .to_string();

        let target = config.get("target")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Theme manifest missing 'target' field: {}", toml_path.display()))?
            .to_string();

        let theme_dir = dir.canonicalize()
            .unwrap_or_else(|_| dir.to_path_buf());

        Ok(ThemeManifest {
            name,
            target,
            description: config.get("description").and_then(|v| v.as_str()).map(String::from),
            defaults: config.get("defaults").cloned(),
            vars: config.get("vars").cloned(),
            theme_dir,
        })
    }
}

/// Resolve a theme directory by name.
/// Checks `{project_root}/_calepin/themes/{name}/theme.toml`.
pub fn resolve_theme_dir(name: &str, project_root: &Path) -> Option<PathBuf> {
    let dir = project_root.join("_calepin").join("themes").join(name);
    if dir.join("theme.toml").exists() {
        Some(dir)
    } else {
        None
    }
}

/// List all available themes in the project.
pub fn list_themes(project_root: &Path) -> Vec<ThemeManifest> {
    let themes_dir = project_root.join("_calepin").join("themes");
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
