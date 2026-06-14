//! Theme bundles. A theme is a directory named by family; the target and
//! scope dimensions are well-known filenames inside it: `paged.typ.jinja`,
//! `document.html`, `site.html`. See specs/2026-06-10-theme-bundles-design.md.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use minijinja::{AutoEscape, Environment};
use serde::Serialize;
use serde_json::Value;

pub const DEFAULT_THEME_NAME: &str = "calepin";

/// Where a render gets its theme from. `Default` means "no selection made",
/// which resolves to the builtin `calepin` bundle.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ThemeSelection {
    #[default]
    Default,
    /// `theme = false`: raw output, no theming for any target.
    Disabled,
    Builtin(&'static str),
    Dir(PathBuf),
}

pub(crate) struct BundleFile {
    pub(crate) path: &'static str,
    pub(crate) source: &'static str,
}

pub(crate) struct BundleDef {
    pub(crate) name: &'static str,
    pub(crate) files: &'static [BundleFile],
}

include!(concat!(env!("OUT_DIR"), "/theme_assets.rs"));

static CALEPIN: BundleDef = BundleDef {
    name: "calepin",
    files: CALEPIN_FILES,
};

static ACADEMIC: BundleDef = BundleDef {
    name: "academic",
    files: ACADEMIC_FILES,
};

static BUILTINS: [&BundleDef; 2] = [&CALEPIN, &ACADEMIC];

pub fn builtin_names() -> Vec<&'static str> {
    BUILTINS.iter().map(|bundle| bundle.name).collect()
}

pub(crate) fn builtin_bundle(name: &str) -> Option<&'static BundleDef> {
    BUILTINS.iter().copied().find(|bundle| bundle.name == name)
}

/// Copy a builtin bundle's files into `themes_dir/<name>/`, refusing to touch
/// an existing destination unless `force`.
pub fn eject_builtin(name: &str, themes_dir: &Path, force: bool) -> Result<PathBuf> {
    let bundle = builtin_bundle(name).ok_or_else(|| {
        anyhow!(
            "unknown theme `{name}`; use one of {}",
            builtin_names().join(", ")
        )
    })?;
    let dest = themes_dir.join(bundle.name);
    eject_builtin_to(name, &dest, force)
}

/// Copy a builtin bundle's files into `dest`, refusing to touch an existing
/// destination unless `force`.
pub fn eject_builtin_to(name: &str, dest: &Path, force: bool) -> Result<PathBuf> {
    let bundle = builtin_bundle(name).ok_or_else(|| {
        anyhow!(
            "unknown theme `{name}`; use one of {}",
            builtin_names().join(", ")
        )
    })?;
    if dest.exists() && !force {
        return Err(anyhow!(
            "{} already exists; pass --force to overwrite",
            dest.display()
        ));
    }
    for file in bundle.files {
        write_theme_file(&dest, file.path, file.source)?;
    }
    Ok(dest.to_path_buf())
}

fn write_theme_file(dest: &Path, relative: &str, source: &str) -> Result<()> {
    let path = dest.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(&path, source).with_context(|| format!("failed to write {}", path.display()))
}

impl ThemeSelection {
    /// Parse a CLI or config value. Relative path-like values resolve
    /// against `base_dir` (the config file's directory, or the CLI's cwd).
    pub fn parse(value: &str, base_dir: &Path) -> Result<Self> {
        if value == "false" || value == "none" {
            return Ok(Self::Disabled);
        }
        if let Some(name) = builtin_names().into_iter().find(|name| *name == value) {
            return Ok(Self::Builtin(name));
        }
        if is_path_like(value) {
            let path = Path::new(value);
            let path = if path.is_absolute() {
                path.to_path_buf()
            } else {
                base_dir.join(path)
            };
            if !path.is_dir() {
                return Err(anyhow!("theme directory not found: {}", path.display()));
            }
            return Ok(Self::Dir(path));
        }
        Err(anyhow!(
            "unknown theme `{value}`; use one of {} or a path to a theme directory",
            builtin_names().join(", ")
        ))
    }
}

