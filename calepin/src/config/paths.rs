//! Centralized path resolution, validation, and context.
//!
//! All input paths resolve relative to the project root (the directory
//! containing `_calepin/config.toml`). For document renders without a project
//! config, the project root is the parent directory of the `.qmd` file.
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
    /// Collection (website/book) with a project `_calepin/` directory.
    Collection {
        root: PathBuf,
        config: PathBuf,
    },
}

impl ProjectKind {
    /// Discover the project kind from a path.
    ///
    /// Accepted inputs:
    /// - A `.qmd` file -> `Document`
    /// - A sidecar `{stem}_calepin/config.toml` (with a sibling `{stem}.qmd`) -> `Document`
    /// - A directory containing `_calepin/config.toml` -> `Collection`
    /// - A bare `_calepin/config.toml` path -> `Collection`
    pub fn discover(path: &Path) -> Result<Self> {
        let path = if path.is_relative() {
            std::env::current_dir()
                .unwrap_or_default()
                .join(path)
        } else {
            path.to_path_buf()
        };

        // Case 1: .qmd file
        if path.extension().and_then(|e| e.to_str()) == Some("qmd") {
            if !path.exists() {
                bail!("File not found: {}", path.display());
            }
            let stem = path.file_stem().unwrap().to_string_lossy();
            let parent = path.parent().unwrap_or(Path::new("."));
            let sidecar = parent.join(format!("{}_calepin", stem));
            return Ok(ProjectKind::Document { qmd: path, sidecar });
        }

        // Case 2: config.toml file -- could be sidecar or collection
        if path.file_name().and_then(|n| n.to_str()) == Some("config.toml") {
            if let Some(parent) = path.parent() {
                let parent_name = parent.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                // Collection: parent is _calepin
                if parent_name == "_calepin" {
                    let root = parent.parent().unwrap_or(Path::new("."));
                    return Ok(ProjectKind::Collection {
                        root: root.to_path_buf(),
                        config: path.to_path_buf(),
                    });
                }

                // Sidecar: parent is {stem}_calepin
                if let Some(stem) = parent_name.strip_suffix("_calepin") {
                    let grandparent = parent.parent().unwrap_or(Path::new("."));
                    let qmd = grandparent.join(format!("{}.qmd", stem));
                    if qmd.exists() {
                        return Ok(ProjectKind::Document {
                            qmd,
                            sidecar: parent.to_path_buf(),
                        });
                    }
                    bail!(
                        "Sidecar config found at {} but no matching {}.qmd",
                        path.display(), stem
                    );
                }
            }
            bail!("Unexpected config.toml location: {}", path.display());
        }

        // Case 3: directory
        if path.is_dir() {
            let config = path.join("_calepin").join("config.toml");
            if config.exists() {
                return Ok(ProjectKind::Collection {
                    root: path.to_path_buf(),
                    config,
                });
            }
            bail!(
                "No calepin project found at {}. Run `calepin new` first or specify a .qmd file.",
                path.display()
            );
        }

        bail!("Cannot determine project kind for: {}", path.display());
    }

    /// The directory where `_calepin/` (collection) or sidecar lives.
    pub fn calepin_dir(&self) -> PathBuf {
        match self {
            ProjectKind::Document { sidecar, .. } => sidecar.clone(),
            ProjectKind::Collection { root, .. } => root.join("_calepin"),
        }
    }

    /// The directory where partials should be written/read.
    #[allow(dead_code)]
    pub fn partials_dir(&self) -> PathBuf {
        self.calepin_dir().join("partials")
    }

    /// The directory where assets should be written/read.
    #[allow(dead_code)]
    pub fn assets_dir(&self) -> PathBuf {
        self.calepin_dir().join("assets")
    }
}

// ---------------------------------------------------------------------------
// Active target name (thread-local)
// ---------------------------------------------------------------------------

