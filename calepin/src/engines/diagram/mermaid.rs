use anyhow::{Context, Result};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Output;

use super::{path_arg, run_tool, tool_error, DiagramRun};
use crate::engines::EngineResult;
use crate::utils::tools;

pub(super) const INPUT_EXT: &str = "mmd";

const SANDBOX_ERROR: &str = "No usable sandbox";
const MISSING_CHROME_ERROR: &str = "Could not find Chrome";

pub(super) fn render(run: &DiagramRun<'_>, results: &mut Vec<EngineResult>) -> Result<bool> {
    let Some(mut output) = run_mmdc(run, args(run), results)? else {
        return Ok(false);
    };

    if !output.status.success() {
        let chrome_path = if is_missing_chrome_failure(&output.stderr) {
            chrome_executable_path(run.executables.chrome.as_deref())
        } else {
            None
        };
        if let Some(chrome_path) = chrome_path.as_deref() {
            let Some(retry) =
                run_mmdc(run, puppeteer_args(run, false, Some(chrome_path))?, results)?
            else {
                return Ok(false);
            };
            output = retry;
        }

        if !output.status.success() && is_sandbox_failure(&output.stderr) {
            let Some(retry) = run_mmdc(
                run,
                puppeteer_args(run, true, chrome_path.as_deref())?,
                results,
            )?
            else {
                return Ok(false);
            };
            output = retry;
        }
    }

    if !output.status.success() {
        results.push(tool_error(&run.executables.mmdc, output.stderr));
        return Ok(false);
    }

    Ok(true)
}

fn run_mmdc(
    run: &DiagramRun<'_>,
    args: Vec<OsString>,
    results: &mut Vec<EngineResult>,
) -> Result<Option<Output>> {
    run_tool(&tools::MMDC, &run.executables.mmdc, &args, results)
}

fn args(run: &DiagramRun<'_>) -> Vec<OsString> {
    vec![
        "-i".into(),
        path_arg(run.input_path),
        "-o".into(),
        path_arg(run.fig_path),
        "-b".into(),
        "transparent".into(),
    ]
}

fn puppeteer_args(
    run: &DiagramRun<'_>,
    no_sandbox: bool,
    executable_path: Option<&Path>,
) -> Result<Vec<OsString>> {
    let config_path = run.work_dir.join("puppeteer-config.json");
    let mut config = serde_json::Map::new();
    if no_sandbox {
        config.insert("args".to_string(), serde_json::json!(["--no-sandbox"]));
    }
    if let Some(executable_path) = executable_path {
        config.insert(
            "executablePath".to_string(),
            serde_json::json!(executable_path.to_string_lossy()),
        );
    }
    let json = serde_json::to_string(&config).context("failed to serialize puppeteer config")?;
    std::fs::write(&config_path, format!("{json}\n"))
        .with_context(|| format!("failed to write {}", config_path.display()))?;

    let mut args = args(run);
    args.push("-p".into());
    args.push(path_arg(&config_path));
    Ok(args)
}

fn is_sandbox_failure(stderr: &[u8]) -> bool {
    String::from_utf8_lossy(stderr).contains(SANDBOX_ERROR)
}

fn is_missing_chrome_failure(stderr: &[u8]) -> bool {
    String::from_utf8_lossy(stderr).contains(MISSING_CHROME_ERROR)
}

fn chrome_executable_path(configured: Option<&Path>) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = configured {
        candidates.push(path.to_path_buf());
    }
    if let Some(path) = std::env::var_os("PUPPETEER_EXECUTABLE_PATH") {
        candidates.push(PathBuf::from(path));
    }

    candidates.extend([
        PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
        PathBuf::from("/usr/bin/google-chrome"),
        PathBuf::from("/usr/bin/google-chrome-stable"),
        PathBuf::from("/usr/bin/chromium"),
        PathBuf::from("/usr/bin/chromium-browser"),
        PathBuf::from("/snap/bin/chromium"),
    ]);

    for env_var in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"] {
        if let Some(base) = std::env::var_os(env_var) {
            candidates.push(PathBuf::from(base).join(r"Google\Chrome\Application\chrome.exe"));
        }
    }

    candidates.into_iter().find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::super::execute_diagram;
    use super::super::test_support::{
        assert_successful_plot, env_lock, write_executable, EnvVarGuard,
    };
    use crate::config::ExecutablePaths;
    use crate::typst::model::EngineName;

    #[test]
    fn retries_with_no_sandbox_config_when_chromium_requires_it() {
        let _guard = env_lock();
        let temp_dir = tempfile::tempdir().unwrap();
        let bin_dir = temp_dir.path().join("bin");
        std::fs::create_dir(&bin_dir).unwrap();
        write_executable(
            &bin_dir.join("mmdc"),
            r#"#!/bin/sh
out=""
config=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) shift; out="$1" ;;
    -p|--puppeteerConfigFile) shift; config="$1" ;;
  esac
  shift
done
if [ -z "$config" ]; then
  echo "No usable sandbox!" >&2
  exit 1
fi
if ! grep -q -- "--no-sandbox" "$config"; then
  echo "missing no-sandbox" >&2
  exit 2
fi
printf "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>" > "$out"
"#,
        );
        let _path = EnvVarGuard::prepend_path(bin_dir);

        let fig_path = temp_dir.path().join("figure.svg");
        let source = vec!["flowchart LR".to_string(), "  A --> B".to_string()];
        let results = execute_diagram(
            "flowchart LR\n  A --> B",
            EngineName::Mermaid,
            &fig_path,
            &source,
            &ExecutablePaths::defaults(),
        )
        .unwrap();

        assert_successful_plot(&results, &fig_path);
    }

    #[test]
    fn retries_with_detected_chrome_when_puppeteer_cache_is_missing() {
        let _guard = env_lock();
        let temp_dir = tempfile::tempdir().unwrap();
        let chrome_path = temp_dir.path().join("chrome");
        write_executable(&chrome_path, "#!/bin/sh\n");

        let bin_dir = temp_dir.path().join("bin");
        std::fs::create_dir(&bin_dir).unwrap();
        write_executable(
            &bin_dir.join("mmdc"),
            format!(
                r#"#!/bin/sh
out=""
config=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) shift; out="$1" ;;
    -p|--puppeteerConfigFile) shift; config="$1" ;;
  esac
  shift
done
if [ -z "$config" ]; then
  echo "Could not find Chrome (ver. 148.0.7778.97)." >&2
  exit 1
fi
if ! grep -q -- "{}" "$config"; then
  echo "missing executablePath" >&2
  exit 2
fi
printf "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>" > "$out"
"#,
                chrome_path.display()
            ),
        );
        let _path = EnvVarGuard::prepend_path(bin_dir);
        let _chrome = EnvVarGuard::set("PUPPETEER_EXECUTABLE_PATH", &chrome_path);

        let fig_path = temp_dir.path().join("figure.svg");
        let source = vec!["flowchart LR".to_string(), "  A --> B".to_string()];
        let results = execute_diagram(
            "flowchart LR\n  A --> B",
            EngineName::Mermaid,
            &fig_path,
            &source,
            &ExecutablePaths::defaults(),
        )
        .unwrap();

        assert_successful_plot(&results, &fig_path);
    }
}