/// Same heuristic the old html theme used: anything that looks like a path
/// (separator, leading dot, absolute) is treated as a directory reference.
fn is_path_like(value: &str) -> bool {
    let path = Path::new(value);
    path.is_absolute()
        || path.components().count() > 1
        || value.starts_with('.')
        || value.contains('\\')
        || value.ends_with('/')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlScope {
    Document,
    Site,
}

impl HtmlScope {
    fn entry_file(self) -> &'static str {
        match self {
            Self::Document => "document.html",
            Self::Site => "site.html",
        }
    }
}

/// An HTML entry resolved to render-ready sources. The asset lists come from
/// the same bundle that provided the layout (the spec's "asset closure").
pub struct HtmlEntry {
    pub theme_name: String,
    pub layout: String,
    /// ("partials/<file>.html", source)
    pub partials: Vec<(String, String)>,
    /// (file name, css), sorted by file name
    pub styles: Vec<(String, String)>,
    /// (file name, js), sorted by file name
    pub scripts: Vec<(String, String)>,
    /// True when the layout came from the builtin default bundle (either
    /// because it was selected or via fallback).
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub struct PagedTemplateContext {
    pub input_path: String,
    pub input_dir: String,
    pub input_stem: String,
    pub body: String,
    pub page_meta: Value,
    pub params: Value,
}

impl Default for PagedTemplateContext {
    fn default() -> Self {
        Self {
            input_path: String::new(),
            input_dir: String::new(),
            input_stem: String::new(),
            body: String::new(),
            page_meta: Value::Null,
            params: Value::Object(serde_json::Map::new()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagedSource {
    pub source: String,
    pub owns_body: bool,
}

#[derive(Serialize)]
struct PagedTemplateRenderContext<'a> {
    theme: &'a str,
    target: &'static str,
    document: PagedDocumentContext<'a>,
    params: &'a Value,
}

#[derive(Serialize)]
struct PagedDocumentContext<'a> {
    path: &'a str,
    dir: &'a str,
    stem: &'a str,
    body: &'a str,
    meta: &'a Value,
}

/// Resolve the layout for `scope`. `None` means theming is disabled.
/// Fallback: if the selected theme lacks the entry file, use the builtin
/// default bundle's entry with the default's assets. One level, no chains.
pub fn resolve_html_entry(
    selection: &ThemeSelection,
    scope: HtmlScope,
) -> Result<Option<HtmlEntry>> {
    let entry = scope.entry_file();
    match selection {
        ThemeSelection::Disabled => Ok(None),
        ThemeSelection::Default => Ok(Some(bundle_entry(&CALEPIN, entry, true)?)),
        ThemeSelection::Builtin(name) => {
            let bundle = builtin_bundle(name).ok_or_else(|| {
                anyhow!(
                    "unknown theme `{name}`; use one of {}",
                    builtin_names().join(", ")
                )
            })?;
            if bundle.files.iter().any(|file| file.path == entry) {
                Ok(Some(bundle_entry(
                    bundle,
                    entry,
                    bundle.name == DEFAULT_THEME_NAME,
                )?))
            } else {
                Ok(Some(bundle_entry(&CALEPIN, entry, true)?))
            }
        }
        ThemeSelection::Dir(dir) => {
            validate_theme_dir(dir)?;
            if dir.join(entry).is_file() {
                Ok(Some(dir_entry(dir, entry)?))
            } else {
                Ok(Some(bundle_entry(&CALEPIN, entry, true)?))
            }
        }
    }
}

/// Resolve a website layout at an explicit theme-relative path. Unlike the
/// standard site/document entries, explicit page layouts never fall back to
/// the default theme: the path must exist exactly as written.
pub fn resolve_explicit_site_html_entry(
    selection: &ThemeSelection,
    layout_path: &str,
) -> Result<Option<HtmlEntry>> {
    validate_explicit_html_layout_path(layout_path)?;
    match selection {
        ThemeSelection::Disabled => Err(anyhow!(
            "page layout `{layout_path}` requires an HTML theme, but theming is disabled"
        )),
        ThemeSelection::Default => explicit_bundle_entry(&CALEPIN, layout_path, true),
        ThemeSelection::Builtin(name) => {
            let bundle = builtin_bundle(name).ok_or_else(|| {
                anyhow!(
                    "unknown theme `{name}`; use one of {}",
                    builtin_names().join(", ")
                )
            })?;
            explicit_bundle_entry(bundle, layout_path, bundle.name == DEFAULT_THEME_NAME)
        }
        ThemeSelection::Dir(dir) => {
            validate_theme_dir(dir)?;
            if dir.join(layout_path).is_file() {
                Ok(Some(dir_entry(dir, layout_path)?))
            } else {
                Err(anyhow!(
                    "theme `{}` does not contain page layout `{layout_path}`",
                    dir_theme_name(dir)
                ))
            }
        }
    }
}

/// The Typst source to inject for paged output. `None` disables paged
/// theming entirely. An empty paged.typ.jinja file is rendered as-is (no
/// styling), while an absent file falls back to the default bundle's
/// paged.typ.jinja.
pub fn paged_source(
    selection: &ThemeSelection,
    context: &PagedTemplateContext,
) -> Result<Option<PagedSource>> {
    let default_source = || {
        CALEPIN
            .files
            .iter()
            .find(|file| file.path == "paged.typ.jinja")
            .map(|file| file.source.to_string())
            .expect("builtin calepin bundle ships paged.typ.jinja")
    };
    let render = |name: &str, source: String| {
        let owns_body = source.contains("document.body");
        render_paged_template(name, source, context).map(|source| PagedSource { source, owns_body })
    };
    match selection {
        ThemeSelection::Disabled => Ok(None),
        ThemeSelection::Default => render(DEFAULT_THEME_NAME, default_source()).map(Some),
        ThemeSelection::Builtin(name) => {
            let bundle = builtin_bundle(name).ok_or_else(|| {
                anyhow!(
                    "unknown theme `{name}`; use one of {}",
                    builtin_names().join(", ")
                )
            })?;
            let source = bundle
                .files
                .iter()
                .find(|file| file.path == "paged.typ.jinja")
                .map(|file| file.source.to_string())
                .unwrap_or_else(default_source);
            render(bundle.name, source).map(Some)
        }
        ThemeSelection::Dir(dir) => {
            validate_theme_dir(dir)?;
            let template_path = dir.join("paged.typ.jinja");
            if template_path.is_file() {
                let source = std::fs::read_to_string(&template_path)
                    .with_context(|| format!("failed to read {}", template_path.display()))?;
                let name = dir_theme_name(dir);
                render(&name, source).map(Some)
            } else {
                render(DEFAULT_THEME_NAME, default_source()).map(Some)
            }
        }
    }
}

fn render_paged_template(
    theme_name: &str,
    source: String,
    context: &PagedTemplateContext,
) -> Result<String> {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|_| AutoEscape::None);
    env.add_template_owned("paged.typ.jinja", source)
        .map_err(|error| paged_template_error(theme_name, error))?;
    let template = env
        .get_template("paged.typ.jinja")
        .map_err(|error| paged_template_error(theme_name, error))?;
    template
        .render(PagedTemplateRenderContext {
            theme: theme_name,
            target: "paged",
            document: PagedDocumentContext {
                path: &context.input_path,
                dir: &context.input_dir,
                stem: &context.input_stem,
                body: &context.body,
                meta: &context.page_meta,
            },
            params: &context.params,
        })
        .map_err(|error| paged_template_error(theme_name, error))
}

fn paged_template_error(name: &str, error: minijinja::Error) -> anyhow::Error {
    anyhow!("theme `{name}` paged.typ.jinja: {error}")
}

fn validate_explicit_html_layout_path(value: &str) -> Result<()> {
    let path = Path::new(value);
    if value.trim() != value || value.is_empty() {
        return Err(anyhow!(
            "page layout path must be a non-empty relative path"
        ));
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("html") {
        return Err(anyhow!(
            "page layout path must name an .html file: `{value}`"
        ));
    }
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(anyhow!(
            "page layout path must stay inside the active theme: `{value}`"
        ));
    }
    Ok(())
}

fn validate_theme_dir(dir: &Path) -> Result<()> {
    let has_entry = ["paged.typ.jinja", "document.html", "site.html"]
        .iter()
        .any(|file| dir.join(file).is_file());
    if !has_entry {
        return Err(anyhow!(
            "theme directory {} contains none of paged.typ.jinja, document.html, site.html",
            dir.display()
        ));
    }
    Ok(())
}

fn explicit_bundle_entry(
    bundle: &'static BundleDef,
    entry: &str,
    is_default: bool,
) -> Result<Option<HtmlEntry>> {
    if bundle.files.iter().any(|file| file.path == entry) {
        Ok(Some(bundle_entry(bundle, entry, is_default)?))
    } else {
        Err(anyhow!(
            "theme `{}` does not contain page layout `{entry}`",
            bundle.name
        ))
    }
}

fn bundle_entry(bundle: &'static BundleDef, entry: &str, is_default: bool) -> Result<HtmlEntry> {
    let layout = bundle
        .files
        .iter()
        .find(|file| file.path == entry)
        .map(|file| file.source.to_string())
        .ok_or_else(|| anyhow!("builtin theme `{}` is missing `{entry}`", bundle.name))?;
    let collect = |prefix: &str, ext: &str| -> Vec<(String, String)> {
        let mut files: Vec<(String, String)> = bundle
            .files
            .iter()
            .filter(|file| file.path.starts_with(prefix) && file.path.ends_with(ext))
            .map(|file| {
                let name = file
                    .path
                    .rsplit_once('/')
                    .map(|(_, name)| name)
                    .unwrap_or(file.path);
                (name.to_string(), file.source.to_string())
            })
            .collect();
        files.sort_by(|a, b| a.0.cmp(&b.0));
        files
    };
    Ok(HtmlEntry {
        theme_name: bundle.name.to_string(),
        layout,
        partials: collect("partials/", ".html")
            .into_iter()
            .map(|(name, source)| (format!("partials/{name}"), source))
            .collect(),
        styles: collect("styles/", ".css"),
        scripts: collect("scripts/", ".js"),
        is_default,
    })
}

fn dir_entry(dir: &Path, entry: &str) -> Result<HtmlEntry> {
    let layout_path = dir.join(entry);
    let layout = std::fs::read_to_string(&layout_path)
        .with_context(|| format!("failed to read {}", layout_path.display()))?;
    let name = dir_theme_name(dir);
    Ok(HtmlEntry {
        theme_name: name,
        layout,
        partials: read_theme_files(&dir.join("partials"), "html")?
            .into_iter()
            .map(|(file, source)| (format!("partials/{file}"), source))
            .collect(),
        styles: read_theme_files(&dir.join("styles"), "css")?,
        scripts: read_theme_files(&dir.join("scripts"), "js")?,
        is_default: false,
    })
}

fn dir_theme_name(dir: &Path) -> String {
    dir.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| dir.display().to_string())
}

/// Read every `*.<ext>` file in `dir`, sorted by filename for deterministic
/// order. A missing directory yields no files.
fn read_theme_files(dir: &Path, ext: &str) -> Result<Vec<(String, String)>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("failed to read {}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some(ext))
        .collect();
    paths.sort();

    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        files.push((name, contents));
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parse_builtin_names() {
        let base = Path::new("/tmp");
        assert_eq!(
            ThemeSelection::parse("calepin", base).unwrap(),
            ThemeSelection::Builtin("calepin")
        );
        assert_eq!(
            ThemeSelection::parse("academic", base).unwrap(),
            ThemeSelection::Builtin("academic")
        );
    }

    #[test]
    fn parse_false_and_none_disable() {
        let base = Path::new("/tmp");
        assert_eq!(
            ThemeSelection::parse("false", base).unwrap(),
            ThemeSelection::Disabled
        );
        assert_eq!(
            ThemeSelection::parse("none", base).unwrap(),
            ThemeSelection::Disabled
        );
    }

    #[test]
    fn parse_existing_dir_resolves_relative_to_base() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("mytheme")).unwrap();
        let sel = ThemeSelection::parse("mytheme/", dir.path()).unwrap();
        assert_eq!(sel, ThemeSelection::Dir(dir.path().join("mytheme/")));
    }

    #[test]
    fn parse_unknown_name_errors_with_builtin_list() {
        let err = ThemeSelection::parse("zensical", Path::new("/tmp")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("calepin"), "{msg}");
        assert!(msg.contains("academic"), "{msg}");
    }

    #[test]
    fn parse_missing_dir_errors() {
        let dir = tempfile::tempdir().unwrap();
        let err = ThemeSelection::parse("does/not/exist", dir.path()).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn academic_falls_back_to_default_for_document_scope() {
        let sel = ThemeSelection::Builtin("academic");
        let entry = resolve_html_entry(&sel, HtmlScope::Document)
            .unwrap()
            .unwrap();
        assert_eq!(entry.theme_name, "calepin");
        assert!(entry.is_default);
        let site = resolve_html_entry(&sel, HtmlScope::Site).unwrap().unwrap();
        assert_eq!(site.theme_name, "academic");
        assert!(!site.is_default);
    }

    #[test]
    fn disabled_selection_resolves_to_no_entry_and_no_paged_source() {
        assert!(
            resolve_html_entry(&ThemeSelection::Disabled, HtmlScope::Site)
                .unwrap()
                .is_none()
        );
        assert!(
            paged_source(&ThemeSelection::Disabled, &PagedTemplateContext::default())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn dir_theme_uses_own_entry_and_falls_back_per_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("site.html"),
            "<html><head></head><body>X</body></html>",
        )
        .unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());
        let site = resolve_html_entry(&sel, HtmlScope::Site).unwrap().unwrap();
        assert!(!site.is_default);
        let document = resolve_html_entry(&sel, HtmlScope::Document)
            .unwrap()
            .unwrap();
        assert!(document.is_default);
        // paged.typ.jinja absent: default paged styling
        assert_eq!(
            paged_source(&sel, &PagedTemplateContext::default()).unwrap(),
            paged_source(&ThemeSelection::Default, &PagedTemplateContext::default()).unwrap()
        );
    }

    #[test]
    fn explicit_site_layout_uses_exact_theme_relative_path() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("layouts")).unwrap();
        std::fs::write(dir.path().join("site.html"), "{{ doc.body }}").unwrap();
        std::fs::write(dir.path().join("layouts/landing.html"), "landing").unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());

        let entry = resolve_explicit_site_html_entry(&sel, "layouts/landing.html")
            .unwrap()
            .unwrap();

        assert_eq!(entry.layout, "landing");
        assert!(!entry.is_default);
    }

    #[test]
    fn explicit_builtin_landing_layout_resolves() {
        let entry =
            resolve_explicit_site_html_entry(&ThemeSelection::Default, "layouts/landing.html")
                .unwrap()
                .unwrap();

        assert!(entry.layout.contains("calepin-website-main--landing"));
        assert!(entry.is_default);
    }

    #[test]
    fn explicit_site_layout_rejects_sugar_and_escape_paths() {
        for value in [
            "landing",
            "landing.typ",
            "../landing.html",
            "/tmp/landing.html",
        ] {
            let err = match resolve_explicit_site_html_entry(&ThemeSelection::Default, value) {
                Ok(_) => panic!("expected `{value}` to be rejected"),
                Err(error) => error.to_string(),
            };
            assert!(
                err.contains("page layout path") || err.contains("inside the active theme"),
                "{err}"
            );
        }
    }

    #[test]
    fn explicit_site_layout_does_not_fallback_to_default_theme() {
        let err = match resolve_explicit_site_html_entry(
            &ThemeSelection::Builtin("academic"),
            "document.html",
        ) {
            Ok(_) => panic!("expected missing explicit layout to be rejected"),
            Err(error) => error.to_string(),
        };

        assert!(err.contains("does not contain page layout"), "{err}");
    }

    #[test]
    fn empty_paged_typ_jinja_means_no_styling_not_fallback() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("paged.typ.jinja"), "").unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());
        assert_eq!(
            paged_source(&sel, &PagedTemplateContext::default()).unwrap(),
            Some(PagedSource {
                source: String::new(),
                owns_body: false,
            })
        );
    }

    #[test]
    fn paged_typ_jinja_can_use_calepin_context() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("paged.typ.jinja"),
            r#"#let title = "{{ document.meta.title }}"
