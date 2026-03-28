//! Extension manifest parsing (`extension.toml`).
//!
//! An extension bundles a target definition, partials, modules, assets,
//! and variables into a single distributable directory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use super::targets::Target;

// ---------------------------------------------------------------------------
// ExtensionManifest
// ---------------------------------------------------------------------------

/// Parsed extension manifest (`extension.toml`).
#[derive(Debug, Clone, Deserialize)]
pub struct ExtensionManifest {
    /// Extension name (e.g., "tufte", "slides").
    pub name: String,
    /// Short description.
    #[serde(default)]
    pub description: String,
    /// Semantic version.
    #[serde(default)]
    pub version: String,
    /// Author name.
    #[serde(default)]
    pub author: String,
    /// License identifier.
    #[serde(default)]
    pub license: String,
    /// Parent extension name (single-chain inheritance).
    pub inherits: Option<String>,
    /// Target definition (overrides parent fields).
    #[serde(default)]
    pub target: Option<ExtensionTarget>,
    /// Extension variables, namespaced by extension name in templates.
    #[serde(default)]
    pub vars: HashMap<String, toml::Value>,
    /// Asset declarations.
    #[serde(default)]
    pub assets: ExtensionAssets,
    /// Module declarations.
    #[serde(default)]
    pub modules: Vec<ExtensionModule>,
}

/// Target definition within an extension manifest.
/// Same fields as `Target` but all optional (inherited from parent).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExtensionTarget {
    pub writer: Option<String>,
    pub template: Option<String>,
    pub extension: Option<String>,
    pub fig_extension: Option<String>,
    pub preview: Option<String>,
    pub embed_resources: Option<bool>,
    pub crossref: Option<String>,
    pub toc_headings: Option<bool>,
    pub modules: Option<Vec<String>>,
    pub post: Option<Vec<String>>,
    pub fig_formats: Option<Vec<String>>,
    pub page_vars: Option<HashMap<String, String>>,
    pub vars: Option<toml::Value>,
}

/// Asset declarations in an extension manifest.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExtensionAssets {
    /// CSS files to inject into the page template.
    #[serde(default)]
    pub css: Vec<String>,
    /// JS files to inject into the page template.
    #[serde(default)]
    pub js: Vec<String>,
    /// Static files/directories to copy to assets/ in output.
    #[serde(rename = "static", default)]
    pub static_files: Vec<String>,
}

/// Module declaration within an extension manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct ExtensionModule {
    /// Module name.
    pub name: String,
    /// Short description.
    #[serde(default)]
    pub description: String,
    /// Module kind: span, element_children, element, document, project.
    pub kind: String,
    /// Contexts this module handles: "div", "span", or both.
    #[serde(default)]
    pub contexts: Vec<String>,
    /// Match rules for element-level modules.
    #[serde(rename = "match")]
    pub match_rule: Option<ExtensionMatchRule>,
    /// Path to external executable (script or WASM), relative to extension dir.
    pub run: Option<String>,
    /// Protocol for external modules: "json" (default) or "text".
    #[serde(default = "default_protocol")]
    pub protocol: String,
}

fn default_protocol() -> String {
    "json".to_string()
}

/// Match rules for element-level modules.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExtensionMatchRule {
    /// CSS classes that trigger this module (OR'd).
    #[serde(default)]
    pub classes: Vec<String>,
    /// Attribute names that trigger this module (OR'd).
    #[serde(default)]
    pub attrs: Vec<String>,
    /// ID prefix that triggers this module.
    pub id_prefix: Option<String>,
    /// Restrict to specific writers (omit = all).
    #[serde(default)]
    pub writers: Vec<String>,
    /// Auto-number matching elements.
    #[serde(default)]
    pub number: bool,
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

impl ExtensionManifest {
    /// Load an extension manifest from a directory containing `extension.toml`.
    pub fn load(dir: &Path) -> Result<Self> {
        let toml_path = dir.join("extension.toml");
        let content = std::fs::read_to_string(&toml_path)
            .with_context(|| format!("Failed to read {}", toml_path.display()))?;
        let manifest: ExtensionManifest = toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", toml_path.display()))?;
        Ok(manifest)
    }

