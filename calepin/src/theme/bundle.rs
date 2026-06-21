use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

pub(crate) struct BundleFile {
    pub(crate) path: &'static str,
    pub(crate) source: &'static str,
}

pub(crate) struct BundleDef {
    pub(crate) name: &'static str,
    pub(crate) files: &'static [BundleFile],
}

impl BundleDef {
    pub(crate) fn file(&self, path: &str) -> Option<&'static str> {
        self.files
            .iter()
            .find(|file| file.path == path)
            .map(|file| file.source)
    }

    pub(crate) fn has_file(&self, path: &str) -> bool {
        self.file(path).is_some()
    }
}

include!(concat!(env!("OUT_DIR"), "/theme_assets.rs"));

pub(super) static CALEPIN: BundleDef = BundleDef {
    name: "calepin",
    files: CALEPIN_FILES,
};

static ACADEMIC: BundleDef = BundleDef {
    name: "academic",
    files: ACADEMIC_FILES,
};

static BUILTINS: [&BundleDef; 2] = [&CALEPIN, &ACADEMIC];

pub(crate) fn shared_file(path: &str) -> Option<&'static str> {
    SHARED_FILES
        .iter()
        .find(|file| file.path == path)
        .map(|file| file.source)
}

pub fn builtin_names() -> Vec<&'static str> {
    BUILTINS.iter().map(|bundle| bundle.name).collect()
}

pub(crate) fn builtin_bundle(name: &str) -> Option<&'static BundleDef> {
    BUILTINS.iter().copied().find(|bundle| bundle.name == name)
}

pub(super) fn require_builtin(name: &str) -> Result<&'static BundleDef> {
    builtin_bundle(name).ok_or_else(|| {
        anyhow!(
            "unknown theme `{name}`; use one of {}",
            builtin_names().join(", ")
        )
    })
}

/// Copy a builtin bundle's files into `themes_dir/<name>/`, refusing to touch
/// an existing destination unless `force`.
#[cfg(test)]
pub(super) fn eject_builtin(name: &str, themes_dir: &Path, force: bool) -> Result<PathBuf> {
    let bundle = require_builtin(name)?;
    let dest = themes_dir.join(bundle.name);
    eject_builtin_to(name, &dest, force)
}

/// Copy a builtin bundle's files into `dest`, refusing to touch an existing
/// destination unless `force`.
pub fn eject_builtin_to(name: &str, dest: &Path, force: bool) -> Result<PathBuf> {
    let bundle = require_builtin(name)?;
    if dest.exists() && !force {
        return Err(anyhow!(
            "{} already exists; pass --force to overwrite",
            dest.display()
        ));
    }
    for file in bundle.files {
        write_theme_file(dest, file.path, file.source)?;
    }
    write_resolved_shared_files(bundle, dest)?;
    Ok(dest.to_path_buf())
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

fn write_resolved_shared_files(bundle: &BundleDef, dest: &Path) -> Result<()> {
    let manifest = bundle_manifest(bundle)?;
    for name in manifest.shared.partials {
        write_resolved_shared_file(bundle, dest, "partials", &name, "html")?;
    }
    for name in manifest.shared.styles {
        write_resolved_shared_file(bundle, dest, "styles", &name, "css")?;
    }
    for name in manifest.shared.scripts {
        write_resolved_shared_file(bundle, dest, "scripts", &name, "js")?;
    }
    Ok(())
}

fn bundle_manifest(bundle: &BundleDef) -> Result<ThemeManifest> {
    let Some(source) = bundle.file("theme.toml") else {
        return Ok(ThemeManifest::default());
    };
    toml::from_str(source)
        .with_context(|| format!("failed to parse builtin theme `{}` theme.toml", bundle.name))
}

fn write_resolved_shared_file(
    bundle: &BundleDef,
    dest: &Path,
    subdir: &str,
    name: &str,
    ext: &str,
) -> Result<()> {
    validate_shared_import(name, ext)?;
    let relative = format!("{subdir}/{name}");
    if bundle.has_file(&relative) {
        return Ok(());
    }
    let source = shared_file(&relative)
        .ok_or_else(|| anyhow!("shared {ext} import `{name}` was not found"))?;
    write_theme_file(dest, &relative, source)
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

fn write_theme_file(dest: &Path, relative: &str, source: &str) -> Result<()> {
    let path = dest.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(&path, source).with_context(|| format!("failed to write {}", path.display()))
}
