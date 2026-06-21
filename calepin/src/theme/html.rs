use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use super::bundle::{require_builtin, shared_file, BundleDef, CALEPIN};
use super::{
    dir_theme_name, read_theme_files, read_theme_files_any, validate_theme_dir, ThemeSelection,
    DEFAULT_THEME_NAME,
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
#[derive(Debug, Clone)]
pub struct HtmlEntry {
    pub theme_name: String,
    pub layout: String,
    /// ("partials/<file>.html", source)
    pub partials: Vec<(String, String)>,
    /// (file name, css), shared imports first, then theme-local files.
    pub styles: Vec<(String, String)>,
    /// (file name, js), shared imports first, then theme-local files.
    pub scripts: Vec<(String, String)>,
    /// (file name, text), shared imports first, then theme-local files.
    pub assets: Vec<(String, String)>,
    /// True when the layout came from the builtin default bundle (either
    /// because it was selected or via fallback).
    pub is_default: bool,
}

impl HtmlEntry {
    pub fn append_styles(&mut self, styles: Vec<crate::config::CssOverride>) {
        self.styles
            .extend(styles.into_iter().map(|style| (style.name, style.css)));
    }
}

pub fn style_only_html_entry(styles: Vec<crate::config::CssOverride>) -> HtmlEntry {
    let mut entry = HtmlEntry {
        theme_name: "styles".to_string(),
        layout: r#"{{ doc.head }}
{% if site.stylesheet %}
  <link rel="stylesheet" href="{{ site.stylesheet }}">
{% else %}
  {% for style in styles %}
  <style>
{{ style.css }}
  </style>
  {% endfor %}
{% endif %}
{% for stylesheet in site.config_stylesheets %}
  <link rel="stylesheet" href="{{ stylesheet }}">
{% endfor %}
{{ doc.body_open }}
{{ doc.body }}
{{ doc.body_close }}"#
            .to_string(),
        partials: Vec::new(),
        styles: Vec::new(),
        scripts: Vec::new(),
        assets: Vec::new(),
        is_default: false,
    };
    entry.append_styles(styles);
    entry
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ThemeManifest {
    shared: SharedImports,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SharedImports {
    partials: Vec<String>,
    css: Vec<String>,
    js: Vec<String>,
    assets: Vec<String>,
}

/// Resolve the layout for `scope`. `None` means raw Typst output.
/// Fallback: if the selected theme lacks the entry file, use the builtin
/// default bundle's entry with the default's assets. One level, no chains.
pub fn resolve_html_entry(
    selection: &ThemeSelection,
    scope: HtmlScope,
) -> Result<Option<HtmlEntry>> {
    let entry = scope.entry_file();
    match selection {
        ThemeSelection::Typst => Ok(None),
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
        ThemeSelection::Typst => Err(anyhow!(
            "page layout `{layout_path}` requires an HTML theme, but theme is `typst`"
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
        styles: bundle_assets(bundle, &manifest.shared.css, "css/", "css")?,
        scripts: bundle_assets(bundle, &manifest.shared.js, "js/", "js")?,
        assets: bundle_assets_any(bundle, &manifest.shared.assets, "assets/")?,
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
        styles: dir_assets(dir, &manifest.shared.css, "css", "css")?,
        scripts: dir_assets(dir, &manifest.shared.js, "js", "js")?,
        assets: dir_assets_any(dir, &manifest.shared.assets, "assets")?,
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

fn collect_shared_assets(
    local_files: Vec<(String, String)>,
    imports: &[String],
    label: &str,
    extension: Option<&str>,
    mut resolve_import: impl FnMut(&str) -> Result<Option<String>>,
) -> Result<Vec<(String, String)>> {
    let mut imported = std::collections::BTreeSet::new();
    let mut files = Vec::new();

    for name in imports {
        validate_shared_import(name, extension)?;
        if !imported.insert(name.clone()) {
            return Err(anyhow!("shared import `{name}` is listed more than once"));
        }

        if let Some((_, source)) = local_files.iter().find(|(file, _)| file == name) {
            files.push((name.clone(), source.clone()));
            continue;
        }

        let source = resolve_import(name)?
            .ok_or_else(|| anyhow!("shared {label} import `{name}` was not found"))?;
        files.push((name.clone(), source));
    }

    files.extend(
        local_files
            .into_iter()
            .filter(|(name, _)| !imported.contains(name)),
    );
    Ok(files)
}

fn bundle_assets(
    bundle: &BundleDef,
    imports: &[String],
    prefix: &str,
    ext: &str,
) -> Result<Vec<(String, String)>> {
    let local_files = bundle_files(bundle, prefix, Some(ext));
    collect_shared_assets(local_files, imports, ext, Some(ext), |name| {
        let path = format!("{prefix}{name}");
        Ok(bundle
            .file(&path)
            .or_else(|| shared_file(&path))
            .map(str::to_string))
    })
}

fn bundle_assets_any(
    bundle: &BundleDef,
    imports: &[String],
    prefix: &str,
) -> Result<Vec<(String, String)>> {
    let local_files = bundle_files(bundle, prefix, None);
    collect_shared_assets(local_files, imports, "asset", None, |name| {
        let path = format!("{prefix}{name}");
        Ok(bundle
            .file(&path)
            .or_else(|| shared_file(&path))
            .map(str::to_string))
    })
}

fn bundle_files(bundle: &BundleDef, prefix: &str, ext: Option<&str>) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = bundle
        .files
        .iter()
        .filter(|file| {
            if !file.path.starts_with(prefix) {
                return false;
            }
            match ext {
                Some(ext) => file.path.ends_with(&format!(".{ext}")),
                None => true,
            }
        })
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
    collect_shared_assets(
        local_files,
        imports,
        ext,
        Some(ext),
        |name| {
            let path = format!("{subdir}/{name}");
            Ok(dir_shared_file(dir, &path)?.or_else(|| shared_file(&path).map(str::to_string)))
        },
    )
}

fn dir_assets_any(dir: &Path, imports: &[String], subdir: &str) -> Result<Vec<(String, String)>> {
    let local_files = read_theme_files_any(&dir.join(subdir))?;
    collect_shared_assets(local_files, imports, "asset", None, |name| {
        let path = format!("{subdir}/{name}");
        Ok(dir_shared_file(dir, &path)?.or_else(|| shared_file(&path).map(str::to_string)))
    })
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

fn validate_shared_import(name: &str, ext: Option<&str>) -> Result<()> {
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
    if let Some(ext) = ext {
        if path.extension().and_then(|extension| extension.to_str()) != Some(ext) {
            return Err(anyhow!("shared import `{name}` must be a .{ext} file"));
        }
    }
    Ok(())
}
