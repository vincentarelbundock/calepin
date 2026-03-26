//! Centralized path resolution, validation, and context.
//!
//! All input paths resolve relative to the project root (the directory
//! containing `_calepin.toml`). For document renders without a project
//! config, the project root is the parent directory of the `.qmd` file.
//! The output directory is where finished files are written; no inputs
//! resolve from it.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::config::Metadata;

// ---------------------------------------------------------------------------
// Active target name (thread-local)
// ---------------------------------------------------------------------------

thread_local! {
    static ACTIVE_TARGET: RefCell<Option<String>> = RefCell::new(None);
    static PROJECT_ROOT: RefCell<Option<PathBuf>> = RefCell::new(None);
}

/// Set the active target name for template resolution.
/// When set, `resolve_template` checks `_calepin/templates/{target}/`
/// before `_calepin/templates/{base}/`.
pub fn set_active_target(target: Option<&str>) {
    ACTIVE_TARGET.with(|t| {
        *t.borrow_mut() = target.map(|s| s.to_string());
    });
}

pub fn get_active_target() -> Option<String> {
    ACTIVE_TARGET.with(|t| t.borrow().clone())
}

/// Set the project root for all path resolution.
pub fn set_project_root(root: Option<&Path>) {
    PROJECT_ROOT.with(|r| {
        *r.borrow_mut() = root.map(|p| p.to_path_buf());
    });
}

pub fn get_project_root() -> PathBuf {
    PROJECT_ROOT.with(|r| {
        r.borrow().clone().unwrap_or_else(|| PathBuf::from("."))
    })
}

// ---------------------------------------------------------------------------
// PathContext
// ---------------------------------------------------------------------------

/// Path context carried through the render pipeline.
///
/// All input paths resolve relative to `project_root` (the directory
/// containing `_calepin.toml`, or the `.qmd` parent in document mode).
/// The output directory is only for writing; no input files resolve from it.
#[derive(Debug, Clone)]
pub struct PathContext {
    /// Project root: directory containing `_calepin.toml`, or `.qmd` parent
    /// in document mode. All input paths resolve from here.
    pub project_root: PathBuf,
    /// Where output files are written. No input files resolve from here.
    pub output_dir: PathBuf,
}

impl PathContext {
    /// Construct a PathContext, with optional project root override.
    /// In document mode (no override), project_root = input's parent directory.
    pub fn new(input: &Path, output_path: &Path, project_root_override: Option<&Path>) -> Self {
        if let Some(root) = project_root_override {
            Self {
                project_root: root.to_path_buf(),
                output_dir: output_path.parent().unwrap_or(Path::new(".")).to_path_buf(),
            }
        } else {
            Self::for_document(input, output_path)
        }
    }

    /// Build a PathContext for a document render (no project root override).
    pub fn for_document(input: &Path, output: &Path) -> Self {
        let project_root = input.parent().unwrap_or(Path::new(".")).to_path_buf();
        let output_dir = output.parent().unwrap_or(Path::new(".")).to_path_buf();
        Self { project_root, output_dir }
    }

    /// Resolve the figure output directory for a given document stem.
    pub fn figures_dir(&self, stem: &str) -> PathBuf {
        calepin_dir(&self.output_dir, &["files", stem])
    }

    /// Resolve the cache directory for a given document stem.
    pub fn cache_root(&self, stem: &str) -> PathBuf {
        calepin_dir(&self.project_root, &["cache", stem])
    }

