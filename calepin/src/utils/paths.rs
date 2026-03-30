//! Centralized path resolution, validation, and context.
//!
//! All input paths resolve relative to the project directory (the directory
//! containing `.qmd` files). The root sidecar (`{stem}_calepin/`) holds
//! project-wide config, partials, extensions, modules, and assets.
//! For document renders without a sidecar config, the project directory
//! is the parent directory of the `.qmd` file.
//! The output directory is where finished files are written; no inputs
//! resolve from it.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::config::Metadata;

// ---------------------------------------------------------------------------
// ProjectKind: unified document/collection discovery
// ---------------------------------------------------------------------------

/// The kind of project a path resolves to.
#[derive(Debug, Clone)]
pub enum ProjectKind {
    /// Single `.qmd` document with its sidecar directory.
    Document {
        qmd: PathBuf,
        sidecar: PathBuf,
    },
    /// Collection (website/book) with a root sidecar.
    Collection {
        /// Directory containing `.qmd` files.
        project_dir: PathBuf,
        /// Path to the root sidecar's `config.toml`.
        config: PathBuf,
        /// Root sidecar directory (e.g., `index_calepin/`).
        root_sidecar: PathBuf,
    },
}

impl ProjectKind {
    /// Discover the project kind from a `.qmd` file or directory.
    ///
    /// Detection order:
    /// 1. `{stem}_calepin/config.toml` with a collection target -> `Collection`
    /// 2. Legacy `_calepin/config.toml` with `[[contents]]` -> `Collection` (deprecated)
    /// 3. Otherwise -> `Document`
    pub fn discover(path: &Path) -> Result<Self> {
        let path = if path.is_relative() {
            normalize_path(&std::env::current_dir()
                .unwrap_or_default()
                .join(path))
        } else {
            normalize_path(path)
        };

        // Directory: look for index.qmd
        if path.is_dir() {
            let index = path.join("index.qmd");
            if index.exists() {
                return Self::discover(&index);
            }
            bail!(
                "No index.qmd found in {}. Create one or specify a .qmd file.",
                path.display()
            );
        }

        // Must be a .qmd file
        if path.extension().and_then(|e| e.to_str()) != Some("qmd") {
            bail!("Expected a .qmd file, got: {}", path.display());
        }
        if !path.exists() {
            bail!("File not found: {}", path.display());
        }

        let parent = path.parent().unwrap_or(Path::new("."));
        let stem = path.file_stem().unwrap().to_string_lossy();
        let sidecar = parent.join(format!("{}_calepin", stem));

        // 1. New convention: {stem}_calepin/config.toml with a collection target
        let sidecar_config = sidecar.join("config.toml");
        if sidecar_config.exists() {
            if let Some(target_name) = read_target_from_config(&sidecar_config) {
                if crate::config::extension::is_collection_target(&target_name) {
                    return Ok(ProjectKind::Collection {
                        project_dir: parent.to_path_buf(),
                        config: sidecar_config,
                        root_sidecar: sidecar,
                    });
                }
            }
        }

        // 2. Legacy: _calepin/config.toml with [[contents]]
        let legacy_config = calepin_dir(parent, &[]).join("config.toml");
        if legacy_config.exists() {
            if let Ok(text) = std::fs::read_to_string(&legacy_config) {
                if text.contains("[[contents]]") {
                    eprintln!(
                        "\x1b[33mWarning:\x1b[0m _calepin/config.toml is deprecated. \
                         Move config to {}_calepin/config.toml with `target = \"website\"` \
                         (or the appropriate collection target).",
                        stem
                    );
                    return Ok(ProjectKind::Collection {
                        project_dir: parent.to_path_buf(),
                        config: legacy_config,
                        root_sidecar: calepin_dir(parent, &[]),
                    });
                }
            }
        }

        // Single document
        Ok(ProjectKind::Document { qmd: path, sidecar })
    }

