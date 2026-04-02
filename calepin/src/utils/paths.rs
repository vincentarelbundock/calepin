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
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// Active target name (thread-local)
// ---------------------------------------------------------------------------

thread_local! {
    static ACTIVE_TARGET: RefCell<Option<String>> = RefCell::new(None);
    /// Target inheritance chain (child-first), e.g. ["slides", "html"].
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
/// Chain is child-first, e.g. ["slides", "html"].
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
/// Returns `None` if the directory does not exist.
pub fn resolve_sidecar_dir(input: &Path) -> Option<PathBuf> {
    let stem = input.file_stem()?.to_string_lossy();
    let sidecar_name = format!("{}_calepin", stem);
    let dir = input.parent()?.join(&sidecar_name);
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

/// Resolve the sidecar directory for an input file, creating it if it
/// does not exist. Use this only for top-level entry points (document mode
/// via `resolve_context`), not for per-page sidecars in collection mode.
pub fn resolve_or_create_sidecar_dir(input: &Path) -> Option<PathBuf> {
    let stem = input.file_stem()?.to_string_lossy();
    let sidecar_name = format!("{}_calepin", stem);
    let dir = input.parent()?.join(&sidecar_name);
    if !dir.is_dir() {
        let _ = std::fs::create_dir_all(&dir);
    }
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

/// Resolve a target name to its embedded directory name.
/// Handles aliases (e.g., "html" -> "tailwind") by checking the builtin
/// extension manifest's actual name, which matches the directory in targets/.
pub fn resolve_embedded_dir_name(name: &str) -> String {
    crate::config::extension::builtin_extension(name)
        .map(|m| m.name)
        .unwrap_or_else(|| name.to_string())
}

pub fn ensure_sidecar_populated(sidecar: &Path, target: &str) {
    use crate::render::elements::BUILTIN_EXTENSIONS;

    // Write templates for the target (child-first so child templates take priority,
    // then parent fills in any missing templates as fallback)
    let empty = std::collections::HashMap::new();
    let project_root = sidecar.parent().unwrap_or(std::path::Path::new("."));
    let chain = crate::config::extension::inheritance_chain(project_root, target, &empty);
    let dest = sidecar.join("templates").join(target);
    for chain_target in &chain {
        let dir_name = resolve_embedded_dir_name(chain_target);
        let tpl_path = format!("{}/templates", dir_name);
        if let Some(dir) = BUILTIN_EXTENSIONS.get_dir(&tpl_path) {
            write_builtin_dir_if_missing(dir, &tpl_path, &dest);
        }
    }

    // Overlay templates from installed extensions (child-first).
    // Extension templates override auto-populated built-in copies.
    overlay_extension_templates(sidecar, target, &chain, &dest);

    // Write built-in assets from the inheritance chain (child-first, then parent fills gaps)
    let assets_dest = sidecar.join("assets");
    let _ = std::fs::create_dir_all(&assets_dest);
    for chain_target in &chain {
        let dir_name = resolve_embedded_dir_name(chain_target);
        let assets_path = format!("{}/assets", dir_name);
        if let Some(assets_dir) = BUILTIN_EXTENSIONS.get_dir(&assets_path) {
            write_builtin_dir_if_missing(assets_dir, &assets_path, &assets_dest);
        }
    }
}

/// Recursively write files from a built-in directory, stripping the prefix.
/// Only writes files that do not already exist (sidecar overrides are preserved).
fn write_builtin_dir_if_missing(dir: &include_dir::Dir<'static>, prefix: &str, dest: &std::path::Path) {
    let prefix_path = std::path::Path::new(prefix);
    for file in dir.files() {
        let rel = file.path().strip_prefix(prefix_path).unwrap_or(file.path());
        let out = dest.join(rel);
        if out.exists() { continue; }
        if let Some(parent) = out.parent() { let _ = std::fs::create_dir_all(parent); }
        if let Some(content) = file.contents_utf8() {
            let _ = std::fs::write(&out, content);
        } else {
            let _ = std::fs::write(&out, file.contents());
        }
    }
    for subdir in dir.dirs() {
        let subdir_name = subdir.path().strip_prefix(prefix_path)
            .unwrap_or(subdir.path());
        write_builtin_dir_if_missing(subdir, &subdir.path().display().to_string(), &dest.join(subdir_name));
    }
}

/// Copy templates from installed extensions into the sidecar templates directory.
/// Installed extension templates override auto-populated built-in copies.
/// Walks the inheritance chain child-first: each extension in the chain contributes
/// templates from its `templates/{target}/` or `templates/{writer}/` subdirectory.
fn overlay_extension_templates(sidecar: &Path, target: &str, chain: &[String], dest: &Path) {
    let ext_dirs = [
        Some(sidecar.join("extensions")),
    ];
    // Walk chain child-first so the most specific extension wins
    for chain_name in chain {
        for ext_base in ext_dirs.iter().flatten() {
            let ext_dir = ext_base.join(chain_name);
            if !ext_dir.join("extension.toml").exists() { continue; }
            let tpl_base = ext_dir.join("templates");
            if !tpl_base.is_dir() { continue; }
            // Check target-named subdir first, then each chain name as writer fallback
            let mut candidates: Vec<std::path::PathBuf> = vec![tpl_base.join(target)];
            for name in chain {
                let p = tpl_base.join(name);
                if !candidates.contains(&p) { candidates.push(p); }
            }
            for src_dir in &candidates {
                if !src_dir.is_dir() { continue; }
                copy_dir_overwrite(src_dir, dest);
                break; // use first matching subdir
            }
        }
    }
}

/// Copy files from src to dest, overwriting existing files.
fn copy_dir_overwrite(src: &Path, dest: &Path) {
    let entries = match std::fs::read_dir(src) {
        Ok(e) => e,
        Err(_) => return,
    };
    let _ = std::fs::create_dir_all(dest);
    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let out = dest.join(&file_name);
        if path.is_file() {
            let _ = std::fs::copy(&path, &out);
        } else if path.is_dir() {
            copy_dir_overwrite(&path, &out);
        }
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

    /// Resolve the figure output directory for a given document stem.
    /// Places `{leaf_stem}_files/` next to the output file.
    pub fn figures_dir(&self, stem: &str) -> PathBuf {
        self.output_dir.join(figures_dir_name(stem))
    }

    /// Resolve the cache directory for a given document stem.
    /// Cache lives at `{project_root}/.calepin/cache/{stem}/`.
    pub fn cache_dir(&self, stem: &str) -> PathBuf {
        self.project_root.join(".calepin").join("cache").join(stem)
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
// Path helpers
// ---------------------------------------------------------------------------

/// Ensure a path is absolute. Relative paths are resolved against the current
/// working directory.
pub fn ensure_absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    }
}

/// Return the figures directory name for a given document stem: `{leaf}_files`.
pub fn figures_dir_name(stem: &str) -> String {
    let leaf = Path::new(stem).file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(stem);
    format!("{}_files", leaf)
}

/// Compute a figure path relative to the output directory.
/// Returns `{dir_basename}/{filename}`, e.g. `myfile_files/fig-plot-1.png`.
pub fn figure_rel_path(fig_dir: &Path, filename: &str) -> PathBuf {
    let dir_name = fig_dir.file_name().unwrap_or_default();
    Path::new(dir_name).join(filename)
}

/// When the user-requested output extension differs from the writer's native
/// extension (e.g. writer produces `.typ` but user wants `.pdf`), returns
/// `(intermediate_path, Some(final_path))`. When they match, returns
/// `(user_output, None)`.
pub fn resolve_intermediate_output(
    user_output: &Path,
    writer_ext: &str,
    has_post_commands: bool,
) -> (PathBuf, Option<PathBuf>) {
    let o_ext = user_output.extension().and_then(|e| e.to_str()).unwrap_or("");
    if o_ext != writer_ext && has_post_commands {
        (user_output.with_extension(writer_ext), Some(user_output.to_path_buf()))
    } else {
        (user_output.to_path_buf(), None)
    }
}

/// Copy the `{stem}_files/` directory from the input's parent to the output
/// directory so post commands (tectonic, typst) can find figure files.
/// Returns `Some(dst)` if a copy was made (caller should clean up after post
/// commands), or `None` if no copy was needed.
pub fn copy_figures_for_output(input: &Path, output_dir: &Path, stem: &str) -> Option<PathBuf> {
    let dir_name = figures_dir_name(stem);
    let src = input.parent().unwrap_or(Path::new(".")).join(&dir_name);
    let dst = output_dir.join(&dir_name);
    if src.is_dir() && !dst.exists() {
        copy_dir_recursive(&src, &dst).ok();
        Some(dst)
    } else {
        None
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

/// Default output directory suffix for collection builds: `{stem}_output`.
pub const DEFAULT_OUTPUT_SUFFIX: &str = "_output";

/// Resolve the output directory for a collection build.
/// Uses the config `output` field if set, otherwise `{stem}_output`
/// (where `stem` is the entry-point filename without extension, typically `index`).
/// The result is always absolute (joined to `project_root`).
pub fn output_dir(project_root: &Path, config_output: Option<&str>, stem: &str) -> PathBuf {
    let p = match config_output {
        Some(name) => PathBuf::from(name),
        None => PathBuf::from(format!("{}{}", stem, DEFAULT_OUTPUT_SUFFIX)),
    };
    if p.is_absolute() {
        p
    } else {
        project_root.join(p)
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
/// Modules use extension.toml as their manifest.
pub fn resolve_module_dir(name: &str, _project_root: &Path) -> Option<PathBuf> {
    let candidates = [
        get_page_sidecar().map(|s| s.join("modules").join(name)),
        get_root_sidecar().map(|s| s.join("modules").join(name)),
    ];
    candidates.into_iter()
        .flatten()
        .find(|dir| dir.join("extension.toml").exists())
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
                .join("modules").join(plugin).join("extension.toml");
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
        meta.plugins = vec!["tabset".to_string(), "theorem".to_string()];
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
