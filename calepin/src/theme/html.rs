use std::path::Path;

use anyhow::{anyhow, Context, Result};

use super::bundle::{require_builtin, BundleDef, CALEPIN};
use super::{
    dir_theme_name, read_theme_files, validate_theme_dir, ThemeSelection, DEFAULT_THEME_NAME,
};

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
            let bundle = require_builtin(name)?;
            if bundle.has_file(entry) {
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
            let bundle = require_builtin(name)?;
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

fn explicit_bundle_entry(
    bundle: &'static BundleDef,
    entry: &str,
    is_default: bool,
) -> Result<Option<HtmlEntry>> {
    if bundle.has_file(entry) {
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
        .file(entry)
        .map(str::to_string)
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
