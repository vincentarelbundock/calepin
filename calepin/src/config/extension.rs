//! Extension manifest parsing (`extension.toml`).
//!
//! An extension bundles a target definition, partials, modules, assets,
//! and variables into a single distributable directory.

use std::cell::RefCell;
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
#[allow(dead_code)]
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
    /// Parent extension name (single-target shorthand).
    /// When set, the extension defines a single target with this parent.
    /// For multi-target extensions, use `[targets.*]` instead.
    pub inherits: Option<String>,
    /// Single target definition (shorthand, used with top-level `inherits`).
    #[serde(default)]
    pub target: Option<ExtensionTarget>,
    /// Named targets (multi-target extensions). Each target carries its own `inherits`.
    #[serde(default)]
    pub targets: HashMap<String, ExtensionTarget>,
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
    /// Parent target to inherit from (used in `[targets.*]` entries).
    pub inherits: Option<String>,
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
#[allow(dead_code)]
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

/// Convert an `ExtensionTarget` to a `Target`.
fn extension_target_to_target(et: &ExtensionTarget) -> Target {
    Target {
        inherits: et.inherits.clone(),
        writer: et.writer.clone().unwrap_or_default(),
        template: et.template.clone(),
        extension: et.extension.clone(),
        fig_extension: et.fig_extension.clone(),
        preview: et.preview.clone(),
        embed_resources: et.embed_resources,
        vars: et.vars.clone(),
        post: et.post.clone().unwrap_or_default(),
        modules: et.modules.clone().unwrap_or_default(),
        crossref: et.crossref.clone(),
        toc_headings: et.toc_headings,
        page_vars: et.page_vars.clone().unwrap_or_default(),
        fig_formats: et.fig_formats.clone().unwrap_or_default(),
    }
}

fn default_protocol() -> String {
    "json".to_string()
}

/// Match rules for element-level modules.
#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)]
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

    /// Convert the extension's default target definition to a `Target` struct.
    /// Uses the single-target shorthand (`inherits` + `[target]`).
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

    /// Look up a named target from the `[targets.*]` table.
    /// Returns a `Target` struct with the named target's `inherits` field.
    pub fn named_target(&self, name: &str) -> Option<Target> {
        let et = self.targets.get(name)?;
        Some(extension_target_to_target(et))
    }

    /// Return all named targets defined by this extension.
    pub fn named_targets(&self) -> Vec<(String, Target)> {
        self.targets.iter()
            .map(|(name, et)| (name.clone(), extension_target_to_target(et)))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Discover installed extensions in `_calepin/extensions/`.
#[allow(dead_code)]
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
                    if is_valid_extension_name(name) {
                        found.push((name.to_string(), path));
                    }
                }
            }
        }
    }
    found
}

/// Load all installed extensions from `_calepin/extensions/`.
#[allow(dead_code)]
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
// Cached manifest loading
// ---------------------------------------------------------------------------

thread_local! {
    static MANIFEST_CACHE: RefCell<HashMap<PathBuf, ExtensionManifest>> = RefCell::new(HashMap::new());
}

/// Load an extension manifest with caching. Parses once per path per thread.
pub fn load_cached(dir: &Path) -> Option<ExtensionManifest> {
    let toml_path = dir.join("extension.toml");
    MANIFEST_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(manifest) = cache.get(&toml_path) {
            return Some(manifest.clone());
        }
        let content = std::fs::read_to_string(&toml_path).ok()?;
        let manifest: ExtensionManifest = toml::from_str(&content).ok()?;
        cache.insert(toml_path, manifest.clone());
        Some(manifest)
    })
}

// ---------------------------------------------------------------------------
// Extension chain walking
// ---------------------------------------------------------------------------

