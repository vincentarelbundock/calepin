use anyhow::{anyhow, Context, Result};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::utils::process::is_executable_file;

#[cfg(windows)]
const UPDATER_BINARY: &str = "calepin-update.exe";
#[cfg(not(windows))]
const UPDATER_BINARY: &str = "calepin-update";

pub fn handle_update() -> Result<()> {
    let updater = find_updater().ok_or_else(missing_updater_error)?;
    // On Windows the installer run by calepin-update cannot overwrite
    // calepin.exe while this process waits for it below: the loader keeps the
    // image file locked for writing as long as the process lives. Renaming a
    // running executable is allowed, though, so move our own file aside and
    // let the installer write a fresh calepin.exe at the usual path. The
    // updater does the same for its own binary.
    let moved_aside = move_running_exe_aside();

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
/// this works around. Returns the aside and original paths.
#[cfg(windows)]
fn move_running_exe_aside() -> Option<(PathBuf, PathBuf)> {
    let original = std::env::current_exe().ok()?;
    let mut aside = original.as_os_str().to_os_string();
    // Same suffix self-replace uses, so a stale copy is recognizable.
    aside.push(".previous.exe");
    let aside = PathBuf::from(aside);
    // Replaces a stale leftover from an earlier interrupted update, if any.
    std::fs::rename(&original, &aside).ok()?;
    Some((aside, original))
}

/// The update did not happen: put the old binary back.
#[cfg(windows)]
fn restore_moved_aside_exe(moved: Option<(PathBuf, PathBuf)>) {
    if let Some((aside, original)) = moved {
        let _ = std::fs::rename(&aside, &original);
    }
}

/// The update succeeded: drop the old binary. std::fs cannot delete the
/// image of a live process, which this still is, so this goes through
/// self-replace. A leftover is harmless and replaced by the next update.
#[cfg(windows)]
fn discard_moved_aside_exe(moved: Option<(PathBuf, PathBuf)>) {
    if let Some((aside, _)) = moved {
        if self_replace::self_delete_at(&aside).is_err() {
            cwarn!(
                "could not remove the previous binary at {}",
                aside.display()
            );
        }
    }
}

#[cfg(not(windows))]
fn move_running_exe_aside() -> Option<(PathBuf, PathBuf)> {
    None
}

#[cfg(not(windows))]
fn restore_moved_aside_exe(_moved: Option<(PathBuf, PathBuf)>) {}

#[cfg(not(windows))]
fn discard_moved_aside_exe(_moved: Option<(PathBuf, PathBuf)>) {}

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
