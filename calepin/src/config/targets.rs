//! Target resolution, validation, and inheritance.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Target types
// ---------------------------------------------------------------------------

/// A named output profile.
#[derive(Debug, Clone, Deserialize)]
pub struct Target {
    /// Inherit all fields from another target.
    pub inherits: Option<String>,
    /// Output writer: html, latex, typst, or markdown.
    #[serde(default)]
    pub writer: String,
    /// Document template name (default: "main").
    pub template: Option<String>,
    /// Output file extension (no dot).
    pub extension: Option<String>,
    /// Default extension for generated figures.
    pub fig_extension: Option<String>,
    /// Preview behavior: "serve", "open", or "none".
    pub preview: Option<String>,
    /// Whether to produce a self-contained file (inline CSS/JS, base64 images).
    pub standalone: Option<bool>,
    /// Arbitrary key-value pairs passed to templates as target_vars.
    pub vars: Option<toml::Value>,
    /// Post-processing commands run after rendering.
    #[serde(default)]
    pub post: Vec<String>,
    /// Body transform modules applied after element rendering, before crossref.
    /// Order matters. Named modules are resolved from the built-in registry.
    #[serde(default)]
    pub modules: Vec<String>,
    /// Cross-reference resolution strategy: "html", "latex", or "plain".
    /// Default: inferred from writer.
    pub crossref: Option<String>,
    /// Whether to pass headings to the page template for TOC generation.
    /// Default: true for html, false for latex.
    pub toc_headings: Option<bool>,
    /// Extra template variables injected during page assembly.
    /// These override computed values. Useful for setting `base = "html"`
    /// in slides or other derived targets.
    #[serde(default)]
    pub page_vars: HashMap<String, String>,
    /// Preferred image formats for figure variant selection, in priority order.
    /// Default: writer-appropriate list (e.g., ["svg", "png", "jpg"] for html).
    #[serde(default)]
    pub fig_formats: Vec<String>,
}

impl Target {
    /// Template name. Always set after resolution against the built-in config.
    #[allow(dead_code)]
    pub fn template_name(&self) -> &str {
        self.template.as_deref().unwrap_or("main")
    }

    /// Output file extension. Always set after resolution against the built-in config.
    pub fn output_extension(&self) -> &str {
        self.extension.as_deref().unwrap_or(&self.writer)
    }

    /// Default figure extension. Always set after resolution against the built-in config.
    #[allow(dead_code)]
    pub fn fig_ext(&self) -> &str {
        self.fig_extension.as_deref().unwrap_or("png")
    }

}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Valid writer names.
pub const VALID_WRITERS: &[&str] = &["html", "latex", "typst", "markdown"];

impl Target {
    /// Validate a target's fields. Returns a descriptive error on failure.
    pub fn validate(&self) -> Result<()> {
        if !VALID_WRITERS.contains(&self.writer.as_str()) {
            bail!("writer must be one of: {} (got '{}')", VALID_WRITERS.join(", "), self.writer);
        }
        if let Some(ref ext) = self.extension {
            validate_extension(ext, "extension")?;
        }
        if let Some(ref ext) = self.fig_extension {
            validate_extension(ext, "fig-extension")?;
        }
        if let Some(ref mode) = self.preview {
            match mode.as_str() {
                "serve" | "open" | "none" => {}
                other => bail!("preview must be one of: serve, open, none (got '{}')", other),
            }
        }
        Ok(())
    }
}

