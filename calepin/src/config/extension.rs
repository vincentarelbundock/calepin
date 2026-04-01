//! Extension manifest parsing (`extension.toml`).
//!
//! An extension bundles a target definition, templates, modules, assets,
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
    /// Scaffold declarations (document, website, book).
    #[serde(default)]
    pub scaffold: ExtensionScaffolds,
    /// Whether this extension defines a collection target (website, book).
    #[serde(default)]
    pub collection: bool,
    /// Name of the default color scheme (looked up from built-in or installed schemes).
    #[serde(default)]
    pub default_colors: Option<String>,
    /// Single color scheme definition (used by color scheme extensions in `src/themes/`).
    #[serde(default)]
    pub colors: Option<ColorsDef>,
    /// Inline named color schemes (for bundling multiple schemes).
    #[serde(default)]
    pub themes: HashMap<String, ColorsDef>,
}

/// Scaffold declarations in an extension manifest.
/// Keys are scaffold names (e.g., "website", "book", "document").
pub type ExtensionScaffolds = HashMap<String, ScaffoldInfo>;

/// Metadata for a scaffold declaration.
#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)]
pub struct ScaffoldInfo {
    /// Project kind: "collection" (directory project) or "document" (single .qmd).
    #[serde(default = "default_scaffold_kind")]
    pub kind: String,
    /// Short description of what this scaffold creates.
    #[serde(default)]
    pub description: String,
}

fn default_scaffold_kind() -> String {
    "collection".to_string()
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
    pub standalone: Option<bool>,
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
#[allow(dead_code)]
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
        standalone: et.standalone,
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
    #[allow(dead_code)]
    pub number: bool,
}

impl ExtensionMatchRule {
    /// Check if this match rule applies to the given element properties.
    pub fn matches(
        &self,
        classes: &[String],
        attrs: &std::collections::HashMap<String, String>,
        id: Option<&str>,
        format: &str,
    ) -> bool {
        if !self.writers.is_empty() && !self.writers.iter().any(|f| f == format) {
            return false;
        }
        if self.classes.iter().any(|c| classes.iter().any(|cls| cls == c)) {
            return true;
        }
        if self.attrs.iter().any(|a| attrs.contains_key(a)) {
            return true;
        }
        if let (Some(prefix), Some(id_val)) = (&self.id_prefix, id) {
            if id_val.starts_with(prefix.as_str()) {
                return true;
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Theme definitions
// ---------------------------------------------------------------------------

/// A color scheme definition bundling CSS color tokens and syntax highlighting themes.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ColorsDef {
    /// Syntax highlighting theme names (light/dark).
    #[serde(default)]
    pub highlight: Option<ColorsHighlight>,
    /// CSS custom property color tokens (light/dark).
    #[serde(default)]
    pub tokens: Option<ColorTokens>,
}

/// Syntax highlighting theme pair for a color scheme.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ColorsHighlight {
    /// Light-mode `.tmTheme` name.
    pub light: Option<String>,
    /// Dark-mode `.tmTheme` name.
    pub dark: Option<String>,
}

/// CSS custom property color tokens for light and dark modes.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ColorTokens {
    /// Light-mode color values keyed by token name (e.g., "page" -> "#fafbfc").
    #[serde(default)]
    pub light: HashMap<String, String>,
    /// Dark-mode color values keyed by token name (e.g., "page" -> "#2f3643").
    #[serde(default)]
    pub dark: HashMap<String, String>,
}

impl ColorsDef {
    /// Generate CSS custom property declarations from this theme's colors.
    /// Returns a string like `:root { --c-page: #fff; ... } .dark { --c-page: #111; ... }`
    pub fn generate_color_css(&self) -> String {
        let colors = match &self.tokens {
            Some(c) if !c.light.is_empty() || !c.dark.is_empty() => c,
            _ => return String::new(),
        };
        let mut css = String::new();
        if !colors.light.is_empty() {
            css.push_str(":root {\n");
            let mut keys: Vec<&String> = colors.light.keys().collect();
            keys.sort();
            for key in keys {
                css.push_str(&format!("  --c-{}: {};\n", key, colors.light[key]));
            }
            css.push_str("}\n");
        }
        if !colors.dark.is_empty() {
            css.push_str(".dark {\n");
            let mut keys: Vec<&String> = colors.dark.keys().collect();
            keys.sort();
            for key in keys {
                css.push_str(&format!("  --c-{}: {};\n", key, colors.dark[key]));
            }
            css.push_str("  color-scheme: dark;\n");
            css.push_str("}\n");
        }
        css
    }

