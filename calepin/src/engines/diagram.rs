// Diagram engines: stateless CLI tools that convert source code to SVG.

mod d2;
mod dot;
mod mermaid;
mod tikz;

use anyhow::{Context, Result};
use std::borrow::Cow;
use std::ffi::OsString;
use std::path::Path;
use std::process::Output;

use crate::config::ExecutablePaths;
use crate::engines::EngineResult;
use crate::typst::model::EngineName;
use crate::utils::tools::{self, Tool};

type PrepareSourceFn = for<'code> fn(&'code str) -> Cow<'code, str>;
type RenderFn = for<'a> fn(&DiagramRun<'a>, &mut Vec<EngineResult>) -> Result<bool>;

struct DiagramSpec {
    input_ext: &'static str,
    prepare_source: PrepareSourceFn,
    render: RenderFn,
}

pub(super) struct DiagramRun<'a> {
    pub(super) input_path: &'a Path,
    pub(super) fig_path: &'a Path,
    pub(super) work_dir: &'a Path,
    pub(super) executables: &'a ExecutablePaths,
}

pub fn execute_diagram(
    code: &str,
    engine: EngineName,
    fig_path: &Path,
    source: &[String],
    executables: &ExecutablePaths,
) -> Result<Vec<EngineResult>> {
    let mut results = vec![EngineResult::Source(source.to_vec())];
    let spec = diagram_spec(engine)
        .ok_or_else(|| anyhow::anyhow!("unsupported diagram engine `{}`", engine))?;

    if let Some(parent) = fig_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let temp_dir = tempfile::Builder::new()
        .prefix("calepin-diagram-")
        .tempdir()
        .context("failed to create temporary diagram directory")?;
    let input_path = temp_dir.path().join(format!("input.{}", spec.input_ext));
    let input_source = (spec.prepare_source)(code);
    std::fs::write(&input_path, input_source.as_bytes())
        .with_context(|| format!("failed to write {}", input_path.display()))?;

    let run = DiagramRun {
        input_path: &input_path,
        fig_path,
        work_dir: temp_dir.path(),
        executables,
    };
    let rendered = (spec.render)(&run, &mut results)?;
    if rendered && fig_path.exists() {
        results.push(EngineResult::Plot(fig_path.to_path_buf()));
    }

    Ok(results)
}

fn diagram_spec(engine: EngineName) -> Option<DiagramSpec> {
    match engine {
        EngineName::Mermaid => Some(DiagramSpec {
            input_ext: mermaid::INPUT_EXT,
            prepare_source: identity_source,
            render: mermaid::render,
        }),
        EngineName::Dot => Some(DiagramSpec {
            input_ext: dot::INPUT_EXT,
            prepare_source: identity_source,
            render: dot::render,
        }),
        EngineName::Tikz => Some(DiagramSpec {
            input_ext: tikz::INPUT_EXT,
            prepare_source: tikz::prepare_source,
            render: tikz::render,
        }),
        EngineName::D2 => Some(DiagramSpec {
            input_ext: d2::INPUT_EXT,
            prepare_source: identity_source,
            render: d2::render,
        }),
        _ => None,
    }
}

fn identity_source(code: &str) -> Cow<'_, str> {
    Cow::Borrowed(code)
}

pub(super) fn path_arg(path: &Path) -> OsString {
    path.as_os_str().to_os_string()
}

pub(super) fn run_checked_tool(
    tool: &Tool,
    program: &Path,
    args: &[OsString],
    results: &mut Vec<EngineResult>,
) -> Result<bool> {
    let Some(output) = run_tool(tool, program, args, results)? else {
        return Ok(false);
    };
    if !output.status.success() {
        results.push(tool_error(program, output.stderr));
        return Ok(false);
    }
    Ok(true)
}

pub(super) fn run_tool(
    tool: &Tool,
    program: &Path,
    args: &[OsString],
    results: &mut Vec<EngineResult>,
) -> Result<Option<Output>> {
    match std::process::Command::new(program).args(args).output() {
        Ok(out) => Ok(Some(out)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            results.push(EngineResult::Error(tools::configured_not_found_message(
                tool, program,
            )));
            Ok(None)
        }
        Err(error) => Err(error).with_context(|| format!("failed to run {}", program.display())),
    }
}

pub(super) fn tool_error(program: &Path, stderr: Vec<u8>) -> EngineResult {
    let stderr = String::from_utf8_lossy(&stderr);
    EngineResult::Error(format!("{} failed: {}", program.display(), stderr.trim()))
}

#[cfg(test)]
pub(super) mod test_support {
    use crate::engines::EngineResult;
    use std::ffi::{OsStr, OsString};
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};

    pub(super) fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    pub(super) struct EnvVarGuard {
        key: &'static str,
        old_value: Option<OsString>,
    }

    impl EnvVarGuard {
        pub(super) fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
            let guard = Self {
                key,
                old_value: std::env::var_os(key),
            };
            std::env::set_var(key, value);
            guard
        }

        pub(super) fn prepend_path(path: PathBuf) -> Self {
            let old_value = std::env::var_os("PATH");
            let mut paths = vec![path];
            if let Some(old_path) = &old_value {
                paths.extend(std::env::split_paths(old_path));
            }
            let guard = Self {
                key: "PATH",
                old_value,
            };
            std::env::set_var("PATH", std::env::join_paths(paths).unwrap());
            guard
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.old_value {
                std::env::set_var(self.key, value);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    #[cfg(unix)]
    pub(super) fn write_executable(path: &Path, contents: impl AsRef<[u8]>) {
        use std::os::unix::fs::PermissionsExt;

        std::fs::write(path, contents).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    pub(super) fn assert_successful_plot(results: &[EngineResult], fig_path: &Path) {
        assert!(fig_path.exists());
        assert!(results
            .iter()
            .any(|result| matches!(result, EngineResult::Plot(path) if path == fig_path)));
        assert!(!results
            .iter()
            .any(|result| matches!(result, EngineResult::Error(_))));
    }
}
