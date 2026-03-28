//! Theme system: bundled sets of partials + assets + config.
//!
//! A theme packages partials, assets, and an optional config fragment that
//! can be applied to a document sidecar or collection `_calepin/` directory.

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

static THEME_MINIMAL: Dir<'static> = include_dir::include_dir!("$CARGO_MANIFEST_DIR/src/themes/minimal");

// ---------------------------------------------------------------------------
// ThemeManifest
// ---------------------------------------------------------------------------

/// Metadata about a theme (parsed from `theme.toml`).
#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
pub struct ThemeManifest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub date: String,
    /// The base format this theme inherits from (e.g., "html", "website", "latex").
    #[serde(default)]
    pub inherit: String,
}

// ---------------------------------------------------------------------------
// Theme
// ---------------------------------------------------------------------------

/// A theme that can be applied to a project.
#[allow(dead_code)]
pub struct Theme {
    pub manifest: ThemeManifest,
    /// Collected files: relative path -> contents.
    files: BTreeMap<PathBuf, Vec<u8>>,
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
                author: String::new(),
                version: String::new(),
                license: String::new(),
                date: String::new(),
                inherit: "website".to_string(),
            },
            files,
        }
    }

    /// Load the built-in minimal theme (no sidebar, no table of contents).
    pub fn builtin_minimal() -> Self {
        let mut files = BTreeMap::new();
        collect_embedded_files(&THEME_MINIMAL, Path::new(""), &mut files);
        files.remove(Path::new("theme.toml"));
        Self {
            manifest: ThemeManifest {
                name: "minimal".to_string(),
                description: "Clean layout without sidebar or table of contents".to_string(),
                author: String::new(),
                version: String::new(),
                license: String::new(),
                date: String::new(),
                inherit: "website".to_string(),
            },
            files,
        }
    }

    /// Load a built-in theme by name.
    pub fn builtin(name: &str) -> Option<Self> {
        match name {
            "default" => Some(Self::builtin_default()),
            "minimal" => Some(Self::builtin_minimal()),
            _ => None,
        }
    }

    /// Resolve a theme by name (built-in) or path (local directory).
    pub fn resolve(name: &str) -> Result<Self> {
        if let Some(theme) = Self::builtin(name) {
            return Ok(theme);
        }
        let path = Path::new(name);
        if path.is_dir() {
            return Self::from_path(path);
        }
        bail!(
            "Unknown theme: \"{}\". Built-in themes: default, minimal.",
            name
        );
    }

    /// Load a theme from a local directory.
    ///
    /// Expected layout:
    /// ```text
    /// theme.toml          # name, description, inherit, ...
    /// partials/            # mirrors _calepin/partials/
    /// assets/              # mirrors _calepin/assets/
    /// ```
    pub fn from_path(path: &Path) -> Result<Self> {
        if !path.is_dir() {
            bail!("Theme path is not a directory: {}", path.display());
        }

        let manifest_path = path.join("theme.toml");
        let manifest: ThemeManifest = if manifest_path.exists() {
            let text = std::fs::read_to_string(&manifest_path)
                .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
            toml::from_str(&text)
                .with_context(|| format!("Failed to parse {}", manifest_path.display()))?
        } else {
            ThemeManifest {
                name: path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                description: String::new(),
                author: String::new(),
                version: String::new(),
                license: String::new(),
                date: String::new(),
                inherit: String::new(),
            }
        };

        let mut files = BTreeMap::new();
        for subdir in &["partials", "assets"] {
            let dir = path.join(subdir);
            if dir.is_dir() {
                collect_fs_files(&dir, &Path::new(subdir), &mut files)?;
            }
        }

        Ok(Self { manifest, files })
    }

    /// Apply theme files to a project, writing to `_calepin/`.
    pub fn apply(&self, kind: &ProjectKind) -> Result<()> {
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

fn collect_embedded_files(dir: &Dir<'static>, prefix: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
    for file in dir.files() {
        let rel = prefix.join(file.path());
        out.insert(rel, file.contents().to_vec());
    }
    for subdir in dir.dirs() {
        collect_embedded_files(subdir, prefix, out);
    }
}

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
