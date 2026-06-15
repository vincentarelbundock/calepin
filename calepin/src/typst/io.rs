use std::path::Path;

use anyhow::{Context, Result};

pub fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    Ok(())
}

pub fn write_if_changed(path: &Path, bytes: impl AsRef<[u8]>) -> Result<()> {
    let bytes = bytes.as_ref();
    ensure_parent(path)?;
    if std::fs::read(path).is_ok_and(|existing| existing == bytes) {
        return Ok(());
    }
    std::fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}
