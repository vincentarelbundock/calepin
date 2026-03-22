//! Centralized path resolution, validation, and context.
//!
//! All input paths resolve relative to `document_dir` (the parent of the .qmd file).
//! The output directory is where finished files are written; no inputs resolve from it.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::types::Metadata;

// ---------------------------------------------------------------------------
// PathContext
// ---------------------------------------------------------------------------

/// Path context carried through the render pipeline.
///
/// All input paths resolve relative to `document_dir`. The output directory
/// is only for writing; no input files are ever resolved from it.
#[derive(Debug, Clone)]
pub struct PathContext {
    /// Parent directory of the .qmd file being rendered.
    /// All input paths (bibliography, css, includes, plugins, _calepin/) resolve from here.
    pub document_dir: PathBuf,
    /// Where output files are written. No input files resolve from here.
    pub output_dir: PathBuf,
    /// Subdirectory name for generated figures (default: "_calepin_files").
    pub files_dir: String,
    /// Subdirectory name for execution cache (default: "_calepin_cache").
    pub cache_dir: String,
}

impl PathContext {
    /// Build a PathContext for a single-file render.
    pub fn for_single_file(input: &Path, output: &Path) -> Self {
        let document_dir = input.parent().unwrap_or(Path::new(".")).to_path_buf();
        let output_dir = output.parent().unwrap_or(Path::new(".")).to_path_buf();
        Self {
            document_dir,
            output_dir,
            files_dir: "_calepin_files".to_string(),
            cache_dir: "_calepin_cache".to_string(),
        }
    }

    /// Apply overrides from parsed metadata (calepin.files-dir, calepin.cache-dir).
    pub fn apply_metadata(&mut self, meta: &Metadata) {
        if let Some(ref d) = meta.files_dir {
            self.files_dir = d.clone();
        }
        if let Some(ref d) = meta.cache_dir {
            self.cache_dir = d.clone();
        }
    }

    /// Resolve the figure output directory for a given document stem.
    pub fn figures_dir(&self, stem: &str) -> PathBuf {
        self.output_dir.join(&self.files_dir).join(stem)
    }

    /// Resolve the cache directory for a given document stem.
    pub fn cache_root(&self, stem: &str) -> PathBuf {
        self.document_dir.join(&self.cache_dir).join(stem)
    }
}

// ---------------------------------------------------------------------------
// Path resolution
// ---------------------------------------------------------------------------

/// Resolve a file by checking document-local then user-global directories.
/// Returns the first path that exists, or None.
///
/// Resolution order:
///   1. `{document_dir}/_calepin/{dir}/{filename}`
///   2. `~/.config/calepin/{dir}/{filename}`
pub fn resolve_path(document_dir: &Path, dir: &str, filename: &str) -> Option<PathBuf> {
    let local = document_dir.join("_calepin").join(dir).join(filename);
    if local.exists() {
        return Some(local);
    }

    if let Ok(home) = std::env::var("HOME") {
        let user = Path::new(&home)
            .join(".config/calepin")
            .join(dir)
            .join(filename);
        if user.exists() {
            return Some(user);
        }
    }

    None
}

/// Backward-compatible wrapper: resolves relative to CWD.
pub fn resolve_path_cwd(dir: &str, filename: &str) -> Option<PathBuf> {
    resolve_path(Path::new("."), dir, filename)
}

// ---------------------------------------------------------------------------
// New three-layer resolution (project root / user config / built-in)
// ---------------------------------------------------------------------------

/// Map a base name to its file extension for template/component lookup.
/// Derives the mapping from the built-in calepin.toml.
pub fn base_to_ext(base: &str) -> &str {
    let target = crate::project::builtin_config().targets.get(base);
    target
        .and_then(|t| t.extension.as_deref())
        .unwrap_or(base)
}