    /// The root sidecar directory.
    pub fn calepin_dir(&self) -> PathBuf {
        match self {
            ProjectKind::Document { sidecar, .. } => sidecar.clone(),
            ProjectKind::Collection { root_sidecar, .. } => root_sidecar.clone(),
        }
    }

}

/// Read the `target` field from a TOML config file without full parsing.
fn read_target_from_config(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let table: toml::Value = toml::from_str(&text).ok()?;
    table.get("target")
        .or_else(|| table.get("format"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// Active target name (thread-local)
// ---------------------------------------------------------------------------

thread_local! {
    static ACTIVE_TARGET: RefCell<Option<String>> = RefCell::new(None);
    /// Target inheritance chain (child-first), e.g. ["minimal", "website", "html"].
    /// Used by partial resolution to walk the chain instead of checking target/base.
    static ACTIVE_INHERITANCE_CHAIN: RefCell<Vec<String>> = RefCell::new(Vec::new());
    static PROJECT_DIR: RefCell<Option<PathBuf>> = RefCell::new(None);
    static ROOT_SIDECAR: RefCell<Option<PathBuf>> = RefCell::new(None);
    static PAGE_SIDECAR: RefCell<Option<PathBuf>> = RefCell::new(None);
    /// Extension partial directories to check, in inheritance order (child first).
    static EXTENSION_PARTIAL_DIRS: RefCell<Vec<PathBuf>> = RefCell::new(Vec::new());
    /// Side-loaded extension names (from calepin.extensions config).
    static SIDELOADED_EXTENSIONS: RefCell<Vec<String>> = RefCell::new(Vec::new());
}

/// Set the active target name for template resolution.
pub fn set_active_target(target: Option<&str>) {
    ACTIVE_TARGET.with(|t| {
        *t.borrow_mut() = target.map(|s| s.to_string());
    });
}

pub fn get_active_target() -> Option<String> {
    ACTIVE_TARGET.with(|t| t.borrow().clone())
}

/// Set the target inheritance chain for partial resolution.
/// Chain is child-first, e.g. ["minimal", "website", "html"].
pub fn set_active_inheritance_chain(chain: Vec<String>) {
    ACTIVE_INHERITANCE_CHAIN.with(|c| {
        *c.borrow_mut() = chain;
    });
}

pub fn get_active_inheritance_chain() -> Vec<String> {
    ACTIVE_INHERITANCE_CHAIN.with(|c| c.borrow().clone())
}

/// Set both the active target and its inheritance chain.
/// Convenience wrapper that keeps the two in sync.
pub fn set_active_target_with_chain(target: Option<&str>, chain: Vec<String>) {
    set_active_target(target);
    set_active_inheritance_chain(chain);
}

/// Set the project directory (the directory containing .qmd files).
pub fn set_project_dir(root: Option<&Path>) {
    PROJECT_DIR.with(|r| {
        *r.borrow_mut() = root.map(|p| p.to_path_buf());
    });
}

pub fn get_project_dir() -> PathBuf {
    PROJECT_DIR.with(|r| {
        r.borrow().clone().unwrap_or_else(|| PathBuf::from("."))
    })
}

/// Set the root sidecar directory (project-wide config, partials, extensions).
/// For collections, this is the entry point's sidecar (e.g., `index_calepin/`).
/// For single documents, this equals the page sidecar.
pub fn set_root_sidecar(root: Option<&Path>) {
    ROOT_SIDECAR.with(|r| {
        *r.borrow_mut() = root.map(|p| p.to_path_buf());
    });
}

pub fn get_root_sidecar() -> Option<PathBuf> {
    ROOT_SIDECAR.with(|r| r.borrow().clone())
}

/// Set the per-page sidecar directory for partial/module resolution.
pub fn set_page_sidecar(root: Option<&Path>) {
    PAGE_SIDECAR.with(|r| {
        *r.borrow_mut() = root.map(|p| p.to_path_buf());
    });
}

pub fn get_page_sidecar() -> Option<PathBuf> {
    PAGE_SIDECAR.with(|r| r.borrow().clone())
}

/// Set extension partial directories (child-first order).
/// Called once during target resolution to establish the extension chain.
pub fn set_extension_partial_dirs(dirs: Vec<PathBuf>) {
    EXTENSION_PARTIAL_DIRS.with(|d| {
        *d.borrow_mut() = dirs;
    });
}

pub fn get_extension_partial_dirs() -> Vec<PathBuf> {
    EXTENSION_PARTIAL_DIRS.with(|d| d.borrow().clone())
}

/// Set side-loaded extension names (from `[calepin] extensions = [...]`).
pub fn set_sideloaded_extensions(names: Vec<String>) {
    SIDELOADED_EXTENSIONS.with(|e| {
        *e.borrow_mut() = names;
    });
}

pub fn get_sideloaded_extensions() -> Vec<String> {
    SIDELOADED_EXTENSIONS.with(|e| e.borrow().clone())
}

/// Given the path to a sidecar config file (e.g. `<root>/index_calepin/config.toml`
/// or legacy `<root>/_calepin/config.toml`), return the project root directory.
/// The config's parent is a sidecar directory ending in `_calepin`; the project
/// root is its grandparent.
pub fn resolve_project_root(config_path: &Path, fallback: &Path) -> PathBuf {
    if let Some(parent) = config_path.parent() {
        let is_sidecar = parent.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.ends_with("_calepin"))
            .unwrap_or(false);
        if is_sidecar {
            let root = parent.parent().unwrap_or(fallback);
            if root.as_os_str().is_empty() {
                return fallback.to_path_buf();
            }
            return root.to_path_buf();
        }
        if parent.as_os_str().is_empty() {
            return fallback.to_path_buf();
        }
        return parent.to_path_buf();
    }
    fallback.to_path_buf()
}

/// Resolve the sidecar directory for an input file.
///
/// Sidecars always live next to their `.qmd` file: `{parent}/{stem}_calepin/`.
///
/// If the directory does not exist, creates it. In document mode (no project
/// root set), a default `config.toml` and built-in partials are scaffolded;
/// in collection mode, only the directory is created.
pub fn resolve_sidecar_dir(input: &Path) -> Option<PathBuf> {
    let stem = input.file_stem()?.to_string_lossy();
    let sidecar_name = format!("{}_calepin", stem);
    let dir = input.parent()?.join(&sidecar_name);
    let project_dir = PROJECT_DIR.with(|r| r.borrow().clone());

    if !dir.is_dir() {
        if project_dir.is_some() {
            // Collection mode: just create the directory
            std::fs::create_dir_all(&dir).ok();
        } else {
            // Document mode: full scaffold with config.toml and partials
            create_sidecar(&dir);
        }
    }
    Some(dir)
}

/// Create a sidecar directory with a default `config.toml` and all built-in partials.
pub fn create_sidecar(dir: &Path) {
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("Warning: could not create sidecar directory {}: {}", dir.display(), e);
        return;
    }
    let config = format!("{}\n{}", crate::config::SHARED_TOML, crate::config::DOCUMENT_TOML);
    if let Err(e) = std::fs::write(dir.join("config.toml"), &config) {
        eprintln!("Warning: could not write sidecar config: {}", e);
    }
    // Write all built-in partials so users can customize them.
    write_builtin_partials(&dir.join("partials"));
}

/// Write all built-in partials into the given directory, preserving subdirectory structure.
pub fn write_builtin_partials(dest: &Path) {
    use crate::render::elements::BUILTIN_PARTIALS;
    write_embedded_dir(&BUILTIN_PARTIALS, dest);
}

/// Write an embedded `include_dir::Dir` to disk, preserving subdirectory structure.
/// Silently skips files that fail to write.
pub fn write_embedded_dir(dir: &include_dir::Dir<'static>, dest: &Path) {
    for file in dir.files() {
        let target = dest.join(file.path());
        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&target, file.contents());
    }
    for subdir in dir.dirs() {
        write_embedded_dir(subdir, dest);
    }
}

