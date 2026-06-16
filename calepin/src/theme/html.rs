use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use super::bundle::{require_builtin, shared_file, BundleDef, CALEPIN};
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
            Self::Document => "layouts/notebook.html",
            Self::Site => "layouts/webpage.html",
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
    /// (file name, css), shared imports first, then theme-local files.
    pub styles: Vec<(String, String)>,
    /// (file name, js), shared imports first, then theme-local files.
    pub scripts: Vec<(String, String)>,
    /// True when the layout came from the builtin default bundle (either
    /// because it was selected or via fallback).
    pub is_default: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ThemeManifest {
    shared: SharedImports,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SharedImports {
    partials: Vec<String>,
    styles: Vec<String>,
    scripts: Vec<String>,
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
            } else if !dir.join(HtmlScope::Site.entry_file()).is_file() {
                explicit_bundle_entry(&CALEPIN, layout_path, true)
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
    let manifest = bundle_manifest(bundle)?;
    Ok(HtmlEntry {
        theme_name: bundle.name.to_string(),
        layout,
        partials: bundle_assets(bundle, &manifest.shared.partials, "partials/", "html")?
            .into_iter()
            .map(|(name, source)| (format!("partials/{name}"), source))
            .collect(),
        styles: bundle_assets(bundle, &manifest.shared.styles, "styles/", "css")?,
        scripts: bundle_assets(bundle, &manifest.shared.scripts, "scripts/", "js")?,
        is_default,
    })
}

fn dir_entry(dir: &Path, entry: &str) -> Result<HtmlEntry> {
    let layout_path = dir.join(entry);
    let layout = std::fs::read_to_string(&layout_path)
        .with_context(|| format!("failed to read {}", layout_path.display()))?;
    let name = dir_theme_name(dir);
    let manifest = dir_manifest(dir)?;
    Ok(HtmlEntry {
        theme_name: name,
        layout,
        partials: dir_assets(dir, &manifest.shared.partials, "partials", "html")?
            .into_iter()
            .map(|(file, source)| (format!("partials/{file}"), source))
            .collect(),
        styles: dir_assets(dir, &manifest.shared.styles, "styles", "css")?,
        scripts: dir_assets(dir, &manifest.shared.scripts, "scripts", "js")?,
        is_default: false,
    })
}

fn bundle_manifest(bundle: &BundleDef) -> Result<ThemeManifest> {
    let Some(source) = bundle.file("theme.toml") else {
        return Ok(ThemeManifest::default());
    };
    toml::from_str(source)
        .with_context(|| format!("failed to parse builtin theme `{}` theme.toml", bundle.name))
}

fn dir_manifest(dir: &Path) -> Result<ThemeManifest> {
    let path = dir.join("theme.toml");
    if !path.is_file() {
        return Ok(ThemeManifest::default());
    }
    let source = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&source).with_context(|| format!("failed to parse {}", path.display()))
}

fn bundle_assets(
    bundle: &BundleDef,
    imports: &[String],
    prefix: &str,
    ext: &str,
) -> Result<Vec<(String, String)>> {
    let local_files = bundle_files(bundle, prefix, ext);
    let mut imported = std::collections::BTreeSet::new();
    let mut files = Vec::new();
    for name in imports {
        validate_shared_import(name, ext)?;
        if !imported.insert(name.clone()) {
            return Err(anyhow!(
                "shared {ext} import `{name}` is listed more than once"
            ));
        }
        let path = format!("{prefix}{name}");
        let source = bundle
            .file(&path)
            .or_else(|| shared_file(&path))
            .ok_or_else(|| anyhow!("shared {ext} import `{name}` was not found"))?;
        files.push((name.clone(), source.to_string()));
    }
    files.extend(
        local_files
            .into_iter()
            .filter(|(name, _)| !imported.contains(name)),
    );
    Ok(files)
}

fn bundle_files(bundle: &BundleDef, prefix: &str, ext: &str) -> Vec<(String, String)> {
    let suffix = format!(".{ext}");
    let mut files: Vec<(String, String)> = bundle
        .files
        .iter()
        .filter(|file| file.path.starts_with(prefix) && file.path.ends_with(&suffix))
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
}

fn dir_assets(
    dir: &Path,
    imports: &[String],
    subdir: &str,
    ext: &str,
) -> Result<Vec<(String, String)>> {
    let local_files = read_theme_files(&dir.join(subdir), ext)?;
    let mut imported = std::collections::BTreeSet::new();
    let mut files = Vec::new();
    for name in imports {
        validate_shared_import(name, ext)?;
        if !imported.insert(name.clone()) {
            return Err(anyhow!(
                "shared {ext} import `{name}` is listed more than once"
            ));
        }
        if let Some((_, source)) = local_files.iter().find(|(file, _)| file == name) {
            files.push((name.clone(), source.clone()));
            continue;
        }
        let path = format!("{subdir}/{name}");
        let source = dir_shared_file(dir, &path)?
            .or_else(|| shared_file(&path).map(str::to_string))
            .ok_or_else(|| anyhow!("shared {ext} import `{name}` was not found"))?;
        files.push((name.clone(), source));
    }
    files.extend(
        local_files
            .into_iter()
            .filter(|(name, _)| !imported.contains(name)),
    );
    Ok(files)
}

fn dir_shared_file(dir: &Path, relative: &str) -> Result<Option<String>> {
    let Some(parent) = dir.parent() else {
        return Ok(None);
    };
    let path = parent.join("shared").join(relative);
    if !path.is_file() {
        return Ok(None);
    }
    std::fs::read_to_string(&path)
        .map(Some)
        .with_context(|| format!("failed to read {}", path.display()))
}

fn validate_shared_import(name: &str, ext: &str) -> Result<()> {
    if name.trim() != name || name.is_empty() {
        return Err(anyhow!("shared import names must be non-empty filenames"));
    }
    if name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err(anyhow!(
            "shared import `{name}` must be a filename, not a path"
        ));
    }
    let path = Path::new(name);
    if path.components().count() != 1
        || path.file_name().and_then(|file| file.to_str()) != Some(name)
    {
        return Err(anyhow!(
            "shared import `{name}` must be a filename, not a path"
        ));
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some(ext) {
        return Err(anyhow!("shared import `{name}` must be a .{ext} file"));
    }
    Ok(())
}