/// Resolve a component (element template) using the three-layer model.
///
/// Lookup order (first match wins):
///   1. `{project_root}/components/{base}/{name}.{ext}` (base-specific)
///   2. `{project_root}/components/common/{name}.jinja` (generic)
///   3. `~/.config/calepin/components/{base}/{name}.{ext}`
///   4. `~/.config/calepin/components/common/{name}.jinja`
///   5. (caller falls back to built-in)
///
/// Also checks legacy `_calepin/elements/{name}.{ext}` for backward compatibility.
pub fn resolve_component(name: &str, base: &str) -> Option<PathBuf> {
    let ext = base_to_ext(base);
    let base_specific = format!("{}.{}", name, ext);
    let generic = format!("{}.jinja", name);

    // Project root (CWD for now)
    let root = Path::new(".");

    // Base-specific in project
    let p = root.join("components").join(base).join(&base_specific);
    if p.exists() { return Some(p); }

    // Generic in project
    let p = root.join("components").join("common").join(&generic);
    if p.exists() { return Some(p); }

    // Legacy: _calepin/elements/
    let p = root.join("_calepin").join("elements").join(&base_specific);
    if p.exists() { return Some(p); }

    // User config
    if let Ok(home) = std::env::var("HOME") {
        let user = Path::new(&home).join(".config/calepin");
        let p = user.join("components").join(base).join(&base_specific);
        if p.exists() { return Some(p); }
        let p = user.join("components").join("common").join(&generic);
        if p.exists() { return Some(p); }
        // Legacy user path
        let p = user.join("elements").join(&base_specific);
        if p.exists() { return Some(p); }
    }

    None
}

/// Resolve a page template using the three-layer model.
///
/// Lookup order (first match wins):
///   1. `{project_root}/templates/{base}/{template_name}.{ext}`
///   2. `{project_root}/templates/common/{template_name}.jinja`
///   3. `~/.config/calepin/templates/{base}/{template_name}.{ext}`
///   4. `~/.config/calepin/templates/common/{template_name}.jinja`
///   5. (caller falls back to built-in)
///
/// Also checks legacy `_calepin/templates/` for backward compatibility.
pub fn resolve_template(template_name: &str, base: &str) -> Option<PathBuf> {
    let ext = base_to_ext(base);
    let base_specific = format!("{}.{}", template_name, ext);
    let generic = format!("{}.jinja", template_name);

    let root = Path::new(".");

    // Base-specific in project
    let p = root.join("templates").join(base).join(&base_specific);
    if p.exists() { return Some(p); }

    // Generic in project
    let p = root.join("templates").join("common").join(&generic);
    if p.exists() { return Some(p); }

    // Legacy: _calepin/templates/
    // Map base name to legacy filename format (e.g., "calepin.html")
    let legacy_name = format!("{}.{}", template_name, base);
    let p = root.join("_calepin").join("templates").join(&legacy_name);
    if p.exists() { return Some(p); }

    // User config
    if let Ok(home) = std::env::var("HOME") {
        let user = Path::new(&home).join(".config/calepin");
        let p = user.join("templates").join(base).join(&base_specific);
        if p.exists() { return Some(p); }
        let p = user.join("templates").join("common").join(&generic);
        if p.exists() { return Some(p); }
        // Legacy user path
        let p = user.join("templates").join(&legacy_name);
        if p.exists() { return Some(p); }
    }

    None
}

/// Find the first file matching an extension in `{document_dir}/_calepin/{dir}/`
/// then `~/.config/calepin/{dir}/`.
/// Returns the alphabetically first match.
pub fn resolve_first_match(document_dir: &Path, dir: &str, extension: &str) -> Option<PathBuf> {
    let dirs: Vec<PathBuf> = {
        let mut v = vec![document_dir.join("_calepin").join(dir)];
        if let Ok(home) = std::env::var("HOME") {
            v.push(Path::new(&home).join(".config/calepin").join(dir));
        }
        v
    };
    for d in &dirs {
        if let Ok(entries) = std::fs::read_dir(d) {
            let mut matches: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(extension))
                .collect();
            matches.sort();
            if let Some(first) = matches.into_iter().next() {
                return Some(first);
            }
        }
    }
    None
}