// ---------------------------------------------------------------------------
// PathContext
// ---------------------------------------------------------------------------

/// Path context carried through the render pipeline.
///
/// All input paths resolve relative to `project_root` (the directory
/// containing `_calepin/config.toml`, or the `.qmd` parent in document mode).
/// The output directory is only for writing; no input files resolve from it.
#[derive(Debug, Clone)]
pub struct PathContext {
    /// Project root: directory containing `_calepin/config.toml`, or `.qmd` parent
    /// in document mode. All input paths resolve from here.
    pub project_root: PathBuf,
    /// Where output files are written. No input files resolve from here.
    pub output_dir: PathBuf,
}

impl PathContext {
    /// Construct a PathContext, with optional project root override.
    /// In document mode (no override), project_root = input's parent directory.
    pub fn new(input: &Path, output_path: &Path, project_root_override: Option<&Path>) -> Self {
        let project_root = project_root_override
            .map(|r| r.to_path_buf())
            .unwrap_or_else(|| input.parent().unwrap_or(Path::new(".")).to_path_buf());
        let output_dir = output_path.parent().unwrap_or(Path::new(".")).to_path_buf();
        Self { project_root, output_dir }
    }

    /// Resolve a subdirectory, checking the sidecar first then falling back
    /// to `_calepin/{subdir}/{stem}/` under `fallback_root`.
    fn sidecar_or_project_subdir(subdir: &str, stem: &str, fallback_root: &Path) -> PathBuf {
        if let Some(sidecar) = get_page_sidecar() {
            sidecar.join(subdir)
        } else {
            calepin_dir(fallback_root, &[subdir, stem])
        }
    }

