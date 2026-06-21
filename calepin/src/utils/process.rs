use anyhow::anyhow;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use crate::utils::path::is_path_like;
use crate::utils::tools::Tool;

pub fn validate_executable(
    program: &Path,
    action: &str,
    tool: Option<&Tool>,
) -> anyhow::Result<()> {
    if executable_exists(program) {
        Ok(())
    } else {
        Err(missing_executable_error(program, action, tool))
    }
}

pub fn validate_python_interpreter(
    program: &Path,
    action: &str,
    tool: Option<&Tool>,
) -> anyhow::Result<()> {
    validate_executable(program, action, tool)?;
    let output = Command::new(program)
        .args(["-s", "-c", "import sys; print(sys.executable)"])
        .stdin(Stdio::null())
        .output()
        .map_err(|error| spawn_error(program, action, error, tool))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(unusable_python_error(program, action, &output, tool))
    }
}

#[cfg(windows)]
pub fn python_interpreter_is_usable(program: &Path) -> bool {
    validate_python_interpreter(program, "check Python interpreter", None).is_ok()
}

pub fn spawn_error(
    program: &Path,
    action: &str,
    error: io::Error,
    tool: Option<&Tool>,
) -> anyhow::Error {
    if error.kind() == io::ErrorKind::NotFound {
        missing_executable_error(program, action, tool)
    } else {
        anyhow!(
            "failed to {action} with executable {}: {}",
            program.display(),
            error
        )
    }
}

fn missing_executable_error(program: &Path, action: &str, tool: Option<&Tool>) -> anyhow::Error {
    let hint = tool
        .map(|tool| format!(" {}", tool.install_hint))
        .unwrap_or_default();
    let configured = program.display();
    if is_path_like(program) {
        anyhow!("executable not found while trying to {action}: {configured}")
    } else {
        anyhow!("executable `{configured}` not found on PATH while trying to {action}.{hint}")
    }
}

fn unusable_python_error(
    program: &Path,
    action: &str,
    output: &Output,
    tool: Option<&Tool>,
) -> anyhow::Error {
    let details = command_output(output);
    let hint = tool
        .map(|tool| format!(" {}", tool.install_hint))
        .unwrap_or_default();
    if looks_like_windows_python_store_alias(&details) {
        return anyhow!(
            "Python executable {} is a Windows App execution alias, not a usable Python interpreter, while trying to {action}: {} Disable the Python App execution aliases in Windows Settings or set `[executables] python` to the full path of python.exe.",
            program.display(),
            details
        );
    }
    anyhow!(
        "Python executable {} failed while trying to {action}: {}{}",
        program.display(),
        details,
        hint
    )
}

fn command_output(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => format!("exit status {}", output.status),
        (false, true) => stdout,
        (true, false) => stderr,
        (false, false) => format!("{stdout}\n{stderr}"),
    }
}

fn looks_like_windows_python_store_alias(output: &str) -> bool {
    output.contains("Python was not found")
        && output.contains("Microsoft Store")
        && output.contains("App execution aliases")
}

fn executable_exists(program: &Path) -> bool {
    if is_path_like(program) {
        return is_executable_file(program);
    }
    std::env::var_os("PATH")
        .map(|path| {
            std::env::split_paths(&path).any(|dir| {
                path_candidates(&dir, program).any(|candidate| is_executable_file(&candidate))
            })
        })
        .unwrap_or(false)
}

fn path_candidates<'a>(dir: &'a Path, program: &'a Path) -> impl Iterator<Item = PathBuf> + 'a {
    let direct = std::iter::once(dir.join(program));
    #[cfg(windows)]
    {
        let has_extension = program.extension().is_some();
        let pathext = std::env::var_os("PATHEXT")
            .map(|value| {
                value
                    .to_string_lossy()
                    .split(';')
                    .filter(|extension| !extension.is_empty())
                    .map(|extension| {
                        let extension = extension.trim_start_matches('.');
                        dir.join(program).with_extension(extension)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                ["COM", "EXE", "BAT", "CMD"]
                    .into_iter()
                    .map(|extension| dir.join(program).with_extension(extension))
                    .collect()
            });
        return direct.chain((!has_extension).then_some(pathext).into_iter().flatten());
    }
    #[cfg(not(windows))]
    {
        direct
    }
}

#[cfg(unix)]
pub(crate) fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
pub(crate) fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testutil::{env_lock, EnvVarGuard};
    use std::path::PathBuf;

    #[test]
    fn missing_configured_path_names_the_path() {
        let error = spawn_error(
            &PathBuf::from("/tmp/missing-tool"),
            "run external tool",
            io::Error::from(io::ErrorKind::NotFound),
            None,
        )
        .to_string();

        assert!(error.contains("executable not found"));
        assert!(error.contains("/tmp/missing-tool"));
        assert!(error.contains("run external tool"));
    }

    #[test]
    fn missing_bare_command_names_path_lookup() {
        let error = spawn_error(
            Path::new("tool"),
            "run external tool",
            io::Error::from(io::ErrorKind::NotFound),
            None,
        )
        .to_string();

        assert!(error.contains("`tool` not found on PATH"));
        assert!(error.contains("run external tool"));
    }

    #[test]
    fn validate_accepts_executable_path() {
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("tool");
        std::fs::write(&executable, "").unwrap();
        make_executable(&executable);

        validate_executable(&executable, "run external tool", None).unwrap();
    }

    #[test]
    fn validate_searches_path_for_bare_command() {
        let _env_lock = env_lock();
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("tool");
        std::fs::write(&executable, "").unwrap();
        make_executable(&executable);
        let _path = EnvVarGuard::set("PATH", std::env::join_paths([dir.path()]).unwrap());

        validate_executable(Path::new("tool"), "run external tool", None).unwrap();
    }

    #[test]
    fn validate_python_rejects_windows_store_alias_message() {
        let dir = tempfile::tempdir().unwrap();
        let executable = write_script(
            &dir.path().join("python3"),
            "Python was not found; run without arguments to install from the Microsoft Store, or disable this shortcut from Settings Apps Advanced app settings App execution aliases.\n",
            9009,
        );

        let error =
            validate_python_interpreter(&executable, "execute Python chunks", None).unwrap_err();
        let message = error.to_string();

        assert!(
            message.contains("Microsoft Store") || message.contains("App execution aliases"),
            "{message}"
        );
        assert!(
            message.contains(&executable.display().to_string()),
            "{message}"
        );
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = path.metadata().unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    #[cfg(not(unix))]
    fn make_executable(_path: &Path) {}

    #[cfg(unix)]
    fn write_script(path: &Path, message: &str, exit_code: i32) -> PathBuf {
        std::fs::write(
            path,
            format!("#!/bin/sh\nprintf '%s' {:?}\nexit {}\n", message, exit_code),
        )
        .unwrap();
        make_executable(path);
        path.to_path_buf()
    }

    #[cfg(windows)]
    fn write_script(path: &Path, message: &str, exit_code: i32) -> PathBuf {
        let path = path.with_extension("cmd");
        std::fs::write(
            &path,
            format!("@echo off\necho {message}\nexit /B {exit_code}\n"),
        )
        .unwrap();
        path
    }
}
