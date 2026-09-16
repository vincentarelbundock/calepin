use anyhow::{anyhow, Context, Result};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::utils::process::is_executable_file;

#[cfg(windows)]
const UPDATER_BINARY: &str = "calepin-update.exe";
#[cfg(not(windows))]
const UPDATER_BINARY: &str = "calepin-update";

/// The running executable, renamed aside on Windows so the installer can
/// write a fresh copy at the original path. Kept as a small struct (rather
/// than a tuple) so it can be shared with the Ctrl+C handler installed for
/// the duration of the update. Unused on other platforms, where the
/// moved-aside mechanism does not exist at all.
#[cfg(windows)]
#[derive(Debug, Clone)]
struct MovedExe {
    aside: PathBuf,
    original: PathBuf,
}

#[cfg(not(windows))]
type MovedExe = ();

pub fn handle_update() -> Result<()> {
    let updater = find_updater().ok_or_else(missing_updater_error)?;
    // On Windows the installer run by calepin-update cannot overwrite
    // calepin.exe while this process waits for it below: the loader keeps the
    // image file locked for writing as long as the process lives. Renaming a
    // running executable is allowed, though, so move our own file aside and
    // let the installer write a fresh calepin.exe at the usual path. The
    // updater does the same for its own binary.
    let moved_aside = move_running_exe_aside();
    // Without this, interrupting the update lets the process exit through
    // the OS's default Ctrl+C disposition before the code below runs,
    // leaving `calepin.exe` renamed aside with nothing left to restore it
    // on a later run.
    // `MovedExe` is `()` (and therefore `Copy`) off Windows, but a real
    // struct that needs cloning on Windows since `moved_aside` is still
    // used below; the clone is a no-op on other platforms.
    #[allow(clippy::clone_on_copy)]
    install_update_ctrl_c_restore(moved_aside.clone());

    let status = Command::new(&updater)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("failed to run {}", updater.display()));

    let status = match status {
        Ok(status) => status,
        Err(error) => {
            restore_moved_aside_exe(moved_aside);
            return Err(error);
        }
    };

    if status.success() {
        discard_moved_aside_exe(moved_aside);
    } else {
        restore_moved_aside_exe(moved_aside);
    }

    std::process::exit(status.code().unwrap_or(1));
}

/// Rename the running executable to `<name>.previous.exe` so the installer
/// can write a new one at the original path. Best-effort: when the rename
/// fails the update simply proceeds and can still hit the sharing violation
/// this works around.
#[cfg(windows)]
fn move_running_exe_aside() -> Option<MovedExe> {
    let original = std::env::current_exe().ok()?;
    let mut aside = original.as_os_str().to_os_string();
    // Same suffix self-replace uses, so a stale copy is recognizable.
    aside.push(".previous.exe");
    let aside = PathBuf::from(aside);
    // Replaces a stale leftover from an earlier interrupted update, if any.
    std::fs::rename(&original, &aside).ok()?;
    Some(MovedExe { aside, original })
}

/// The update did not happen: put the old binary back.
#[cfg(windows)]
fn restore_moved_aside_exe(moved: Option<MovedExe>) {
    if let Some(moved) = moved {
        let _ = std::fs::rename(&moved.aside, &moved.original);
    }
}

/// The update reported success: verify a fresh `calepin.exe` actually landed
/// at the original path before deleting the parked copy. `axoupdater` can
/// exit 0 without installing anything (already up to date, user declined, or
/// it updated a different install found via `PATH`), in which case discarding
/// the aside copy unconditionally would leave no binary at all. std::fs
/// cannot delete the image of a live process, which the parked copy still
/// is, so removal goes through self-replace.
#[cfg(windows)]
fn discard_moved_aside_exe(moved: Option<MovedExe>) {
    let Some(moved) = moved else {
        return;
    };
    match fresh_binary_was_installed(&moved) {
        Ok(true) => {
            if self_replace::self_delete_at(&moved.aside).is_err() {
                cwarn!(
                    "could not remove the previous binary at {}",
                    moved.aside.display()
                );
            }
        }
        Ok(false) => {
            cwarn!(
                "calepin update reported success but no new binary was found at {}; restoring the previous version",
                moved.original.display()
            );
            restore_moved_aside_exe(Some(moved));
        }
        Err(error) => {
            cwarn!(
                "calepin update reported success but {} could not be verified ({}); restoring the previous version",
                moved.original.display(),
                error
            );
            restore_moved_aside_exe(Some(moved));
        }
    }
}

/// True when a file exists at `moved.original` and it is not simply the
/// parked copy left in place (same size and modification time as
/// `moved.aside`).
#[cfg(windows)]
fn fresh_binary_was_installed(moved: &MovedExe) -> std::io::Result<bool> {
    let new_metadata = std::fs::metadata(&moved.original)?;
    let aside_metadata = std::fs::metadata(&moved.aside)?;
    Ok(new_metadata.len() != aside_metadata.len()
        || new_metadata.modified()? != aside_metadata.modified()?)
}