    /// Resolve the figure output directory for a given document stem.
    pub fn figures_dir(&self, stem: &str) -> PathBuf {
        Self::sidecar_or_project_subdir("files", stem, &self.output_dir)
    }

    /// Resolve the cache directory for a given document stem.
    pub fn cache_dir(&self, stem: &str) -> PathBuf {
        Self::sidecar_or_project_subdir("cache", stem, &self.project_root)
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
}

// ---------------------------------------------------------------------------
// Path resolution
// ---------------------------------------------------------------------------

/// Remove `.` and resolve `..` components without touching the filesystem.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => { out.pop(); }
            c => out.push(c),
        }
    }
    out
}

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

/// Default output directory name for collection builds.
pub const DEFAULT_OUTPUT_DIR: &str = "_calepin_output";

/// Resolve the output directory for a collection build.
/// Uses the config `output` field if set, otherwise `DEFAULT_OUTPUT_DIR`.
/// The result is always absolute (joined to `project_root`).
pub fn output_dir(project_root: &Path, config_output: Option<&str>) -> PathBuf {
    let name = config_output.unwrap_or(DEFAULT_OUTPUT_DIR);
    let p = PathBuf::from(name);
    if p.is_absolute() {
        p
    } else {
        project_root.join(name)
    }
}

/// Partials directory: root sidecar's `partials/`, or legacy `_calepin/partials`.
pub fn partials_dir(project_root: &Path) -> PathBuf {
    if let Some(sidecar) = get_root_sidecar() {
        sidecar.join("partials")
    } else {
        project_root.join("_calepin/partials")
    }
}

/// Assets directory: root sidecar's `assets/`, or legacy `_calepin/assets`.
pub fn assets_dir(project_root: &Path) -> PathBuf {
    if let Some(sidecar) = get_root_sidecar() {
        sidecar.join("assets")
    } else {
        project_root.join("_calepin/assets")
    }
}