    /// Convert the extension's target definition to a `Target` struct.
    /// Fields not set in the extension are left as defaults (to be filled
    /// by inheritance resolution).
    pub fn to_target(&self) -> Target {
        let ext_target = self.target.as_ref();
        Target {
            inherits: self.inherits.clone(),
            writer: ext_target.and_then(|t| t.writer.clone()).unwrap_or_default(),
            template: ext_target.and_then(|t| t.template.clone()),
            extension: ext_target.and_then(|t| t.extension.clone()),
            fig_extension: ext_target.and_then(|t| t.fig_extension.clone()),
            preview: ext_target.and_then(|t| t.preview.clone()),
            compile: None,
            embed_resources: ext_target.and_then(|t| t.embed_resources),
            vars: ext_target.and_then(|t| t.vars.clone()),
            post: ext_target.and_then(|t| t.post.clone()).unwrap_or_default(),
            modules: ext_target.and_then(|t| t.modules.clone()).unwrap_or_default(),
            crossref: ext_target.and_then(|t| t.crossref.clone()),
            toc_headings: ext_target.and_then(|t| t.toc_headings),
            page_vars: ext_target.and_then(|t| t.page_vars.clone()).unwrap_or_default(),
            fig_formats: ext_target.and_then(|t| t.fig_formats.clone()).unwrap_or_default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Discover installed extensions in `_calepin/extensions/`.
pub fn discover_extensions(project_root: &Path) -> Vec<(String, PathBuf)> {
    let extensions_dir = project_root.join("_calepin").join("extensions");
    if !extensions_dir.is_dir() {
        return Vec::new();
    }
    let mut found = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&extensions_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("extension.toml").exists() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    found.push((name.to_string(), path));
                }
            }
        }
    }
    found
}

/// Load all installed extensions from `_calepin/extensions/`.
pub fn load_extensions(project_root: &Path) -> Result<HashMap<String, (ExtensionManifest, PathBuf)>> {
    let mut extensions = HashMap::new();
    for (name, path) in discover_extensions(project_root) {
        let manifest = ExtensionManifest::load(&path)
            .with_context(|| format!("Failed to load extension '{}'", name))?;
        extensions.insert(name, (manifest, path));
    }
    Ok(extensions)
}

// ---------------------------------------------------------------------------
// Built-in extension manifests (embedded at compile time)
// ---------------------------------------------------------------------------

/// Built-in extension manifest sources, embedded at compile time.
pub const BUILTIN_EXTENSIONS: &[(&str, &str)] = &[
    ("html", include_str!("../extensions/html/extension.toml")),
    ("latex", include_str!("../extensions/latex/extension.toml")),
    ("typst", include_str!("../extensions/typst/extension.toml")),
    ("markdown", include_str!("../extensions/markdown/extension.toml")),
    ("revealjs", include_str!("../extensions/revealjs/extension.toml")),
    ("website", include_str!("../extensions/website/extension.toml")),
    ("book", include_str!("../extensions/book/extension.toml")),
];

/// Parse a built-in extension manifest by name.
pub fn builtin_extension(name: &str) -> Option<ExtensionManifest> {
    BUILTIN_EXTENSIONS.iter()
        .find(|(n, _)| *n == name)
        .and_then(|(_, content)| toml::from_str(content).ok())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_extensions_parse() {
        for (name, content) in BUILTIN_EXTENSIONS {
            let manifest: ExtensionManifest = toml::from_str(content)
                .unwrap_or_else(|e| panic!("Failed to parse {}/extension.toml: {}", name, e));
            assert_eq!(manifest.name, *name, "Extension name mismatch for {}", name);
        }
    }

    #[test]
    fn test_html_extension_target() {
        let manifest = builtin_extension("html").unwrap();
        let target = manifest.to_target();
        assert_eq!(target.writer, "html");
        assert_eq!(target.extension.as_deref(), Some("html"));
        assert!(target.modules.contains(&"highlight".to_string()));
        assert!(target.modules.contains(&"append_footnotes".to_string()));
    }

    #[test]
    fn test_revealjs_inherits_html() {
        let manifest = builtin_extension("revealjs").unwrap();
        assert_eq!(manifest.inherits.as_deref(), Some("html"));
        let target = manifest.to_target();
        assert!(target.modules.contains(&"split_slides".to_string()));
        assert!(!target.modules.contains(&"embed_images".to_string()));
    }

    #[test]
    fn test_website_has_project_modules() {
        let manifest = builtin_extension("website").unwrap();
        assert_eq!(manifest.inherits.as_deref(), Some("html"));
        let project_modules: Vec<&str> = manifest.modules.iter()
            .filter(|m| m.kind == "project")
            .map(|m| m.name.as_str())
            .collect();
        assert!(project_modules.contains(&"site_wrap"));
        assert!(project_modules.contains(&"crossref_global"));
    }

    #[test]
    fn test_book_has_orchestrator() {
        let manifest = builtin_extension("book").unwrap();
        let project_modules: Vec<&str> = manifest.modules.iter()
            .filter(|m| m.kind == "project")
            .map(|m| m.name.as_str())
            .collect();
        assert!(project_modules.contains(&"orchestrator"));
    }
}