fn validate_extension(ext: &str, field: &str) -> Result<()> {
    if ext.is_empty() || !ext.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) {
        bail!("{} must be lowercase alphanumeric (got '{}')", field, ext);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Inheritance resolution
// ---------------------------------------------------------------------------

/// Resolve `inherits` chains across all targets. Supports chained inheritance.
/// Detects cycles.
pub fn resolve_inheritance(targets: &mut HashMap<String, Target>) -> Result<()> {
    let names: Vec<String> = targets.keys().cloned().collect();
    for name in &names {
        let mut resolved = resolve_one(name, targets, &mut Vec::new())?;
        resolved.inherits = None;
        targets.insert(name.clone(), resolved);
    }
    Ok(())
}

/// Recursively resolve a single target's inheritance chain.
fn resolve_one(
    name: &str,
    targets: &HashMap<String, Target>,
    chain: &mut Vec<String>,
) -> Result<Target> {
    if chain.contains(&name.to_string()) {
        bail!("circular inheritance: {} -> {}", chain.join(" -> "), name);
    }

    let target = targets.get(name)
        .cloned()
        .or_else(|| super::builtin_metadata().targets.get(name).cloned())
        .or_else(|| resolve_extension_target(name).ok().flatten())
        .or_else(|| super::extension::builtin_extension(name).map(|m| m.to_target()))
        .ok_or_else(|| anyhow::anyhow!(
            "target '{}' not found (referenced in inherits chain: {})",
            name, chain.join(" -> "),
        ))?;

    if let Some(ref parent_name) = target.inherits {
        chain.push(name.to_string());
        let parent = resolve_one(parent_name, targets, chain)?;
        chain.pop();
        Ok(merge_targets(&parent, &target))
    } else {
        Ok(target)
    }
}

/// Merge a child target on top of a parent. Child fields override parent fields.
fn merge_targets(parent: &Target, child: &Target) -> Target {
    Target {
        inherits: None,
        writer: if child.writer.is_empty() { parent.writer.clone() } else { child.writer.clone() },
        template: child.template.clone().or_else(|| parent.template.clone()),
        extension: child.extension.clone().or_else(|| parent.extension.clone()),
        fig_extension: child.fig_extension.clone().or_else(|| parent.fig_extension.clone()),
        preview: child.preview.clone().or_else(|| parent.preview.clone()),
        standalone: child.standalone.or(parent.standalone),
        vars: child.vars.clone().or_else(|| parent.vars.clone()),
        post: if child.post.is_empty() { parent.post.clone() } else { child.post.clone() },
        modules: if child.modules.is_empty() { parent.modules.clone() } else { child.modules.clone() },
        crossref: child.crossref.clone().or_else(|| parent.crossref.clone()),
        toc_headings: child.toc_headings.or(parent.toc_headings),
        page_vars: if child.page_vars.is_empty() { parent.page_vars.clone() } else { child.page_vars.clone() },
        fig_formats: if child.fig_formats.is_empty() { parent.fig_formats.clone() } else { child.fig_formats.clone() },
    }
}

// ---------------------------------------------------------------------------
// Target resolution
// ---------------------------------------------------------------------------

/// Resolve a target by name.
///
/// Lookup order:
///   1. Project config (`config.toml` found on disk)
///   2. Installed extensions (`{stem}_calepin/extensions/{name}/`)
///   3. Named targets in installed extensions
///   4. Built-in config (embedded default `config.toml`)
///   5. Built-in extension manifests (embedded in binary)
pub fn resolve_target(name: &str, targets: &std::collections::HashMap<String, Target>) -> Result<Target> {
    // 1. User targets
    if let Some(target) = targets.get(name) {
        return Ok(merge_with_builtin(target));
    }

    // 2. Installed extension as target
    if let Some(target) = resolve_extension_target(name)? {
        return Ok(merge_with_builtin(&target));
    }

    // 3. Named targets in installed extensions (sidecar then project)
    let project_root = crate::paths::get_project_dir();
    let sidecar_extensions = crate::paths::get_page_sidecar()
        .map(|s| super::extension::discover_extensions_in(&s.join("extensions")))
        .unwrap_or_default();
    let project_extensions = super::extension::discover_extensions(&project_root);
    for (_ext_name, ext_path) in sidecar_extensions.iter().chain(project_extensions.iter()) {
        if let Some(manifest) = super::extension::load_cached(ext_path) {
            if let Some(target) = manifest.named_target(name) {
                return Ok(merge_with_builtin(&resolve_with_inheritance(name, target)?));
            }
        }
    }

    // 4. Built-in config (from document.toml/collection.toml)
    if let Some(target) = super::builtin_metadata().targets.get(name) {
        return Ok(target.clone());
    }

    // 5. Built-in extension manifests (embedded in binary)
    if let Some(manifest) = super::extension::builtin_extension(name) {
        let target = manifest.to_target();
        return Ok(merge_with_builtin(&resolve_with_inheritance(name, target)?));
    }

    bail!(
        "Unknown target '{}'. Define it in your sidecar config.toml under [targets.{}].",
        name, name,
    )
}

/// Try to load an extension directory as a target (checks sidecar then project).
fn resolve_extension_target(name: &str) -> Result<Option<Target>> {
    let project_root = crate::paths::get_project_dir();
    let candidates = [
        crate::paths::get_page_sidecar().map(|s| s.join("extensions").join(name)),
        Some(crate::paths::extensions_dir(&project_root).join(name)),
    ];
    for dir in candidates.iter().flatten() {
        if dir.join("extension.toml").exists() {
            if let Ok(manifest) = super::extension::ExtensionManifest::load(dir) {
                let target = manifest.to_target();
                return Ok(Some(resolve_with_inheritance(name, target)?));
            }
        }
    }
    Ok(None)
}

/// If a target has `inherits`, resolve the chain; otherwise return as-is.
fn resolve_with_inheritance(name: &str, target: Target) -> Result<Target> {
    if target.inherits.is_some() {
        let mut target_map = HashMap::new();
        target_map.insert(name.to_string(), target);
        resolve_inheritance(&mut target_map)?;
        Ok(target_map.remove(name).unwrap())
    } else {
        Ok(target)
    }
}

/// Fill unset fields in a user target from the built-in target for the same base.
fn merge_with_builtin(user: &Target) -> Target {
    let builtin = super::builtin_metadata().targets.get(&user.writer);
    Target {
        inherits: None,
        writer: user.writer.clone(),
        template: user.template.clone().or_else(|| builtin.and_then(|b| b.template.clone())),
        extension: user.extension.clone().or_else(|| builtin.and_then(|b| b.extension.clone())),
        fig_extension: user.fig_extension.clone().or_else(|| builtin.and_then(|b| b.fig_extension.clone())),
        preview: user.preview.clone().or_else(|| builtin.and_then(|b| b.preview.clone())),
        standalone: user.standalone.or(builtin.and_then(|b| b.standalone)),
        vars: user.vars.clone(),
        post: user.post.clone(),
        modules: if user.modules.is_empty() {
            builtin.map(|b| b.modules.clone()).unwrap_or_default()
        } else {
            user.modules.clone()
        },
        crossref: user.crossref.clone().or_else(|| builtin.and_then(|b| b.crossref.clone())),
        toc_headings: user.toc_headings.or(builtin.and_then(|b| b.toc_headings)),
        page_vars: if user.page_vars.is_empty() {
            builtin.map(|b| b.page_vars.clone()).unwrap_or_default()
        } else {
            user.page_vars.clone()
        },
        fig_formats: if user.fig_formats.is_empty() {
            builtin.map(|b| b.fig_formats.clone()).unwrap_or_default()
        } else {
            user.fig_formats.clone()
        },
    }
}

/// Convert a `HashMap<String, MetaValue>` to a minijinja Value for template access.
pub fn build_jinja_vars(vars: &std::collections::HashMap<String, crate::value::Value>) -> minijinja::Value {
    if vars.is_empty() {
        return minijinja::Value::from(());
    }
    let json = crate::value::to_json(&crate::value::Value::Table(
        vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    ));
    minijinja::Value::from_serialize(&json)
}

// ---------------------------------------------------------------------------
// Output path resolution
// ---------------------------------------------------------------------------

/// Compute the output path for a target render.
pub fn resolve_target_output_path(
    input: &Path,
    target_name: &str,
    ext: &str,
    project_root: Option<&Path>,
    output_dir: Option<&str>,
) -> PathBuf {
    if let (Some(root), Some(out)) = (project_root, output_dir) {
        let abs_input = crate::paths::ensure_absolute(input);
        let relative = abs_input.strip_prefix(root)
            .unwrap_or(&abs_input);
        root.join(out).join(target_name).join(relative).with_extension(ext)
    } else {
        input.with_extension(ext)
    }
}
