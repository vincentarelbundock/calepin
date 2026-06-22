use std::path::Path;

use super::bundle::{require_builtin, shared_file, BundleDef};
use super::{
    dir_theme_name, read_local_theme_manifest, read_theme_files, read_theme_files_any,
    resolve_theme_chain, ThemeLayer, ThemeSelection, DEFAULT_THEME_NAME,
};
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

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

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ThemeManifest {
    #[allow(dead_code)]
    extends: Option<String>,
    shared: SharedImports,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SharedImports {
    partials: Vec<String>,
    styles: Vec<String>,
    scripts: Vec<String>,
    css: Vec<String>,
    js: Vec<String>,
    assets: Vec<String>,
}

/// Resolve the layout for `scope`. `None` means raw Typst output.
pub fn resolve_html_entry(
    selection: &ThemeSelection,
    scope: HtmlScope,
) -> Result<Option<HtmlEntry>> {
    let entry = scope.entry_file();
    let chain = resolve_theme_chain(selection)?;
    chain_html_entry(&chain.layers, entry, chain.terminal_typst)
}

/// Resolve a website layout at an explicit theme-relative path.
pub fn resolve_explicit_site_html_entry(
    selection: &ThemeSelection,
    layout_path: &str,
) -> Result<Option<HtmlEntry>> {
    validate_explicit_html_layout_path(layout_path)?;
    let chain = resolve_theme_chain(selection)?;
    if chain.layers.is_empty() && chain.terminal_typst {
        return Err(anyhow!(
            "page layout `{layout_path}` requires an HTML theme, but theme is `typst`"
        ));
    }
    chain_html_entry(&chain.layers, layout_path, chain.terminal_typst)
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

fn chain_html_entry(
    layers: &[ThemeLayer],
    entry: &str,
    terminal_typst: bool,
) -> Result<Option<HtmlEntry>> {
    if layers.is_empty() && terminal_typst {
        return Ok(None);
    }
    let Some((_layout_index, layout)) = find_chain_layout(layers, entry)? else {
        let theme = layers
            .last()
            .map(layer_name)
            .unwrap_or_else(|| "typst".to_string());
        return Err(anyhow!(
            "theme `{theme}` does not contain page layout `{entry}`"
        ));
    };
    let mut partials = Vec::new();
    let mut styles = Vec::new();
    let mut scripts = Vec::new();
    let mut assets = Vec::new();
    for layer in layers {
        merge_named_assets(&mut partials, layer_partials(layer)?);
        merge_named_assets(&mut styles, layer_styles(layer)?);
        merge_named_assets(&mut scripts, layer_scripts(layer)?);
        merge_named_assets(&mut assets, layer_assets_any(layer)?);
    }
    Ok(Some(HtmlEntry {
        theme_name: layer_name(layers.last().unwrap()),
        layout,
        partials,
        styles,
        scripts,
        assets,
        is_default: matches!(layers.last(), Some(ThemeLayer::Builtin(DEFAULT_THEME_NAME))),
    }))
}

fn find_chain_layout(layers: &[ThemeLayer], entry: &str) -> Result<Option<(usize, String)>> {
    for (index, layer) in layers.iter().enumerate().rev() {
        match layer {
            ThemeLayer::Builtin(name) => {
                let bundle = require_builtin(name)?;
                if let Some(source) = bundle.file(entry) {
                    return Ok(Some((index, source.to_string())));
                }
            }
            ThemeLayer::Dir(dir) => {
                let path = dir.join(entry);
                if path.is_file() {
                    let source = std::fs::read_to_string(&path)
                        .with_context(|| format!("failed to read {}", path.display()))?;
                    return Ok(Some((index, source)));
                }
            }
        }
    }
    Ok(None)
}

fn layer_name(layer: &ThemeLayer) -> String {
    match layer {
        ThemeLayer::Builtin(name) => (*name).to_string(),
        ThemeLayer::Dir(dir) => dir_theme_name(dir),
    }
}

fn merge_named_assets(target: &mut Vec<(String, String)>, incoming: Vec<(String, String)>) {
    for (name, source) in incoming {
        if let Some((_, existing_source)) =
            target.iter_mut().find(|(existing, _)| existing == &name)
        {
            *existing_source = source;
        } else {
            target.push((name, source));
        }
    }
}

fn layer_partials(layer: &ThemeLayer) -> Result<Vec<(String, String)>> {
    match layer {
        ThemeLayer::Builtin(name) => {
            let bundle = require_builtin(name)?;
            let manifest = bundle_manifest(bundle)?;
            Ok(
                bundle_assets(bundle, &manifest.shared.partials, "partials/", "html")?
                    .into_iter()
                    .map(|(name, source)| (format!("partials/{name}"), source))
                    .collect(),
            )
        }
        ThemeLayer::Dir(dir) => {
            let manifest = read_local_theme_manifest(dir)?;
            Ok(
                dir_assets(dir, &manifest.shared.partials, "partials", "html")?
                    .into_iter()
                    .map(|(name, source)| (format!("partials/{name}"), source))
                    .collect(),
            )
        }
    }
}

fn style_imports(shared: &SharedImports) -> Vec<String> {
    shared
        .css
        .iter()
        .chain(shared.styles.iter())
        .cloned()
        .collect()
}

fn script_imports(shared: &SharedImports) -> Vec<String> {
    shared
        .js
        .iter()
        .chain(shared.scripts.iter())
        .cloned()
        .collect()
}

fn local_style_imports(shared: &super::LocalThemeSharedImports) -> Vec<String> {
    shared
        .css
        .iter()
        .chain(shared.styles.iter())
        .cloned()
        .collect()
}

fn local_script_imports(shared: &super::LocalThemeSharedImports) -> Vec<String> {
    shared
        .js
        .iter()
        .chain(shared.scripts.iter())
        .cloned()
        .collect()
}

fn layer_styles(layer: &ThemeLayer) -> Result<Vec<(String, String)>> {
    match layer {
        ThemeLayer::Builtin(name) => {
            let bundle = require_builtin(name)?;
            let manifest = bundle_manifest(bundle)?;
            bundle_assets(bundle, &style_imports(&manifest.shared), "css/", "css")
        }
        ThemeLayer::Dir(dir) => {
            let manifest = read_local_theme_manifest(dir)?;
            dir_assets_flexible(
                dir,
                &local_style_imports(&manifest.shared),
                &["styles", "css"],
                "css",
            )
        }
    }
}

fn layer_scripts(layer: &ThemeLayer) -> Result<Vec<(String, String)>> {
    match layer {
        ThemeLayer::Builtin(name) => {
            let bundle = require_builtin(name)?;
            let manifest = bundle_manifest(bundle)?;
            bundle_assets(bundle, &script_imports(&manifest.shared), "js/", "js")
        }
        ThemeLayer::Dir(dir) => {
            let manifest = read_local_theme_manifest(dir)?;
            dir_assets_flexible(
                dir,
                &local_script_imports(&manifest.shared),
                &["scripts", "js"],
                "js",
            )
        }
    }
}

fn layer_assets_any(layer: &ThemeLayer) -> Result<Vec<(String, String)>> {
    match layer {
        ThemeLayer::Builtin(name) => {
            let bundle = require_builtin(name)?;
            let manifest = bundle_manifest(bundle)?;
            bundle_assets_any(bundle, &manifest.shared.assets, "assets/")
        }
        ThemeLayer::Dir(dir) => {
            let manifest = read_local_theme_manifest(dir)?;
            dir_assets_any(dir, &manifest.shared.assets, "assets")
        }
    }
}

fn bundle_manifest(bundle: &BundleDef) -> Result<ThemeManifest> {
    let Some(source) = bundle.file("theme.toml") else {
        return Ok(ThemeManifest::default());
    };
    toml::from_str(source)
        .with_context(|| format!("failed to parse builtin theme `{}` theme.toml", bundle.name))
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

fn dir_assets_flexible(
    dir: &Path,
    imports: &[String],
    subdirs: &[&str],
    ext: &str,
) -> Result<Vec<(String, String)>> {
    let mut local_files = Vec::new();
    for subdir in subdirs {
        merge_named_assets(&mut local_files, read_theme_files(&dir.join(subdir), ext)?);
    }
    collect_shared_assets(local_files, imports, ext, Some(ext), |name| {
        for subdir in subdirs {
            let path = format!("{subdir}/{name}");
            if let Some(source) = dir_shared_file(dir, &path)? {
                return Ok(Some(source));
            }
            if let Some(source) = shared_file(&path).map(str::to_string) {
                return Ok(Some(source));
            }
        }
        Ok(None)
    })
}

fn dir_assets(
    dir: &Path,
    imports: &[String],
    subdir: &str,
    ext: &str,
) -> Result<Vec<(String, String)>> {
    let local_files = read_theme_files(&dir.join(subdir), ext)?;
    collect_shared_assets(local_files, imports, ext, Some(ext), |name| {
        let path = format!("{subdir}/{name}");
        Ok(dir_shared_file(dir, &path)?.or_else(|| shared_file(&path).map(str::to_string)))
    })
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