#let species = "{{ params.Species }}"
"#,
        )
        .unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());
        let context = PagedTemplateContext {
            input_path: "reports/iris.typ".to_string(),
            input_dir: "reports".to_string(),
            input_stem: "iris".to_string(),
            body: "#include \"/.calepin/reports/iris/source.typ\"".to_string(),
            page_meta: serde_json::json!({"title": "Iris Report"}),
            params: serde_json::json!({"Species": "setosa"}),
        };

        let source = paged_source(&sel, &context).unwrap().unwrap();
        assert_eq!(
            source.source,
            "#let title = \"Iris Report\"\n#let species = \"setosa\""
        );
        assert!(!source.owns_body);
    }

    #[test]
    fn paged_typ_jinja_can_place_document_body() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("paged.typ.jinja"),
            r#"#set text(size: 11pt)
{{ document.body }}
[#emph[Generated footer]]
"#,
        )
        .unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());
        let context = PagedTemplateContext {
            input_path: "paper.typ".to_string(),
            input_dir: String::new(),
            input_stem: "paper".to_string(),
            body: "#include \"/.calepin/paper/source.typ\"".to_string(),
            page_meta: serde_json::Value::Null,
            params: serde_json::json!({}),
        };

        let source = paged_source(&sel, &context).unwrap().unwrap();
        assert_eq!(
            source.source,
            "#set text(size: 11pt)\n#include \"/.calepin/paper/source.typ\"\n[#emph[Generated footer]]"
        );
        assert!(source.owns_body);
    }

    #[test]
    fn theme_dir_without_any_entry_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());
        assert!(resolve_html_entry(&sel, HtmlScope::Site).is_err());
    }

    #[test]
    fn theme_dir_with_only_legacy_paged_typ_errors() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("paged.typ"), "#set text(size: 10pt)").unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());

        assert!(paged_source(&sel, &PagedTemplateContext::default()).is_err());
    }

    #[test]
    fn eject_builtin_copies_default_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let dest = eject_builtin(DEFAULT_THEME_NAME, &dir.path().join("themes"), false).unwrap();

        assert_eq!(dest, dir.path().join("themes/calepin"));
        assert!(dest.join("document.html").is_file());
        assert!(dest.join("site.html").is_file());
        assert!(dest.join("layouts/landing.html").is_file());
        assert!(dest.join("paged.typ.jinja").is_file());
        assert!(!dest.join("paged.typ").exists());
        assert!(dest.join("partials/navbar-item.html").is_file());
        assert!(dest.join("partials/theme-switcher.html").is_file());
        assert!(dest.join("partials/scripts.html").is_file());
        assert!(dest.join("partials/site-topbar.html").is_file());
        assert!(dest.join("partials/styles.html").is_file());
        assert!(dest.join("styles/00-theme.css").is_file());
        assert!(dest.join("styles/01-code.css").is_file());
        assert!(dest.join("styles/02-widgets.css").is_file());
        assert!(dest.join("styles/site.css").is_file());
        assert!(dest.join("scripts/00-theme-toggle.js").is_file());
        assert!(dest.join("scripts/01-language-picker.js").is_file());
        assert!(dest.join("scripts/02-copy-code.js").is_file());
        assert!(dest.join("scripts/site.js").is_file());

        assert!(std::fs::read_to_string(dest.join("styles/02-widgets.css"))
            .unwrap()
            .contains("[data-calepin-theme-toggle]"));
        assert!(
            std::fs::read_to_string(dest.join("scripts/02-copy-code.js"))
                .unwrap()
                .contains("window.CalepinCopyCode")
        );
    }

    #[test]
    fn eject_builtin_to_copies_into_requested_directory() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("custom-theme");
        let wrote = eject_builtin_to("academic", &dest, false).unwrap();

        assert_eq!(wrote, dest);
        assert!(wrote.join("site.html").is_file());
        assert!(wrote.join("partials/navbar-item.html").is_file());
        assert!(wrote.join("styles/00-theme.css").is_file());
        assert!(wrote.join("styles/01-code.css").is_file());
        assert!(wrote.join("styles/02-widgets.css").is_file());
        assert!(wrote.join("styles/main.css").is_file());
        assert!(wrote.join("scripts/00-theme-toggle.js").is_file());
        assert!(wrote.join("scripts/01-language-picker.js").is_file());
        assert!(wrote.join("scripts/02-copy-code.js").is_file());
        assert!(wrote.join("scripts/main.js").is_file());
    }

    #[test]
    fn eject_builtin_refuses_existing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let themes = dir.path().join("themes");
        std::fs::create_dir_all(themes.join("calepin")).unwrap();

        let err = eject_builtin(DEFAULT_THEME_NAME, &themes, false).unwrap_err();

        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn eject_builtin_unknown_name_lists_builtins() {
        let dir = tempfile::tempdir().unwrap();
        let err = eject_builtin("zensical", &dir.path().join("themes"), false).unwrap_err();

        assert!(err.to_string().contains("academic"));
    }
}