/// Resolve a plugin directory by name.
/// Checks `{document_dir}/_calepin/plugins/{name}/plugin.toml` (or `plugin.yml`)
/// then `~/.config/calepin/plugins/{name}/plugin.toml` (or `plugin.yml`).
pub fn resolve_plugin_dir(name: &str, document_dir: &Path) -> Option<PathBuf> {
    let local = document_dir.join("_calepin").join("plugins").join(name);
    if local.join("plugin.toml").exists() || local.join("plugin.yml").exists() {
        return Some(local);
    }

    if let Ok(home) = std::env::var("HOME") {
        let user = Path::new(&home).join(".config/calepin/plugins").join(name);
        if user.join("plugin.toml").exists() || user.join("plugin.yml").exists() {
            return Some(user);
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validate all path-bearing fields in metadata against the filesystem.
/// Returns Ok(()) if all paths resolve, or an error listing every missing path.
pub fn validate_paths(meta: &Metadata, ctx: &PathContext, input_name: &str) -> Result<()> {
    let mut errors: Vec<String> = Vec::new();

    // Bibliography files
    for bib in &meta.bibliography {
        let resolved = ctx.document_dir.join(bib);
        if !resolved.exists() {
            errors.push(format!(
                "  bibliography: {}\n    -> not found: {}",
                bib,
                resolved.display()
            ));
        }
    }

    // CSL file (only if explicitly specified)
    if let Some(ref csl) = meta.csl {
        let resolved = ctx.document_dir.join(csl);
        if !resolved.exists() {
            errors.push(format!(
                "  csl: {}\n    -> not found: {}",
                csl,
                resolved.display()
            ));
        }
    }

    // Plugins
    for plugin in &meta.plugins {
        if is_builtin_plugin(plugin) {
            continue;
        }
        let local_dir = ctx.document_dir.join("_calepin/plugins").join(plugin);
        let local_path = local_dir.join("plugin.toml");
        let user_path = std::env::var("HOME").ok().map(|h| {
            Path::new(&h).join(".config/calepin/plugins").join(plugin).join("plugin.toml")
        });
        let found = local_dir.join("plugin.toml").exists()
            || local_dir.join("plugin.yml").exists()
            || user_path.as_ref().map_or(false, |p| p.exists())
            || user_path.as_ref().map_or(false, |p| p.with_file_name("plugin.yml").exists());
        if !found {
            let mut msg = format!(
                "  calepin.plugins: {}\n    -> not found: {}",
                plugin,
                local_path.display()
            );
            if let Some(ref up) = user_path {
                msg.push_str(&format!("\n    -> not found: {}", up.display()));
            }
            errors.push(msg);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        let count = errors.len();
        bail!(
            "{} path error{} in {}:\n\n{}",
            count,
            if count == 1 { "" } else { "s" },
            input_name,
            errors.join("\n\n")
        );
    }
}

/// Built-in plugin names that don't need filesystem resolution.
fn is_builtin_plugin(name: &str) -> bool {
    matches!(name, "tabset" | "layout" | "figure-div" | "table-div" | "theorem" | "callout")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx() -> PathContext {
        PathContext {
            document_dir: PathBuf::from("/nonexistent/dir"),
            output_dir: PathBuf::from("/nonexistent/dir"),
            files_dir: "_calepin_files".to_string(),
            cache_dir: "_calepin_cache".to_string(),
        }
    }

    #[test]
    fn test_empty_metadata_is_valid() {
        let meta = Metadata::default();
        assert!(validate_paths(&meta, &test_ctx(), "test.qmd").is_ok());
    }

    #[test]
    fn test_missing_bibliography() {
        let mut meta = Metadata::default();
        meta.bibliography = vec!["missing.bib".to_string()];
        let err = validate_paths(&meta, &test_ctx(), "test.qmd").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("bibliography: missing.bib"), "got: {}", msg);
        assert!(msg.contains("1 path error"), "got: {}", msg);
    }

    #[test]
    fn test_missing_plugin() {
        let mut meta = Metadata::default();
        meta.plugins = vec!["nonexistent-plugin".to_string()];
        let err = validate_paths(&meta, &test_ctx(), "test.qmd").unwrap_err();
        assert!(err.to_string().contains("calepin.plugins: nonexistent-plugin"));
    }

    #[test]
    fn test_builtin_plugin_not_validated() {
        let mut meta = Metadata::default();
        meta.plugins = vec!["tabset".to_string(), "callout".to_string()];
        assert!(validate_paths(&meta, &test_ctx(), "test.qmd").is_ok());
    }

    #[test]
    fn test_multiple_errors_collected() {
        let mut meta = Metadata::default();
        meta.bibliography = vec!["missing.bib".to_string()];
        meta.csl = Some("missing.csl".to_string());
        let err = validate_paths(&meta, &test_ctx(), "test.qmd").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("2 path errors"), "got: {}", msg);
    }
}