    /// Compute a relative stem from input path, for use as cache/figure key.
    /// Strips the project root prefix and extension, normalizes separators.
    pub fn relative_stem(&self, input: &Path) -> String {
        input.strip_prefix(&self.project_root)
            .unwrap_or(input)
            .with_extension("")
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// Working directory for code engines (R, Python, sh).
    /// Returns the input file's parent directory, or None if empty.
    pub fn code_working_dir(input: &Path) -> Option<&Path> {
        input.parent().and_then(|p| if p.as_os_str().is_empty() { None } else { Some(p) })
    }

    /// Print a diagnostic showing the effective project root.
    pub fn print_root_diagnostic(&self, input: &Path) {
        if crate::cli::is_quiet() { return; }
        let input_dir = input.parent().unwrap_or(Path::new("."));
        let root = if self.project_root.as_os_str().is_empty() { Path::new(".") } else { &self.project_root };
        let idir = if input_dir.as_os_str().is_empty() { Path::new(".") } else { input_dir };
        if idir != root {
            eprintln!("  root: {}  (code chunks run from {})", root.display(), idir.display());
        } else {
            eprintln!("  root: {}", root.display());
        }
    }
}

// ---------------------------------------------------------------------------
// Path resolution
// ---------------------------------------------------------------------------

/// Build a path under the project `_calepin/` directory.
/// Does not check existence -- use `resolve_path` for that.
///
/// Example: `calepin_dir(root, &["templates", "html"])` -> `{root}/_calepin/templates/html`
pub fn calepin_dir(project_root: &Path, segments: &[&str]) -> PathBuf {
    let mut p = project_root.join("_calepin");
    for s in segments {
        p = p.join(s);
    }
    p
}

/// `{root}/_calepin/partials`
pub fn partials_dir(project_root: &Path) -> PathBuf {
    calepin_dir(project_root, &["partials"])
}

/// `{root}/_calepin/assets`
pub fn assets_dir(project_root: &Path) -> PathBuf {
    calepin_dir(project_root, &["assets"])
}

// ---------------------------------------------------------------------------
// Template, snippet, and plugin resolution
// ---------------------------------------------------------------------------

/// Map a base name to its file extension for template/component lookup.
/// Derives the mapping from the built-in _calepin.toml.
pub fn engine_to_ext(base: &str) -> &str {
    let target = crate::project::builtin_metadata().targets.get(base);
    target
        .and_then(|t| t.extension.as_deref())
        .unwrap_or(base)
}

/// Resolve a partial (element or page) under `_calepin/partials/`.
///
/// Lookup order (first match wins):
///   1. `_calepin/partials/{target}/{name}.{ext}` (target-specific)
///   2. `_calepin/partials/{base}/{name}.{ext}` (base-specific, when target != base)
///   3. `_calepin/partials/common/{name}.jinja` (format-agnostic)
///   4. (caller falls back to built-in)
pub fn resolve_partial(name: &str, base: &str) -> Option<PathBuf> {
    let ext = engine_to_ext(base);
    let base_specific = format!("{}.{}", name, ext);
    let generic = format!("{}.jinja", name);

    let root = get_project_root();
    let tpl = partials_dir(&root);
    let active_target = get_active_target();

    // 1. User target-specific (e.g., _calepin/templates/book/)
    if let Some(ref target) = active_target {
        if target != base {
            let p = tpl.join(target).join(&base_specific);
            if p.exists() { return Some(p); }
        }
    }

    // 2. User engine-specific (e.g., _calepin/templates/latex/)
    let p = tpl.join(base).join(&base_specific);
    if p.exists() { return Some(p); }

    // 3. User format-agnostic (e.g., _calepin/templates/common/)
    let p = tpl.join("common").join(&generic);
    if p.exists() { return Some(p); }

    None
}

/// Resolve a snippet file under `_calepin/snippets/`.
///
/// Lookup order (first match wins):
///   1. `_calepin/snippets/{target}/{name}.{ext}` (target-specific)
///   2. `_calepin/snippets/{base}/{name}.{ext}` (base-specific, when target != base)
///   3. `_calepin/snippets/common/{name}.jinja` (format-agnostic)
pub fn resolve_snippet(name: &str, base: &str) -> Option<PathBuf> {
    let ext = engine_to_ext(base);
    let specific = format!("{}.{}", name, ext);
    let generic = format!("{}.jinja", name);

    let root = get_project_root();
    let snip = calepin_dir(&root, &["snippets"]); // no dedicated helper -- snippets are rare
    let active_target = get_active_target();

    // 1. User target-specific
    if let Some(ref target) = active_target {
        if target != base {
            let p = snip.join(target).join(&specific);
            if p.exists() { return Some(p); }
        }
    }

    // 2. User engine-specific
    let p = snip.join(base).join(&specific);
    if p.exists() { return Some(p); }

    // 3. User format-agnostic
    let p = snip.join("common").join(&generic);
    if p.exists() { return Some(p); }

    None
}

/// Resolve a module directory by name.
/// Checks `{project_root}/_calepin/modules/{name}/plugin.toml`.
pub fn resolve_module_dir(name: &str, project_root: &Path) -> Option<PathBuf> {
    let local = calepin_dir(project_root, &["modules", name]);
    if local.join("module.toml").exists() {
        return Some(local);
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
        let resolved = ctx.project_root.join(bib);
        if !resolved.exists() {
            errors.push(format!(
                "  bibliography: {}\n    -> not found: {}",
                bib,
                resolved.display()
            ));
        }
    }

    // CSL file (only if explicitly specified and not a built-in archive name)
    if let Some(ref csl) = meta.csl {
        use hayagriva::archive::ArchivedStyle;
        if ArchivedStyle::by_name(csl).is_none() {
            let resolved = ctx.project_root.join(csl);
            if !resolved.exists() {
                errors.push(format!(
                    "  csl: {}\n    -> not found: {}",
                    csl,
                    resolved.display()
                ));
            }
        }
    }

    // Plugins
    for plugin in &meta.plugins {
        if is_builtin_plugin(plugin) {
            continue;
        }
        let local_dir = ctx.project_root.join("_calepin/modules").join(plugin);
        let local_path = local_dir.join("module.toml");
        if !local_path.exists() {
            errors.push(format!(
                "  calepin.plugins: {}\n    -> not found: {}",
                plugin,
                local_path.display()
            ));
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
            project_root: PathBuf::from("/nonexistent/dir"),
            output_dir: PathBuf::from("/nonexistent/dir"),
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
