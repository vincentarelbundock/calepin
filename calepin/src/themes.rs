//! Asset bundling for project scaffolding.
//!
//! Provides built-in assets (CSS, fonts, icons) for new projects.
//! Templates are no longer bundled -- they come from built-in defaults
//! via layered resolution.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::paths::ProjectKind;
use crate::render::elements::BUILTIN_EXTENSIONS;

/// Copy built-in assets (CSS, fonts, icons) into a project's sidecar directory.
pub fn copy_builtin_assets(kind: &ProjectKind) -> anyhow::Result<()> {
    let assets_dir = BUILTIN_EXTENSIONS.get_dir("html/assets")
        .ok_or_else(|| anyhow::anyhow!("Built-in html/assets not found"))?;

    let mut files = BTreeMap::new();
    let strip = Path::new("html/assets");
    collect_embedded_dir(assets_dir, Path::new("assets"), strip, &mut files);

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