/// Extensions directory: root sidecar's `extensions/`, or legacy `_calepin/extensions`.
pub fn extensions_dir(project_root: &Path) -> PathBuf {
    if let Some(sidecar) = get_root_sidecar() {
        sidecar.join("extensions")
    } else {
        project_root.join("_calepin").join("extensions")
    }
}

// ---------------------------------------------------------------------------
// Template, partial, and plugin resolution
// ---------------------------------------------------------------------------

/// Map a base name to its file extension for template/component lookup.
/// Derives the mapping from the built-in _calepin/config.toml.
pub fn resolve_extension(base: &str) -> &str {
    let target = crate::config::builtin_metadata().targets.get(base);
    target
        .and_then(|t| t.extension.as_deref())
        .unwrap_or(base)
}

/// Check a partials directory for a matching partial file.
/// Walks the inheritance chain, then falls back to `common/`.
fn check_partials_dir(
    tpl: &Path,
    chain: &[String],
    specific: &str,
    generic: &str,
) -> Option<PathBuf> {
    for target in chain {
        let p = tpl.join(target).join(specific);
        if p.exists() { return Some(p); }
    }
    let p = tpl.join("common").join(generic);
    if p.exists() { return Some(p); }
    None
}

/// Resolve a partial (element or page).
///
/// Lookup order (first match wins), walking the inheritance chain at each level:
///   1. Sidecar partials: `{stem}_calepin/partials/{chain...}/` then `common/`
///   2. Project partials: `_calepin/partials/{chain...}/` then `common/`
///   3. Extension partials (child-first inheritance chain)
///   4. (caller falls back to built-in)
///
/// The `writer` parameter determines the file extension (html, tex, typ, md).
pub fn resolve_partial(name: &str, writer: &str) -> Option<PathBuf> {
    let ext = resolve_extension(writer);
    let specific = format!("{}.{}", name, ext);
    let generic = format!("{}.jinja", name);
    let chain = get_active_inheritance_chain();
    // Fall back to writer as a single-element chain when no target is set.
    let fallback;
    let chain = if chain.is_empty() {
        fallback = vec![writer.to_string()];
        &fallback
    } else {
        &chain
    };

    // Check sidecar then project-level partials
    let mut dirs = Vec::with_capacity(2);
    if let Some(sidecar) = get_page_sidecar() {
        dirs.push(sidecar.join("partials"));
    }
    dirs.push(partials_dir(&get_project_dir()));

    for tpl in &dirs {
        if let Some(p) = check_partials_dir(tpl, chain, &specific, &generic) {
            return Some(p);
        }
    }

    // Check extension partials (child-first order)
    for ext_dir in get_extension_partial_dirs() {
        if let Some(p) = check_partials_dir(&ext_dir, chain, &specific, &generic) {
            return Some(p);
        }
    }

    None
}

/// Resolve a module directory by name.
/// Checks sidecar first, then project-level `_calepin/modules/{name}/`.
pub fn resolve_module_dir(name: &str, project_root: &Path) -> Option<PathBuf> {
    let candidates = [
        get_page_sidecar().map(|s| s.join("modules").join(name)),
        Some(calepin_dir(project_root, &["modules", name])),
    ];
    candidates.into_iter()
        .flatten()
        .find(|dir| dir.join("module.toml").exists())
}

// ---------------------------------------------------------------------------
// Filesystem utilities
// ---------------------------------------------------------------------------

/// Copy a directory tree recursively, creating parent directories as needed.
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    use walkdir::WalkDir;
    for entry in WalkDir::new(src) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src)?;
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
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

    // Plugins (check sidecar first, then project-level)
    for plugin in &meta.plugins {
        if is_builtin_plugin(plugin) {
            continue;
        }
        let found = resolve_module_dir(plugin, &ctx.project_root).is_some();
        if !found {
            let local_path = calepin_dir(&ctx.project_root, &["modules", plugin]).join("module.toml");
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
    crate::registry::builtin_module_names().iter().any(|n| n == name)
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
