use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Output};

use anyhow::{anyhow, Context, Result};

use crate::typst::model::LayoutPaths;
use crate::typst::paths::slash_path;
use crate::utils::{process, tools};

pub const INPUT_MODE: &str = "calepin-mode";
pub const INPUT_RESULTS: &str = "calepin-results";
pub const INPUT_TARGET: &str = "calepin-target";
pub const INPUT_ASSETS: &str = "calepin-assets";
pub const INPUT_PAGES: &str = "calepin-pages";
pub const INPUT_CURRENT_HREF: &str = "calepin-current-href";
pub const INPUT_IMAGE_META: &str = "calepin-image-meta";
pub const INPUT_SOURCE_DIR: &str = "calepin-source-dir";

pub const RESERVED_INPUT_KEYS: &[&str] = &[
    INPUT_MODE,
    INPUT_RESULTS,
    INPUT_TARGET,
    INPUT_ASSETS,
    INPUT_PAGES,
    INPUT_CURRENT_HREF,
    INPUT_IMAGE_META,
    INPUT_SOURCE_DIR,
];

pub fn run_typst_capture(
    typst: &Path,
    action: &str,
    args: &[OsString],
    cwd: &Path,
    failure: impl FnOnce(&str) -> String,
    utf8_context: &'static str,
) -> Result<String> {
    let output = run_typst_output(typst, action, args, cwd, failure)?;
    String::from_utf8(output.stdout).context(utf8_context)
}

pub fn run_typst_status(
    typst: &Path,
    action: &str,
    args: &[OsString],
    cwd: &Path,
    failure: impl FnOnce(&str) -> String,
) -> Result<()> {
    run_typst_output(typst, action, args, cwd, failure)?;
    Ok(())
}

fn run_typst_output(
    typst: &Path,
    action: &str,
    args: &[OsString],
    cwd: &Path,
    failure: impl FnOnce(&str) -> String,
) -> Result<Output> {
    process::validate_executable(typst, action, Some(&tools::TYPST))?;
    let output = Command::new(typst)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| process::spawn_error(typst, action, error, Some(&tools::TYPST)))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut message = failure(&stderr);
        if !message.ends_with('\n') {
            message.push('\n');
        }
        return Err(anyhow!("{}typst exited with {}", message, output.status));
    }
    Ok(output)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalepinMode {
    Query,
    Render,
}

impl CalepinMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Render => "render",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalepinTarget {
    Paged,
    Html,
}

impl CalepinTarget {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Paged => "paged",
            Self::Html => "html",
        }
    }
}

pub fn push_input(args: &mut Vec<OsString>, key: &str, value: impl AsRef<str>) {
    args.push("--input".into());
    args.push(format!("{key}={}", value.as_ref()).into());
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypstInput {
    key: &'static str,
    value: String,
}

impl TypstInput {
    pub fn new(key: &'static str, value: impl Into<String>) -> Self {
        Self {
            key,
            value: value.into(),
        }
    }

    pub fn push_to(&self, args: &mut Vec<OsString>) {
        push_input(args, self.key, &self.value);
    }
}

pub fn source_dir_input(layout: &LayoutPaths) -> String {
    layout
        .input_rel
        .parent()
        .map(slash_path)
        .unwrap_or_default()
}

pub fn push_calepin_inputs(
    args: &mut Vec<OsString>,
    mode: CalepinMode,
    results: &str,
    target: CalepinTarget,
) {
    push_input(args, INPUT_MODE, mode.as_str());
    push_input(args, INPUT_RESULTS, results);
    push_input(args, INPUT_TARGET, target.as_str());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn run_typst_status_failure_includes_status_and_stderr() {
        let dir = tempfile::tempdir().unwrap();
        let typst = dir.path().join("typst");
        write_executable(
            &typst,
            "#!/bin/sh\nprintf 'simulated failure\\n' >&2\nexit 23\n",
        );

        let err = run_typst_status(&typst, "run typst", &[], dir.path(), |stderr| {
            format!("typst failed:\n{stderr}")
        })
        .unwrap_err()
        .to_string();

        assert!(err.contains("typst failed"));
        assert!(err.contains("simulated failure"));
        assert!(err.contains("exit status"));
        assert!(err.contains("23"));
    }

    #[test]
    fn calepin_input_construction_uses_typed_values() {
        let mut args = Vec::new();
        push_calepin_inputs(
            &mut args,
            CalepinMode::Render,
            "/.calepin/paper/results.json",
            CalepinTarget::Html,
        );

        let args = args_to_strings(args);
        assert_eq!(
            args,
            [
                "--input",
                "calepin-mode=render",
                "--input",
                "calepin-results=/.calepin/paper/results.json",
                "--input",
                "calepin-target=html",
            ]
        );

        let mut args = Vec::new();
        push_calepin_inputs(
            &mut args,
            CalepinMode::Query,
            "/.calepin/paper/results.json",
            CalepinTarget::Paged,
        );

        let args = args_to_strings(args);
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--input", "calepin-mode=query"]));
        assert!(args
            .windows(2)
            .any(|pair| pair == ["--input", "calepin-target=paged"]));
    }

    #[test]
    fn reserved_input_keys_cover_all_calepin_input_constants() {
        for key in [
            INPUT_MODE,
            INPUT_RESULTS,
            INPUT_TARGET,
            INPUT_ASSETS,
            INPUT_PAGES,
            INPUT_CURRENT_HREF,
            INPUT_IMAGE_META,
            INPUT_SOURCE_DIR,
        ] {
            assert!(
                RESERVED_INPUT_KEYS.contains(&key),
                "{key} should be reserved"
            );
        }
    }

    fn args_to_strings(args: Vec<OsString>) -> Vec<String> {
        args.into_iter()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect()
    }

    #[cfg(unix)]
    fn write_executable(path: &Path, contents: &str) {
        std::fs::write(path, contents).unwrap();
        make_executable(path);
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}