thread_local! {
    static ACTIVE_TARGET: RefCell<Option<String>> = RefCell::new(None);
    static PROJECT_ROOT: RefCell<Option<PathBuf>> = RefCell::new(None);
    static SIDECAR_ROOT: RefCell<Option<PathBuf>> = RefCell::new(None);
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

/// Set the sidecar root for per-document partial/module resolution.
pub fn set_sidecar_root(root: Option<&Path>) {
    SIDECAR_ROOT.with(|r| {
        *r.borrow_mut() = root.map(|p| p.to_path_buf());
    });
}

pub fn get_sidecar_root() -> Option<PathBuf> {
    SIDECAR_ROOT.with(|r| r.borrow().clone())
}

/// Given the path to a config file (e.g. `<root>/_calepin/config.toml`),
/// return the project root directory. If the config lives inside `_calepin/`,
/// the root is its grandparent; otherwise fall back to the config's parent.
pub fn resolve_project_root(config_path: &Path, fallback: &Path) -> PathBuf {
    if let Some(parent) = config_path.parent() {
        if parent.file_name().map(|n| n == "_calepin").unwrap_or(false) {
            let root = parent.parent().unwrap_or(fallback);
            // parent.parent() of a relative "_calepin" is "" -- use fallback
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
/// In collection mode (project root set), sidecars live inside `_calepin/`,
/// preserving the input's subdirectory structure to avoid name collisions:
///   `{project_root}/_calepin/{relative_parent}/{stem}_calepin/`
///
/// For example, `pages/top_link_1.qmd` resolves to
///   `_calepin/pages/top_link_1_calepin/`
/// while `index.qmd` at the root resolves to
///   `_calepin/index_calepin/`
///
/// In document mode (no project root), sidecars live next to the file:
///   `{parent}/{stem}_calepin/`
///
/// If the directory does not exist, creates it. In document mode, a default
/// `config.toml` and built-in partials are scaffolded; in collection mode,
/// only the directory is created (the root `_calepin/config.toml` suffices).
pub fn resolve_sidecar_dir(input: &Path) -> Option<PathBuf> {
    let stem = input.file_stem()?.to_string_lossy();
    let sidecar_name = format!("{}_calepin", stem);

    let dir = if let Some(root) = get_project_root_if_set() {
        // Preserve subdirectory structure relative to project root
        let abs_input = if input.is_relative() {
            std::env::current_dir().unwrap_or_default().join(input)
        } else {
            input.to_path_buf()
        };
        let relative = abs_input.strip_prefix(&root).unwrap_or(&abs_input);
        let relative_parent = relative.parent().unwrap_or(Path::new(""));
        root.join("_calepin").join(relative_parent).join(&sidecar_name)
    } else {
        input.parent()?.join(&sidecar_name)
    };

    if !dir.is_dir() {
        if get_project_root_if_set().is_some() {
            // Collection mode: just create the directory, no config.toml or partials
            std::fs::create_dir_all(&dir).ok();
        } else {
            // Document mode: full scaffold with config.toml and partials
            create_sidecar(&dir);
        }
    }
    Some(dir)
}

/// Returns the project root only if explicitly set (collection mode).
fn get_project_root_if_set() -> Option<PathBuf> {
    PROJECT_ROOT.with(|r| r.borrow().clone())
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
/// When `strip_prefix` is Some, paths are relativized by stripping that prefix.
/// Silently skips files that fail to write.
pub fn write_embedded_dir(dir: &include_dir::Dir<'static>, dest: &Path) {
    write_embedded_dir_impl(dir, dest, None);
}

fn write_embedded_dir_impl(dir: &include_dir::Dir<'static>, dest: &Path, strip_prefix: Option<&Path>) {
    for file in dir.files() {
        let rel = strip_prefix
            .and_then(|p| file.path().strip_prefix(p).ok())
            .unwrap_or(file.path());
        let target = dest.join(rel);
        if let Some(parent) = target.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&target, file.contents());
    }
    for subdir in dir.dirs() {
        write_embedded_dir_impl(subdir, dest, strip_prefix);
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
    /// If a document sidecar exists, files go directly in `{stem}_calepin/files/`.
    /// Otherwise, they go in `_calepin/files/{stem}/` (namespaced by stem).
    pub fn figures_dir(&self, stem: &str) -> PathBuf {
        if let Some(sidecar) = get_sidecar_root() {
            sidecar.join("files")
        } else {
            calepin_dir(&self.output_dir, &["files", stem])
        }
    }

    /// Resolve the cache directory for a given document stem.
    /// If a document sidecar exists, cache goes directly in `{stem}_calepin/cache/`.
    /// Otherwise, it goes in `_calepin/cache/{stem}/` (namespaced by stem).
    pub fn cache_dir(&self, stem: &str) -> PathBuf {
        if let Some(sidecar) = get_sidecar_root() {
            sidecar.join("cache")
        } else {
            calepin_dir(&self.project_root, &["cache", stem])
        }
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
    /// Only prints when the root differs from the input's parent directory
    /// (i.e., in collection builds or when a project root override is active).
    pub fn print_root_diagnostic(&self, input: &Path) {
        if crate::cli::is_quiet() { return; }
        let input_dir = input.parent().unwrap_or(Path::new("."));
        let root = if self.project_root.as_os_str().is_empty() { Path::new(".") } else { &self.project_root };
        let idir = if input_dir.as_os_str().is_empty() { Path::new(".") } else { input_dir };
        let _ = (root, idir); // suppress verbose diagnostic
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
/// Tries target-specific, engine-specific, then format-agnostic paths.
fn check_partials_dir(
    tpl: &Path,
    base: &str,
    base_specific: &str,
    generic: &str,
    active_target: &Option<String>,
) -> Option<PathBuf> {
    if let Some(ref target) = active_target {
        if target != base {
            let p = tpl.join(target).join(base_specific);
            if p.exists() { return Some(p); }
        }
    }
    let p = tpl.join(base).join(base_specific);
    if p.exists() { return Some(p); }
    let p = tpl.join("common").join(generic);
    if p.exists() { return Some(p); }
    None
}

/// Resolve a partial (element or page).
///
/// Lookup order (first match wins):
///   1. `{stem}_calepin/partials/{target}/{name}.{ext}` (sidecar, target-specific)
///   2. `{stem}_calepin/partials/{base}/{name}.{ext}` (sidecar, engine-specific)
///   3. `{stem}_calepin/partials/common/{name}.jinja` (sidecar, format-agnostic)
///   4. `_calepin/partials/{target}/{name}.{ext}` (project, target-specific)
///   5. `_calepin/partials/{base}/{name}.{ext}` (project, engine-specific)
///   6. `_calepin/partials/common/{name}.jinja` (project, format-agnostic)
///   7. (caller falls back to built-in)
pub fn resolve_partial(name: &str, base: &str) -> Option<PathBuf> {
    let ext = resolve_extension(base);
    let base_specific = format!("{}.{}", name, ext);
    let generic = format!("{}.jinja", name);
    let active_target = get_active_target();

    // Check sidecar partials first
    if let Some(sidecar) = get_sidecar_root() {
        let tpl = sidecar.join("partials");
        if let Some(p) = check_partials_dir(&tpl, base, &base_specific, &generic, &active_target) {
            return Some(p);
        }
    }

    // Then project-level partials
    let root = get_project_root();
    let tpl = partials_dir(&root);
    check_partials_dir(&tpl, base, &base_specific, &generic, &active_target)
}

/// Resolve a module directory by name.
/// Checks sidecar first (`{stem}_calepin/modules/{name}/module.toml`),
/// then project-level (`_calepin/modules/{name}/module.toml`).
pub fn resolve_module_dir(name: &str, project_root: &Path) -> Option<PathBuf> {
    // 1. Document sidecar
    if let Some(sidecar) = get_sidecar_root() {
        let local = sidecar.join("modules").join(name);
        if local.join("module.toml").exists() {
            return Some(local);
        }
    }

    // 2. Project-level
    let local = calepin_dir(project_root, &["modules", name]);
    if local.join("module.toml").exists() {
        return Some(local);
    }

    None
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
            let local_path = ctx.project_root.join("_calepin/modules").join(plugin).join("module.toml");
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