    /// Generate the Tailwind CSS `theme.extend.colors` JS object body from this theme's
    /// token keys. Each token `foo` becomes `foo: 'var(--c-foo)'`. The `border` token is
    /// aliased to `brd` to avoid colliding with Tailwind's built-in `border` utility.
    /// Returns the inner content of the JS object (no surrounding braces).
    pub fn generate_tailwind_colors(&self) -> String {
        let colors = match &self.tokens {
            Some(c) => c,
            None => return String::new(),
        };
        // Collect all unique token names from light and dark
        let mut keys: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for k in colors.light.keys() { keys.insert(k.as_str()); }
        for k in colors.dark.keys() { keys.insert(k.as_str()); }

        let mut lines = Vec::new();
        for key in &keys {
            // Tailwind class name: alias "border*" to "brd*" to avoid collision
            let tw_name = if *key == "border" {
                "brd".to_string()
            } else if let Some(suffix) = key.strip_prefix("border-") {
                format!("brd-{}", suffix)
            } else {
                key.to_string()
            };
            // Quote names with dashes
            if tw_name.contains('-') {
                lines.push(format!("        '{}': 'var(--c-{})'", tw_name, key));
            } else {
                lines.push(format!("        {}: 'var(--c-{})'", tw_name, key));
            }
        }
        lines.join(",\n")
    }
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
            standalone: ext_target.and_then(|t| t.standalone),
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

    /// Check if this extension provides a scaffold with the given name.
    pub fn has_scaffold(&self, name: &str) -> bool {
        self.scaffold.contains_key(name)
    }

