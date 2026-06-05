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

use crate::config::{CustomDiagramBackend, DiagramOutput, ExecutablePaths};
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
    let spec = diagram_spec(&engine)
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

pub fn execute_custom_diagram(
    code: &str,
    backend: &CustomDiagramBackend,
    fig_path: &Path,
    source: &[String],
) -> Result<Vec<EngineResult>> {
    let mut results = vec![EngineResult::Source(source.to_vec())];

    if let Some(parent) = fig_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let temp_dir = tempfile::Builder::new()
        .prefix("calepin-diagram-")
        .tempdir()
        .context("failed to create temporary diagram directory")?;
    let input_path = temp_dir
        .path()
        .join(format!("input.{}", backend.input_ext));
    std::fs::write(&input_path, code.as_bytes())
        .with_context(|| format!("failed to write {}", input_path.display()))?;

    let args: Vec<OsString> = backend
        .args
        .iter()
        .map(|arg| {
            if arg == "{input}" {
                path_arg(&input_path)
            } else if arg == "{output}" {
                path_arg(fig_path)
            } else {
                OsString::from(arg)
            }
        })
        .collect();

    let output = match std::process::Command::new(&backend.command)
        .args(&args)
        .output()
    {
        Ok(out) => out,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            results.push(EngineResult::Error(format!(
                "custom diagram command not found: {}",
                backend.command.display()
            )));
            return Ok(results);
        }
        Err(error) => {
            return Err(error).with_context(|| {
                format!("failed to run {}", backend.command.display())
            });
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        results.push(EngineResult::Error(format!(
            "{} failed: {}",
            backend.command.display(),
            stderr.trim()
        )));
        return Ok(results);
    }

    match backend.output {
        DiagramOutput::Stdout => {
            std::fs::write(fig_path, &output.stdout)
                .with_context(|| format!("failed to write {}", fig_path.display()))?;
        }
        DiagramOutput::File => {
            if !fig_path.exists() {
                results.push(EngineResult::Error(format!(
                    "{} succeeded but produced no output file",
                    backend.command.display()
                )));
                return Ok(results);
            }
        }
    }

    results.push(EngineResult::Plot(fig_path.to_path_buf()));
    Ok(results)
}

