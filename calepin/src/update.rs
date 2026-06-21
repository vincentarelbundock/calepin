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
    let status = Command::new(&updater)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("failed to run {}", updater.display()))?;

    std::process::exit(status.code().unwrap_or(1));
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
