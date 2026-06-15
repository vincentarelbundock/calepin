use anyhow::anyhow;
use std::io;
use std::path::{Path, PathBuf};

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
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
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

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = path.metadata().unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    #[cfg(not(unix))]
    fn make_executable(_path: &Path) {}
}
