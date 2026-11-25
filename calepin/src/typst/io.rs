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

/// Prefix for the temporary file `atomic_write` renames onto `path`: the
/// target's own name, kept hidden (`.{name}.`). The default `.tmpXXXXXX` names
/// are anonymous, so watchers that skip generated outputs by name (entry
/// files, for instance) would mistake the short-lived temporary for a source
/// change and rebuild (#131).
fn temp_file_prefix(path: &Path) -> String {
    let mut name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    // Bound the prefix so the temporary name (prefix, a dot, six random
    // characters, `.tmp`) stays within the usual 255-byte component limit even
    // for a target whose own name is close to it. Truncation keeps the start
    // of the name, so a temporary for an entry file still starts with the
    // entry-file prefix.
    const MAX_NAME_BYTES: usize = 200;
    if name.len() > MAX_NAME_BYTES {
        let mut cut = MAX_NAME_BYTES;
        while !name.is_char_boundary(cut) {
            cut -= 1;
        }
        name.truncate(cut);
    }
    if name.starts_with('.') {
        format!("{name}.")
    } else {
        format!(".{name}.")
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let temp_dir = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temp = tempfile::Builder::new()
        .prefix(&temp_file_prefix(path))
        .suffix(".tmp")
        .tempfile_in(temp_dir)
        .with_context(|| format!("failed to create temporary file in {}", temp_dir.display()))?;
    temp.write_all(bytes)
        .with_context(|| format!("failed to write temporary file for {}", path.display()))?;
    temp.as_file_mut()
        .sync_all()
        .with_context(|| format!("failed to flush temporary file for {}", path.display()))?;
    temp.persist(path)
        .map(|_| ())
        .map_err(|err| err.error)
        .with_context(|| format!("failed to write {}", path.display()))?;
    apply_default_file_mode(path)
}

/// `NamedTempFile` creates its backing file with mode 0600 so it persists
/// with the same restrictive permissions, which leaves artifacts like
/// `results.json` and published figures readable only by their owner.
/// Restore the mode a plain `File::create` would have gotten: `0o666` minus
/// the process umask.
#[cfg(unix)]
fn apply_default_file_mode(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mode = 0o666 & !process_umask();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .with_context(|| format!("failed to set permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn apply_default_file_mode(_path: &Path) -> Result<()> {
    Ok(())
}

/// The process umask, read by setting it and immediately restoring it.
/// `umask` has no read-only query form; this is the standard trick, and it
/// is a plain process-wide value, not a per-thread one, so the brief window
/// where it is 0 only matters if another thread creates a file at the exact
/// same instant.
#[cfg(unix)]
fn process_umask() -> u32 {
    use std::sync::OnceLock;
    static UMASK: OnceLock<u32> = OnceLock::new();
    *UMASK.get_or_init(|| unsafe {
        let mask = libc::umask(0);
        libc::umask(mask);
        mask as u32
    })
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

    #[cfg(unix)]
    #[test]
    fn write_if_changed_leaves_files_readable_by_others() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("results.json");

        write_if_changed(&path, br#"{"ok":true}"#).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        let expected = 0o666 & !process_umask();
        assert_eq!(mode, expected);
    }

    #[test]
    fn temp_files_beside_entry_files_are_recognized_as_generated() {
        // Watchers skip generated entry files by name; the temporary that
        // `atomic_write` renames into place must be skipped the same way.
        let entry = format!("{}post.source.typ", crate::typst::paths::ENTRY_FILE_PREFIX);
        let temp = format!("{}Ab12Cd.tmp", temp_file_prefix(Path::new(&entry)));
        assert!(
            crate::typst::paths::is_generated_entry_file(Path::new(&temp)),
            "{temp}"
        );
    }

    #[test]
    fn temp_files_are_hidden() {
        for target in ["results.json", ".hidden", "dir/page.html"] {
            let prefix = temp_file_prefix(Path::new(target));
            assert!(prefix.starts_with('.'), "{target}: {prefix}");
            assert!(!prefix.starts_with(".."), "{target}: {prefix}");
        }
    }

    #[test]
    fn atomic_write_accepts_target_names_near_the_filesystem_limit() {
        let dir = tempfile::tempdir().unwrap();
        // 250 bytes fits a 255-byte component limit on its own; the temporary
        // staged beside it must fit too.
        let entry_prefix = crate::typst::paths::ENTRY_FILE_PREFIX;
        let name = format!("{entry_prefix}{}.typ", "é".repeat(115));
        assert!(name.len() <= 255, "{}", name.len());
        let target = dir.path().join(&name);

        write_if_changed(&target, "content").unwrap();

        assert_eq!(std::fs::read_to_string(&target).unwrap(), "content");
        let temp = format!("{}Ab12Cd.tmp", temp_file_prefix(&target));
        assert!(temp.len() <= 255, "{}", temp.len());
        assert!(crate::typst::paths::is_generated_entry_file(Path::new(
            &temp
        )));
    }

    #[test]
    fn atomic_write_leaves_no_temporary_files_behind() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("results.json");

        write_if_changed(&target, "old").unwrap();
        write_if_changed(&target, "new").unwrap();

        let names = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["results.json".to_string()]);
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
