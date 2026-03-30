//! Centralized path resolution, validation, and context.
//!
//! All input paths resolve relative to the project directory (the directory
//! containing `.qmd` files). The root sidecar (`{stem}_calepin/`) holds
//! project-wide config, templates, extensions, modules, and assets.
//! For document renders without a sidecar config, the project directory
//! is the parent directory of the `.qmd` file.
//! The output directory is where finished files are written; no inputs
//! resolve from it.

use std::cell::RefCell;
use std::collections::HashMap;
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
    /// Detection: `{stem}_calepin/config.toml` with a collection target -> `Collection`,
    /// otherwise -> `Document`.
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

        // {stem}_calepin/config.toml with a collection target -> Collection
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
    /// Used by template resolution to walk the chain instead of checking target/base.
    static ACTIVE_INHERITANCE_CHAIN: RefCell<Vec<String>> = RefCell::new(Vec::new());
    static PROJECT_DIR: RefCell<Option<PathBuf>> = RefCell::new(None);
    static ROOT_SIDECAR: RefCell<Option<PathBuf>> = RefCell::new(None);
    static PAGE_SIDECAR: RefCell<Option<PathBuf>> = RefCell::new(None);
    /// Extension template directories to check, in inheritance order (child first).
    static EXTENSION_TEMPLATE_DIRS: RefCell<Vec<PathBuf>> = RefCell::new(Vec::new());
    /// Side-loaded extension names (from calepin.extensions config).
    static SIDELOADED_EXTENSIONS: RefCell<Vec<String>> = RefCell::new(Vec::new());
    /// Active template variant selections (from [tpl] config).
    static ACTIVE_TPL: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
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

/// Set the target inheritance chain for template resolution.
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

/// Set the root sidecar directory (project-wide config, templates, extensions).
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

/// Set the per-page sidecar directory for template/module resolution.
pub fn set_page_sidecar(root: Option<&Path>) {
    PAGE_SIDECAR.with(|r| {
        *r.borrow_mut() = root.map(|p| p.to_path_buf());
    });
}

pub fn get_page_sidecar() -> Option<PathBuf> {
    PAGE_SIDECAR.with(|r| r.borrow().clone())
}

/// Set extension template directories (child-first order).
/// Called once during target resolution to establish the extension chain.
pub fn set_extension_template_dirs(dirs: Vec<PathBuf>) {
    EXTENSION_TEMPLATE_DIRS.with(|d| {
        *d.borrow_mut() = dirs;
    });
}

pub fn get_extension_template_dirs() -> Vec<PathBuf> {
    EXTENSION_TEMPLATE_DIRS.with(|d| d.borrow().clone())
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

/// Set the active template variant selections (from `[tpl]` config).
pub fn set_active_tpl(tpl: HashMap<String, String>) {
    ACTIVE_TPL.with(|t| {
        *t.borrow_mut() = tpl;
    });
}

pub fn get_active_tpl() -> HashMap<String, String> {
    ACTIVE_TPL.with(|t| t.borrow().clone())
}

/// Given the path to a sidecar config file (e.g. `<root>/index_calepin/config.toml`),
/// return the project root directory. The config's parent is a sidecar directory
/// ending in `_calepin`; the project root is its grandparent.
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
/// root set), a default `config.toml` and built-in templates are scaffolded;
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
            // Document mode: full scaffold with config.toml and templates
            create_sidecar(&dir);
        }
    }
    Some(dir)
}

/// Create a sidecar directory with a default `config.toml`.
/// Templates are ejected later by `ensure_chain_templates` once the target is known.
pub fn create_sidecar(dir: &Path) {
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("Warning: could not create sidecar directory {}: {}", dir.display(), e);
        return;
    }
    let config = format!("{}\n{}", crate::config::SHARED_TOML, crate::config::DOCUMENT_TOML);
    if let Err(e) = std::fs::write(dir.join("config.toml"), &config) {
        eprintln!("Warning: could not write sidecar config: {}", e);
    }
}

/// Prepend an xxh3 hash marker to template content.
pub fn prepend_hash_marker(content: &str) -> String {
    use xxhash_rust::xxh3::xxh3_64;
    let hash = format!("{:016x}", xxh3_64(content.as_bytes()));
    format!("{{# calepin:xxh3:{} #}}\n{}", hash, content)
}

/// Ensure built-in templates for the active target exist on disk.
///
/// Creates a flat `templates/{target}/` directory by walking the inheritance
/// chain from parent to child: parent templates are written first, then child
/// templates overwrite them. Files already on disk are never overwritten, so
/// user edits are preserved across renders and upgrades.
pub fn ensure_chain_templates(dest: &Path) {
    use crate::render::elements::BUILTIN_TEMPLATES;
    let chain = get_active_inheritance_chain();
    if chain.is_empty() { return; }

    // Target name is the first element (child-first chain)
    let target_name = &chain[0];
    let target_dest = dest.join(target_name);

    // Walk child-first: child templates are written first, parent templates
    // fill in the gaps. Files already on disk are never overwritten.
    for chain_target in chain.iter() {
        if let Some(dir) = BUILTIN_TEMPLATES.get_dir(chain_target.as_str()) {
            flatten_dir_into(dir, chain_target, &target_dest);
        }
    }
}

