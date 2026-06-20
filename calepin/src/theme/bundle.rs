use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

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

static REVEALJS: BundleDef = BundleDef {
    name: "revealjs",
    files: REVEALJS_FILES,
};

static BUILTINS: [&BundleDef; 3] = [&CALEPIN, &ACADEMIC, &REVEALJS];

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
pub fn eject_builtin(name: &str, themes_dir: &Path, force: bool) -> Result<PathBuf> {
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
    if let Some(parent) = dest.parent() {
        write_shared_files(&parent.join("shared"), force)?;
    }
    Ok(dest.to_path_buf())
}

fn write_shared_files(dest: &Path, force: bool) -> Result<()> {
    for file in SHARED_FILES {
        let path = dest.join(file.path);
        if path.exists() && !force {
            continue;
        }
        write_theme_file(dest, file.path, file.source)?;
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