fn diagram_spec(engine: &EngineName) -> Option<DiagramSpec> {
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
mod custom_diagram_tests {
    use super::execute_custom_diagram;
    use super::test_support::{assert_successful_plot, env_lock, write_executable, EnvVarGuard};
    use crate::config::{CustomDiagramBackend, DiagramOutput};
    use std::path::PathBuf;

    fn file_backend(cmd: &str) -> CustomDiagramBackend {
        CustomDiagramBackend {
            command: PathBuf::from(cmd),
            input_ext: "txt".to_string(),
            args: vec!["{input}".to_string(), "{output}".to_string()],
            output: DiagramOutput::File,
        }
    }

    fn stdout_backend(cmd: &str) -> CustomDiagramBackend {
        CustomDiagramBackend {
            command: PathBuf::from(cmd),
            input_ext: "txt".to_string(),
            args: vec!["{input}".to_string()],
            output: DiagramOutput::Stdout,
        }
    }

    #[test]
    fn file_output_backend_produces_plot() {
        let _guard = env_lock();
        let temp_dir = tempfile::tempdir().unwrap();
        let bin_dir = temp_dir.path().join("bin");
        std::fs::create_dir(&bin_dir).unwrap();
        write_executable(
            &bin_dir.join("mydiag"),
            r#"#!/bin/sh
printf "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>" > "$2"
"#,
        );
        let _path = EnvVarGuard::prepend_path(bin_dir);

        let fig_path = temp_dir.path().join("figure.svg");
        let source = vec!["source code".to_string()];
        let results =
            execute_custom_diagram("source code", &file_backend("mydiag"), &fig_path, &source)
                .unwrap();

        assert_successful_plot(&results, &fig_path);
    }

    #[test]
    fn stdout_output_backend_produces_plot() {
        let _guard = env_lock();
        let temp_dir = tempfile::tempdir().unwrap();
        let bin_dir = temp_dir.path().join("bin");
        std::fs::create_dir(&bin_dir).unwrap();
        write_executable(
            &bin_dir.join("mydiag"),
            r#"#!/bin/sh
printf "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>"
"#,
        );
        let _path = EnvVarGuard::prepend_path(bin_dir);

        let fig_path = temp_dir.path().join("figure.svg");
        let source = vec!["source code".to_string()];
        let results =
            execute_custom_diagram("source code", &stdout_backend("mydiag"), &fig_path, &source)
                .unwrap();

        assert_successful_plot(&results, &fig_path);
    }

    #[test]
    fn failing_backend_emits_error_result() {
        let _guard = env_lock();
        let temp_dir = tempfile::tempdir().unwrap();
        let bin_dir = temp_dir.path().join("bin");
        std::fs::create_dir(&bin_dir).unwrap();
        write_executable(
            &bin_dir.join("mydiag"),
            "#!/bin/sh\necho 'bad input' >&2\nexit 1\n",
        );
        let _path = EnvVarGuard::prepend_path(bin_dir);

        let fig_path = temp_dir.path().join("figure.svg");
        let source = vec!["bad".to_string()];
        let results =
            execute_custom_diagram("bad", &file_backend("mydiag"), &fig_path, &source).unwrap();

        assert!(results
            .iter()
            .any(|r| matches!(r, crate::engines::EngineResult::Error(_))));
        assert!(!fig_path.exists());
    }

    #[test]
    fn source_written_to_input_file_with_correct_extension() {
        let _guard = env_lock();
        let temp_dir = tempfile::tempdir().unwrap();
        let bin_dir = temp_dir.path().join("bin");
        std::fs::create_dir(&bin_dir).unwrap();
        write_executable(
            &bin_dir.join("mydiag"),
            r#"#!/bin/sh
# verify input has .puml extension and contains expected content
case "$1" in *.puml) ;; *) echo "wrong ext: $1" >&2; exit 1;; esac
if ! grep -q "diagram source" "$1"; then
  echo "missing content" >&2
  exit 1
fi
printf "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>" > "$2"
"#,
        );
        let _path = EnvVarGuard::prepend_path(bin_dir);

        let backend = CustomDiagramBackend {
            command: PathBuf::from("mydiag"),
            input_ext: "puml".to_string(),
            args: vec!["{input}".to_string(), "{output}".to_string()],
            output: DiagramOutput::File,
        };
        let fig_path = temp_dir.path().join("figure.svg");
        let source = vec!["diagram source".to_string()];
        let results =
            execute_custom_diagram("diagram source", &backend, &fig_path, &source).unwrap();

        assert_successful_plot(&results, &fig_path);
    }
}

#[cfg(test)]
pub mod test_support {
    use crate::engines::EngineResult;
    use std::ffi::{OsStr, OsString};
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};

    pub fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    pub struct EnvVarGuard {
        key: &'static str,
        old_value: Option<OsString>,
    }

    impl EnvVarGuard {
        pub fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
            let guard = Self {
                key,
                old_value: std::env::var_os(key),
            };
            std::env::set_var(key, value);
            guard
        }

        pub fn prepend_path(path: PathBuf) -> Self {
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
    pub fn write_executable(path: &Path, contents: impl AsRef<[u8]>) {
        use std::os::unix::fs::PermissionsExt;

        std::fs::write(path, contents).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    pub fn assert_successful_plot(results: &[EngineResult], fig_path: &Path) {
        assert!(fig_path.exists());
        assert!(results
            .iter()
            .any(|result| matches!(result, EngineResult::Plot(path) if path == fig_path)));
        assert!(!results
            .iter()
            .any(|result| matches!(result, EngineResult::Error(_))));
    }
}
