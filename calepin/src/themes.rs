//! Asset bundling for project scaffolding.
//!
//! Provides built-in assets (CSS, fonts, icons) for new projects.
//! Partials are no longer bundled -- they come from built-in defaults
//! via layered resolution.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::paths::ProjectKind;

pub use crate::render::elements::BUILTIN_ASSETS;

/// Copy built-in assets (CSS, fonts, icons) into a project's `_calepin/` directory.
pub fn copy_builtin_assets(kind: &ProjectKind) -> anyhow::Result<()> {
    let mut files = BTreeMap::new();
    if let Some(assets_dir) = BUILTIN_ASSETS.get_dir("assets") {
        collect_embedded_dir(assets_dir, Path::new("assets"), Path::new("assets"), &mut files);
    }

    let base = kind.calepin_dir();
    for (rel, content) in &files {
        let target = base.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, content)?;
    }
    Ok(())
}

fn collect_embedded_dir(
    dir: &include_dir::Dir<'static>,
    new_prefix: &Path,
    strip: &Path,
    out: &mut BTreeMap<PathBuf, Vec<u8>>,
) {
    for file in dir.files() {
        let rel = file.path().strip_prefix(strip).unwrap_or(file.path());
        out.insert(new_prefix.join(rel), file.contents().to_vec());
    }
    for subdir in dir.dirs() {
        collect_embedded_dir(subdir, new_prefix, strip, out);
    }
}
