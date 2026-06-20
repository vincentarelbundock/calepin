use std::io::{ErrorKind, Write};
use std::path::Path;

use anyhow::{Context, Result};

pub(crate) fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if parent.as_os_str().is_empty() {
            return Ok(());
        }
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    Ok(())
}

pub(crate) fn write_if_changed(path: &Path, bytes: impl AsRef<[u8]>) -> Result<()> {
    let bytes = bytes.as_ref();
    ensure_parent(path)?;

    match std::fs::read(path) {
        Ok(existing) if existing == bytes => return Ok(()),
        Ok(_) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read existing {}", path.display()));
        }
    }

    atomic_write(path, bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let temp_dir = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(temp_dir)
        .with_context(|| format!("failed to create temporary file in {}", temp_dir.display()))?;
    temp.write_all(bytes)
        .with_context(|| format!("failed to write temporary file for {}", path.display()))?;
    temp.as_file_mut()
        .sync_all()
        .with_context(|| format!("failed to flush temporary file for {}", path.display()))?;
    temp.persist(path)
        .map(|_| ())
        .map_err(|err| err.error)
        .with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_parent_ignores_parentless_relative_paths() {
        ensure_parent(Path::new("output.typ")).unwrap();
    }

    #[test]
    fn write_if_changed_creates_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("result.json");

        write_if_changed(&path, br#"{"ok":true}"#).unwrap();

        assert_eq!(std::fs::read_to_string(path).unwrap(), r#"{"ok":true}"#);
    }

    #[test]
    fn write_if_changed_skips_unchanged_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("result.json");
        std::fs::write(&path, "same").unwrap();
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();

        write_if_changed(&path, "same").unwrap();

        let after = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(after, before);
    }

    #[test]
    fn write_if_changed_updates_changed_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("result.json");
        std::fs::write(&path, "old").unwrap();

        write_if_changed(&path, "new").unwrap();

        assert_eq!(std::fs::read_to_string(path).unwrap(), "new");
    }

    #[test]
    fn write_if_changed_reports_existing_read_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("target");
        std::fs::create_dir(&path).unwrap();

        let err = write_if_changed(&path, "new").unwrap_err().to_string();

        assert!(err.contains("failed to read existing"), "{err}");
        assert!(err.contains("target"), "{err}");
    }
}
