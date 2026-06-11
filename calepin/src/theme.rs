//! Theme bundles. A theme is a directory named by family; the target and
//! scope dimensions are well-known filenames inside it: `paged.typ`,
//! `document.html`, `site.html`. See specs/2026-06-10-theme-bundles-design.md.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

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

static CALEPIN: BundleDef = BundleDef {
    name: "calepin",
    files: &[
        BundleFile {
            path: "document.html",
            source: include_str!("assets/themes/calepin/document.html"),
        },
        BundleFile {
            path: "site.html",
            source: include_str!("assets/themes/calepin/site.html"),
        },
        BundleFile {
            path: "paged.typ",
            source: include_str!("assets/themes/calepin/paged.typ"),
        },
        BundleFile {
            path: "partials/theme-switcher.html",
            source: include_str!("assets/themes/calepin/partials/theme-switcher.html"),
        },
        BundleFile {
            path: "styles/document.css",
            source: include_str!("assets/themes/calepin/styles/document.css"),
        },
        BundleFile {
            path: "styles/site.css",
            source: include_str!("assets/themes/calepin/styles/site.css"),
        },
        BundleFile {
            path: "scripts/site.js",
            source: include_str!("assets/themes/calepin/scripts/site.js"),
        },
    ],
};

static ACADEMIC: BundleDef = BundleDef {
    name: "academic",
    files: &[
        BundleFile {
            path: "site.html",
            source: include_str!("assets/themes/academic/site.html"),
        },
        BundleFile {
            path: "styles/main.css",
            source: include_str!("assets/themes/academic/styles/main.css"),
        },
        BundleFile {
            path: "scripts/main.js",
            source: include_str!("assets/themes/academic/scripts/main.js"),
        },
    ],
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
    if dest.exists() && !force {
        return Err(anyhow!(
            "{} already exists; pass --force to overwrite",
            dest.display()
        ));
    }
    for file in bundle.files {
        let path = dest.join(file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        std::fs::write(&path, file.source)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(dest)
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

/// The Typst source to inject for paged output. `None` disables paged
/// theming entirely. An empty file is returned as-is (no styling), while an
/// absent file falls back to the default bundle's paged.typ.
pub fn paged_source(selection: &ThemeSelection) -> Result<Option<String>> {
    let default_source = || {
        CALEPIN
            .files
            .iter()
            .find(|file| file.path == "paged.typ")
            .map(|file| file.source.to_string())
            .expect("builtin calepin bundle ships paged.typ")
    };
    match selection {
        ThemeSelection::Disabled => Ok(None),
        ThemeSelection::Default => Ok(Some(default_source())),
        ThemeSelection::Builtin(name) => {
            let bundle = builtin_bundle(name).ok_or_else(|| {
                anyhow!(
                    "unknown theme `{name}`; use one of {}",
                    builtin_names().join(", ")
                )
            })?;
            Ok(Some(
                bundle
                    .files
                    .iter()
                    .find(|file| file.path == "paged.typ")
                    .map(|file| file.source.to_string())
                    .unwrap_or_else(default_source),
            ))
        }
        ThemeSelection::Dir(dir) => {
            validate_theme_dir(dir)?;
            let path = dir.join("paged.typ");
            if path.is_file() {
                let source = std::fs::read_to_string(&path)
                    .with_context(|| format!("failed to read {}", path.display()))?;
                Ok(Some(source))
            } else {
                Ok(Some(default_source()))
            }
        }
    }
}

fn validate_theme_dir(dir: &Path) -> Result<()> {
    let has_entry = ["paged.typ", "document.html", "site.html"]
        .iter()
        .any(|file| dir.join(file).is_file());
    if !has_entry {
        return Err(anyhow!(
            "theme directory {} contains none of paged.typ, document.html, site.html",
            dir.display()
        ));
    }
    Ok(())
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
    let name = dir
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| dir.display().to_string());
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
        assert!(paged_source(&ThemeSelection::Disabled).unwrap().is_none());
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
        // paged.typ absent: default paged styling
        assert_eq!(
            paged_source(&sel).unwrap(),
            paged_source(&ThemeSelection::Default).unwrap()
        );
    }

    #[test]
    fn empty_paged_typ_means_no_styling_not_fallback() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("paged.typ"), "").unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());
        assert_eq!(paged_source(&sel).unwrap(), Some(String::new()));
    }

    #[test]
    fn theme_dir_without_any_entry_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());
        assert!(resolve_html_entry(&sel, HtmlScope::Site).is_err());
    }

    #[test]
    fn eject_builtin_copies_default_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let dest = eject_builtin(DEFAULT_THEME_NAME, &dir.path().join("themes"), false).unwrap();

        assert_eq!(dest, dir.path().join("themes/calepin"));
        assert!(dest.join("document.html").is_file());
        assert!(dest.join("site.html").is_file());
        assert!(dest.join("paged.typ").is_file());
        assert!(dest.join("styles/site.css").is_file());
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