/// Flatten a built-in directory into dest, skipping files that already exist.
fn flatten_dir_into(dir: &include_dir::Dir<'static>, prefix: &str, dest: &Path) {
    let prefix_path = std::path::Path::new(prefix);
    for file in dir.files() {
        let rel = file.path().strip_prefix(prefix_path).unwrap_or(file.path());
        let out = dest.join(rel);
        if out.exists() { continue; }
        if let Some(parent) = out.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Some(content) = file.contents_utf8() {
            let _ = std::fs::write(&out, prepend_hash_marker(content));
        } else {
            let _ = std::fs::write(&out, file.contents());
        }
    }
    for subdir in dir.dirs() {
        let sub_rel = subdir.path().strip_prefix(prefix_path).unwrap_or(subdir.path());
        flatten_dir_into(subdir, &subdir.path().display().to_string(), &dest.join(sub_rel));
    }
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
/// All input paths resolve relative to `project_root` (the `.qmd` parent
/// directory, or the collection root). The output directory is only for
/// writing; no input files resolve from it.
#[derive(Debug, Clone)]
pub struct PathContext {
    /// Project root: `.qmd` parent in document mode, or the collection root
    /// directory. All input paths resolve from here.
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

    /// Resolve a subdirectory under the page sidecar (`{stem}_calepin/{subdir}/`).
    /// Falls back to `{fallback_root}/{stem}_calepin/{subdir}/` if no sidecar is set.
    fn sidecar_or_project_subdir(subdir: &str, stem: &str, fallback_root: &Path) -> PathBuf {
        if let Some(sidecar) = get_page_sidecar() {
            sidecar.join(subdir)
        } else {
            fallback_root.join(format!("{}_calepin", stem)).join(subdir)
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

/// Templates directory: root sidecar's `templates/`.
pub fn templates_dir(project_root: &Path) -> PathBuf {
    get_root_sidecar()
        .unwrap_or_else(|| get_page_sidecar().unwrap_or_else(|| project_root.to_path_buf()))
        .join("templates")
}

/// Assets directory: root sidecar's `assets/`.
pub fn assets_dir(project_root: &Path) -> PathBuf {
    get_root_sidecar()
        .unwrap_or_else(|| get_page_sidecar().unwrap_or_else(|| project_root.to_path_buf()))
        .join("assets")
}

/// Extensions directory: root sidecar's `extensions/`.
pub fn extensions_dir(project_root: &Path) -> PathBuf {
    get_root_sidecar()
        .unwrap_or_else(|| get_page_sidecar().unwrap_or_else(|| project_root.to_path_buf()))
        .join("extensions")
}

// ---------------------------------------------------------------------------
// Template and plugin resolution
// ---------------------------------------------------------------------------

/// Map a base name to its file extension for template/component lookup.
/// Derives the mapping from the built-in target configuration.
pub fn resolve_extension(base: &str) -> &str {
    let target = crate::config::builtin_metadata().targets.get(base);
    target
        .and_then(|t| t.extension.as_deref())
        .unwrap_or(base)
}

/// Resolve a template (element or page).
///
/// Checks `{stem}_calepin/templates/{target}/{name}.{ext}`.
/// The `writer` parameter determines the file extension (html, tex, typ, md).
pub fn resolve_template(name: &str, writer: &str) -> Option<PathBuf> {
    let ext = resolve_extension(writer);
    let chain = get_active_inheritance_chain();
    let target = chain.first().map(|s| s.as_str()).unwrap_or(writer);
    let tpl_dir = templates_dir(&get_project_dir()).join(target);

    // Check variant first (if tpl has a mapping for this template name)
    let variant = ACTIVE_TPL.with(|t| t.borrow().get(name).cloned());
    if let Some(ref v) = variant {
        let variant_specific = format!("{}.{}.{}", name, v, ext);
        let variant_generic = format!("{}.{}.jinja", name, v);

        let p = tpl_dir.join(&variant_specific);
        if p.exists() { return Some(p); }
        let p = tpl_dir.join(&variant_generic);
        if p.exists() { return Some(p); }

        for ext_dir in get_extension_template_dirs() {
            let p = ext_dir.join(target).join(&variant_specific);
            if p.exists() { return Some(p); }
            let p = ext_dir.join(target).join(&variant_generic);
            if p.exists() { return Some(p); }
        }
    }

    // Base template (no variant)
    let specific = format!("{}.{}", name, ext);
    let generic = format!("{}.jinja", name);

    let p = tpl_dir.join(&specific);
    if p.exists() { return Some(p); }
    let p = tpl_dir.join(&generic);
    if p.exists() { return Some(p); }

    // Check extension templates (target name, then writer name)
    for ext_dir in get_extension_template_dirs() {
        let p = ext_dir.join(target).join(&specific);
        if p.exists() { return Some(p); }
        let p = ext_dir.join(target).join(&generic);
        if p.exists() { return Some(p); }
        // Also check writer subdirectory (e.g., html/) when target differs
        if target != writer {
            let p = ext_dir.join(writer).join(&specific);
            if p.exists() { return Some(p); }
            let p = ext_dir.join(writer).join(&generic);
            if p.exists() { return Some(p); }
        }
    }

    None
}

/// Resolve a module directory by name.
/// Checks page sidecar first, then root sidecar.
pub fn resolve_module_dir(name: &str, _project_root: &Path) -> Option<PathBuf> {
    let candidates = [
        get_page_sidecar().map(|s| s.join("modules").join(name)),
        get_root_sidecar().map(|s| s.join("modules").join(name)),
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
            let expected = get_root_sidecar()
                .or_else(get_page_sidecar)
                .unwrap_or_else(|| ctx.project_root.clone())
                .join("modules").join(plugin).join("module.toml");
            errors.push(format!(
                "  calepin.plugins: {}\n    -> not found: {}",
                plugin,
                expected.display()
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