    /// Get scaffold info by name.
    pub fn get_scaffold(&self, name: &str) -> Option<&ScaffoldInfo> {
        self.scaffold.get(name)
    }

}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// Discover installed extensions in the root sidecar's `extensions/` directory.
#[allow(dead_code)]
pub fn discover_extensions(project_root: &Path) -> Vec<(String, PathBuf)> {
    discover_extensions_in(&crate::paths::extensions_dir(project_root))
}

/// Discover extensions in an arbitrary directory.
pub fn discover_extensions_in(extensions_dir: &Path) -> Vec<(String, PathBuf)> {
    if !extensions_dir.is_dir() {
        return Vec::new();
    }
    let mut found = Vec::new();
    if let Ok(entries) = std::fs::read_dir(extensions_dir) {
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

/// Load all installed extensions from sidecar `extensions/`.
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
    let project_ext_dir = crate::paths::extensions_dir(project_root);
    let sidecar_ext_dir = crate::paths::get_page_sidecar()
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
        // Check named targets in installed extensions (sidecar then project)
        let sidecar_exts = sidecar_ext_dir.as_ref()
            .map(|d| discover_extensions_in(d))
            .unwrap_or_default();
        let project_exts = discover_extensions(project_root);
        let all_extensions: Vec<_> = sidecar_exts.iter().chain(project_exts.iter()).collect();
        for (_ext_name, ext_path) in all_extensions {
            if let Some(manifest) = load_cached(ext_path) {
                if let Some(et) = manifest.targets.get(&name) {
                    // Include the owning extension's templates/assets
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

/// Check whether a target name resolves to a collection type (website, book, etc.).
/// Walks the extension inheritance chain and built-in manifests, returning true
/// if any extension in the chain has `collection = true`.
pub fn is_collection_target(target_name: &str) -> bool {
    // Check installed extensions via walk_chain (checks sidecar, project, built-in)
    let results = walk_chain(
        &crate::paths::get_project_dir(),
        target_name,
        |_, _, manifest| {
            if manifest.collection { Some(true) } else { None }
        },
    );
    if results.contains(&true) {
        return true;
    }
    // Walk built-in manifests directly for targets not found via walk_chain
    let mut current = Some(target_name.to_string());
    let mut visited = std::collections::HashSet::new();
    while let Some(name) = current.take() {
        if !visited.insert(name.clone()) { break; }
        if let Some(manifest) = builtin_extension(&name) {
            if manifest.collection { return true; }
            current = manifest.inherits.clone();
        }
    }
    false
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
    let project_ext_dir = crate::paths::extensions_dir(project_root);
    let sidecar_ext_dir = crate::paths::get_page_sidecar()
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
// Theme resolution
// ---------------------------------------------------------------------------

/// Resolve the default color scheme for a target by walking the extension inheritance chain.
/// Finds the first `default_colors` field, then looks up the scheme by name from
/// installed color extensions, built-in registry, or inline `[themes.*]`.
pub fn resolve_default_colors(target_name: &str) -> Option<ColorsDef> {
    let project_root = crate::paths::get_project_dir();

    // Find the default_colors name from the extension chain
    let default_name = {
        // 1. Installed extensions (child-first)
        let results = walk_chain(&project_root, target_name, |_, _, manifest| {
            manifest.default_colors.clone()
        });
        if let Some(name) = results.into_iter().next() {
            Some(name)
        } else {
            // 2. Built-in extension manifests
            let mut current = Some(target_name.to_string());
            let mut visited = std::collections::HashSet::new();
            let mut found = None;
            while let Some(name) = current.take() {
                if !visited.insert(name.clone()) { break; }
                if let Some(manifest) = builtin_extension(&name) {
                    if manifest.default_colors.is_some() {
                        found = manifest.default_colors;
                        break;
                    }
                    current = manifest.inherits;
                }
            }
            found
        }
    };

    // Look up the color scheme by name
    let default_name = default_name?;
    resolve_colors_by_name(&default_name)
}

/// Look up a color scheme by name: user-installed extensions, then built-in schemes.
pub fn resolve_colors_by_name(name: &str) -> Option<ColorsDef> {
    // 1. User-installed color extension in sidecar or project
    let project_root = crate::paths::get_project_dir();
    let ext_dir = crate::paths::extensions_dir(&project_root).join(name);
    if let Some(manifest) = load_cached(&ext_dir) {
        if manifest.colors.is_some() {
            return manifest.colors;
        }
    }

    // 2. Built-in color scheme registry
    builtin_colors(name)
}

/// Collect all available color schemes for a target (for the theme picker).
/// Gathers schemes from user-installed extensions and built-in schemes,
/// deduplicated by name.
pub fn collect_color_schemes(target_name: &str) -> Vec<(String, ColorsDef)> {
    let project_root = crate::paths::get_project_dir();
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();

    // 1. User-installed color extensions (sidecar/project)
    let extensions_dir = crate::paths::extensions_dir(&project_root);
    if extensions_dir.is_dir() {
        for (name, path) in discover_extensions_in(&extensions_dir) {
            if let Some(manifest) = load_cached(&path) {
                if let Some(colors) = manifest.colors {
                    if seen.insert(name.clone()) {
                        result.push((name, colors));
                    }
                }
            }
        }
    }

    // 2. Inline [themes.*] from extension chain
    let chain_themes = walk_chain(&project_root, target_name, |_, _, manifest| {
        if manifest.themes.is_empty() { None } else { Some(manifest.themes.clone()) }
    });
    for themes_map in chain_themes {
        for (name, def) in themes_map {
            if seen.insert(name.clone()) {
                result.push((name, def));
            }
        }
    }

    // 3. Built-in color schemes
    for (name, content) in BUILTIN_COLOR_EXTENSIONS {
        if seen.insert(name.to_string()) {
            if let Ok(manifest) = toml::from_str::<ExtensionManifest>(content) {
                if let Some(colors) = manifest.colors {
                    result.push((name.to_string(), colors));
                }
            }
        }
    }

    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

/// Resolve the active color scheme for a document.
/// Priority: first entry in `cfg.colors` > extension `default_colors`.
pub fn resolve_active_colors(
    cfg: &std::collections::HashMap<String, crate::value::Value>,
    target_name: &str,
) -> Option<ColorsDef> {
    let user_default = cfg.get("colors")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if let Some(ref name) = user_default {
        resolve_colors_by_name(name)
    } else {
        resolve_default_colors(target_name)
    }
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
    ("latex", include_str!("../targets/latex/extension.toml")),
    ("typst", include_str!("../targets/typst/extension.toml")),
    ("markdown", include_str!("../targets/markdown/extension.toml")),
    ("pdf", include_str!("../targets/pdf/extension.toml")),
    ("pdf-latex", include_str!("../targets/pdf-latex/extension.toml")),
    ("slides", include_str!("../targets/slides/extension.toml")),
    ("book", include_str!("../targets/book/extension.toml")),
    ("book-latex", include_str!("../targets/book-latex/extension.toml")),
    ("html", include_str!("../targets/html/extension.toml")),
    ("website", include_str!("../targets/website/extension.toml")),
];

/// Built-in color scheme manifest sources, embedded at compile time.
/// Each is a standalone extension with a `[colors]` section.
pub const BUILTIN_COLOR_EXTENSIONS: &[(&str, &str)] = &[
    ("nord", include_str!("../themes/nord/extension.toml")),
    ("ayu", include_str!("../themes/ayu/extension.toml")),
    ("catppuccin-frappe", include_str!("../themes/catppuccin-frappe/extension.toml")),
    ("catppuccin-macchiato", include_str!("../themes/catppuccin-macchiato/extension.toml")),
    ("catppuccin-mocha", include_str!("../themes/catppuccin-mocha/extension.toml")),
    ("black", include_str!("../themes/black/extension.toml")),
];

/// Built-in color scheme directories, embedded at compile time.
/// Each contains an extension.toml and .tmTheme files.
pub static BUILTIN_COLOR_DIRS: &[(&str, &Dir<'static>)] = &[
    ("nord", &COLOR_DIR_NORD),
    ("ayu", &COLOR_DIR_AYU),
    ("catppuccin-frappe", &COLOR_DIR_CATPPUCCIN_FRAPPE),
    ("catppuccin-macchiato", &COLOR_DIR_CATPPUCCIN_MACCHIATO),
    ("catppuccin-mocha", &COLOR_DIR_CATPPUCCIN_MOCHA),
    ("black", &COLOR_DIR_BLACK),
];

/// Parse a built-in extension manifest by name.
pub fn builtin_extension(name: &str) -> Option<ExtensionManifest> {
    BUILTIN_EXTENSIONS.iter()
        .find(|(n, _)| *n == name)
        .and_then(|(_, content)| toml::from_str(content).ok())
}

/// Parse a built-in color scheme by name.
/// Returns the `ColorsDef` from the color extension's `[colors]` section.
pub fn builtin_colors(name: &str) -> Option<ColorsDef> {
    BUILTIN_COLOR_EXTENSIONS.iter()
        .find(|(n, _)| *n == name)
        .and_then(|(_, content)| toml::from_str::<ExtensionManifest>(content).ok())
        .and_then(|m| m.colors)
}

/// Return names of all built-in color schemes.
pub fn builtin_color_names() -> Vec<&'static str> {
    BUILTIN_COLOR_EXTENSIONS.iter().map(|(n, _)| *n).collect()
}

/// Look up a `.tmTheme` file from built-in color scheme extension directories.
/// Returns the raw bytes if found.
pub fn builtin_colors_tmtheme(filename: &str) -> Option<&'static [u8]> {
    for (_name, dir) in BUILTIN_COLOR_DIRS {
        if let Some(file) = dir.get_file(filename) {
            return Some(file.contents());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Built-in scaffolds (embedded at compile time)
// ---------------------------------------------------------------------------

use include_dir::{include_dir, Dir};

/// Built-in scaffold directories, embedded from extensions that declare scaffolds.
/// Each entry is `(extension_name, scaffold_name, embedded_dir)`.
pub static BUILTIN_SCAFFOLDS: &[(&str, &str, &Dir<'static>)] = &[
    ("website", "website", &SCAFFOLD_WEBSITE),
    ("book", "book", &SCAFFOLD_BOOK),
    ("book-latex", "book-latex", &SCAFFOLD_BOOK_LATEX),
    ("html", "document", &SCAFFOLD_DOCUMENT),
];

static COLOR_DIR_NORD: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/themes/nord");
static COLOR_DIR_AYU: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/themes/ayu");
static COLOR_DIR_CATPPUCCIN_FRAPPE: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/themes/catppuccin-frappe");
static COLOR_DIR_CATPPUCCIN_MACCHIATO: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/themes/catppuccin-macchiato");
static COLOR_DIR_CATPPUCCIN_MOCHA: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/themes/catppuccin-mocha");
static COLOR_DIR_BLACK: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/themes/black");

static SCAFFOLD_WEBSITE: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/targets/website/scaffold/website");
static SCAFFOLD_BOOK: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/targets/book/scaffold/book");
static SCAFFOLD_BOOK_LATEX: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/targets/book-latex/scaffold/book-latex");
static SCAFFOLD_DOCUMENT: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/targets/html/scaffold/document");

/// Look up a built-in scaffold by name (e.g., "website", "book", "document").
/// Returns the extension name and the embedded directory.
pub fn builtin_scaffold(name: &str) -> Option<(&'static str, &'static Dir<'static>)> {
    BUILTIN_SCAFFOLDS.iter()
        .find(|(_, n, _)| *n == name)
        .map(|(ext, _, dir)| (*ext, *dir))
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
    fn test_book_is_collection() {
        assert!(builtin_extension("book").unwrap().collection);
        assert!(builtin_extension("book-latex").unwrap().collection);
    }

    #[test]
    fn test_collection_field_on_builtins() {
        assert!(builtin_extension("website").unwrap().collection);
        assert!(builtin_extension("book").unwrap().collection);
        assert!(builtin_extension("book-latex").unwrap().collection);
        assert!(!builtin_extension("html").unwrap().collection);
        assert!(!builtin_extension("latex").unwrap().collection);
        assert!(!builtin_extension("slides").unwrap().collection);
    }

    #[test]
    fn test_is_collection_target() {
        assert!(is_collection_target("website"));
        assert!(is_collection_target("book"));
        assert!(is_collection_target("book-latex"));
        assert!(!is_collection_target("html"));
        assert!(!is_collection_target("latex"));
        assert!(!is_collection_target("typst"));
    }
}