/// Walk the extension inheritance chain from `start_name` to root.
/// Calls `visitor` for each extension in child-first order.
/// Returns the collected results.
pub fn walk_chain<T>(
    project_root: &Path,
    start_name: &str,
    mut visitor: impl FnMut(&str, &Path, &ExtensionManifest) -> Option<T>,
) -> Vec<T> {
    let project_ext_dir = project_root.join("_calepin").join("extensions");
    let sidecar_ext_dir = crate::paths::get_sidecar_root()
        .map(|s| s.join("extensions"));

    // Need at least one extensions directory to exist.
    let has_project = project_ext_dir.is_dir();
    let has_sidecar = sidecar_ext_dir.as_ref().map_or(false, |d| d.is_dir());
    if !has_project && !has_sidecar {
        return Vec::new();
    }

    let mut results = Vec::new();
    let mut current = Some(start_name.to_string());
    let mut visited = std::collections::HashSet::new();
    while let Some(name) = current.take() {
        if !visited.insert(name.clone()) { break; }
        // Check sidecar first, then project _calepin/
        let extension_dir = sidecar_ext_dir.as_ref()
            .map(|d| d.join(&name))
            .filter(|d| d.join("extension.toml").exists())
            .unwrap_or_else(|| project_ext_dir.join(&name));
        if let Some(manifest) = load_cached(&extension_dir) {
            if let Some(result) = visitor(&name, &extension_dir, &manifest) {
                results.push(result);
            }
            current = manifest.inherits.clone();
            continue;
        }
        // Check named targets in installed extensions
        let extensions = discover_extensions(project_root);
        for (_ext_name, ext_path) in &extensions {
            if let Some(manifest) = load_cached(ext_path) {
                if let Some(et) = manifest.targets.get(&name) {
                    // Include the owning extension's partials/assets
                    if let Some(result) = visitor(&name, ext_path, &manifest) {
                        results.push(result);
                    }
                    current = et.inherits.clone();
                    break;
                }
            }
        }
    }
    results
}

/// Walk the chain and return just the extension names (child-first).
pub fn chain_names(project_root: &Path, start_name: &str) -> Vec<String> {
    walk_chain(project_root, start_name, |name, _, _| Some(name.to_string()))
}

/// Compute the full target inheritance chain (child-first).
/// Checks installed extensions (sidecar/project), built-in extension manifests,
/// and user target definitions for `inherits` fields.
///
/// Always includes `start_name`. Example: `inheritance_chain("minimal", ...)` ->
/// `["minimal", "website", "html"]`.
pub fn inheritance_chain(
    project_root: &Path,
    start_name: &str,
    user_targets: &std::collections::HashMap<String, super::targets::Target>,
) -> Vec<String> {
    let project_ext_dir = project_root.join("_calepin").join("extensions");
    let sidecar_ext_dir = crate::paths::get_sidecar_root()
        .map(|s| s.join("extensions"));

    let mut chain = Vec::new();
    let mut current = Some(start_name.to_string());
    let mut visited = std::collections::HashSet::new();

    while let Some(name) = current.take() {
        if !visited.insert(name.clone()) { break; }
        chain.push(name.clone());

        // 1. Check installed extension (sidecar then project)
        let ext_dir = sidecar_ext_dir.as_ref()
            .map(|d| d.join(&name))
            .filter(|d| d.join("extension.toml").exists())
            .unwrap_or_else(|| project_ext_dir.join(&name));
        if let Some(manifest) = load_cached(&ext_dir) {
            current = manifest.inherits.clone();
            continue;
        }

        // 1b. Check named targets in installed extensions
        {
            let extensions = discover_extensions(project_root);
            let mut found = false;
            for (_ext_name, ext_path) in &extensions {
                if let Some(manifest) = load_cached(ext_path) {
                    if let Some(et) = manifest.targets.get(&name) {
                        current = et.inherits.clone();
                        found = true;
                        break;
                    }
                }
            }
            if found { continue; }
        }

        // 2. Check built-in extension manifest
        if let Some(manifest) = builtin_extension(&name) {
            current = manifest.inherits.clone();
            continue;
        }

        // 3. Check user target definitions (pre-resolution, with inherits intact)
        if let Some(target) = user_targets.get(&name) {
            current = target.inherits.clone();
            continue;
        }
    }

    chain
}

// ---------------------------------------------------------------------------
// Extension name validation
// ---------------------------------------------------------------------------

/// Validate that an extension directory name is safe and well-formed.
fn is_valid_extension_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
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
    ("pdf", include_str!("../extensions/pdf/extension.toml")),
    ("slides", include_str!("../extensions/slides/extension.toml")),
    ("website", include_str!("../extensions/website/extension.toml")),
    ("book", include_str!("../extensions/book/extension.toml")),
    ("minimal", include_str!("../extensions/minimal/extension.toml")),
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
    fn test_slides_inherits_html() {
        let manifest = builtin_extension("slides").unwrap();
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