/// Installs a Ctrl+C handler for the duration of the update that restores
/// the parked binary before the process exits. `calepin update` runs once
/// and exits, so this handler is never uninstalled; it simply stays armed
/// until the process ends, either from `std::process::exit` below on an
/// interrupt or from `handle_update`'s own exit at the end of a normal run.
#[cfg(windows)]
fn install_update_ctrl_c_restore(moved: Option<MovedExe>) -> Option<()> {
    let moved = moved?;
    ctrlc::set_handler(move || {
        restore_moved_aside_exe(Some(moved.clone()));
        std::process::exit(130);
    })
    .ok()?;
    Some(())
}

#[cfg(not(windows))]
fn move_running_exe_aside() -> Option<MovedExe> {
    None
}

#[cfg(not(windows))]
fn restore_moved_aside_exe(_moved: Option<MovedExe>) {}

#[cfg(not(windows))]
fn discard_moved_aside_exe(_moved: Option<MovedExe>) {}

#[cfg(not(windows))]
fn install_update_ctrl_c_restore(_moved: Option<MovedExe>) -> Option<()> {
    None
}

fn find_updater() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|current_exe| sibling_updater(&current_exe))
        .or_else(|| {
            std::env::var_os("PATH")
                .as_deref()
                .and_then(find_updater_on_path)
        })
}

fn sibling_updater(current_exe: &Path) -> Option<PathBuf> {
    let candidate = current_exe.parent()?.join(UPDATER_BINARY);
    is_executable_file(&candidate).then_some(candidate)
}

fn find_updater_on_path(path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|dir| dir.join(UPDATER_BINARY))
        .find(|candidate| is_executable_file(candidate))
}

fn missing_updater_error() -> anyhow::Error {
    anyhow!(
        "calepin-update was not found.\n\n\
         This Calepin installation cannot be updated automatically unless it was installed with \
         the official installer and updater support is present.\n\n\
         To install the official updater, reinstall Calepin with:\n\n  {}\n\n\
         If Calepin is managed by Cargo, Homebrew, or another package manager, update it with \
         that tool instead.",
        official_installer_command()
    )
}

#[cfg(windows)]
fn official_installer_command() -> &'static str {
    r#"powershell -ExecutionPolicy Bypass -c "irm https://github.com/vincentarelbundock/calepin/releases/latest/download/calepin-installer.ps1 | iex""#
}

#[cfg(not(windows))]
fn official_installer_command() -> &'static str {
    "curl --proto '=https' --tlsv1.2 -LsSf https://github.com/vincentarelbundock/calepin/releases/latest/download/calepin-installer.sh | sh"
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::*;

    #[cfg(windows)]
    #[test]
    fn fresh_binary_detects_unchanged_parked_copy() {
        let dir = tempfile::tempdir().unwrap();
        let aside = dir.path().join("calepin.exe.previous.exe");
        let original = dir.path().join("calepin.exe");
        std::fs::write(&aside, b"same bytes").unwrap();
        std::fs::copy(&aside, &original).unwrap();
        // A plain copy carries the same length; force matching mtimes too so
        // this asserts the "not the same file" case rather than relying on
        // filesystem timestamp resolution.
        let aside_time = std::fs::metadata(&aside).unwrap().modified().unwrap();
        // Opened for writing: setting a file's timestamps needs write access on
        // Windows, where a read-only handle fails with "Access is denied".
        let file = std::fs::File::options()
            .write(true)
            .open(&original)
            .unwrap();
        file.set_modified(aside_time).unwrap();

        let moved = MovedExe { aside, original };
        assert!(!fresh_binary_was_installed(&moved).unwrap());
    }

    #[cfg(windows)]
    #[test]
    fn fresh_binary_detects_new_content() {
        let dir = tempfile::tempdir().unwrap();
        let aside = dir.path().join("calepin.exe.previous.exe");
        let original = dir.path().join("calepin.exe");
        std::fs::write(&aside, b"old bytes").unwrap();
        std::fs::write(&original, b"new bytes, different length").unwrap();

        let moved = MovedExe { aside, original };
        assert!(fresh_binary_was_installed(&moved).unwrap());
    }

    #[cfg(windows)]
    #[test]
    fn fresh_binary_missing_original_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let aside = dir.path().join("calepin.exe.previous.exe");
        let original = dir.path().join("calepin.exe");
        std::fs::write(&aside, b"old bytes").unwrap();

        let moved = MovedExe { aside, original };
        assert!(fresh_binary_was_installed(&moved).is_err());
    }
}
