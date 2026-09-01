use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Output};

use anyhow::{anyhow, Context, Result};

use crate::typst::model::LayoutPaths;
use crate::typst::paths::slash_path;
use crate::utils::{process, tools};

pub const INPUT_MODE: &str = "calepin-mode";
pub const INPUT_RESULTS: &str = "calepin-results";
pub const INPUT_STORE: &str = "calepin-store";
pub const INPUT_TARGET: &str = "calepin-target";
pub const INPUT_ASSETS: &str = "calepin-assets";
pub const INPUT_PAGES: &str = "calepin-pages";
pub const INPUT_CURRENT_HREF: &str = "calepin-current-href";
pub const INPUT_IMAGE_META: &str = "calepin-image-meta";
pub const INPUT_SOURCE_DIR: &str = "calepin-source-dir";
pub const INPUT_SITE_ROOT: &str = "calepin-site-root";

pub const RESERVED_INPUT_KEYS: &[&str] = &[
    INPUT_MODE,
    INPUT_RESULTS,
    INPUT_STORE,
    INPUT_TARGET,
    INPUT_ASSETS,
    INPUT_PAGES,
    INPUT_CURRENT_HREF,
    INPUT_IMAGE_META,
    INPUT_SOURCE_DIR,
    INPUT_SITE_ROOT,
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

/// Runs typst and returns the diagnostics it wrote to stderr on success.
///
/// Typst reports warnings (unsupported elements during HTML export, for
/// example) on stderr even when it exits successfully. Because the process is
/// spawned with captured pipes those warnings are invisible unless a caller
/// relays them, so they are handed back here instead of being discarded.
pub fn run_typst_diagnostics(
    typst: &Path,
    action: &str,
    args: &[OsString],
    cwd: &Path,
    failure: impl FnOnce(&str) -> String,
) -> Result<String> {
    let output = run_typst_output(typst, action, args, cwd, failure)?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(filter_typst_diagnostics(&stderr))
}

/// Typst repeats this warning on every HTML compile; it says nothing about the
/// document, so relaying it once per rendered page would only be noise.
const HTML_PREVIEW_WARNING: &str = "html export is under active development";

/// Drops the boilerplate HTML preview warning (and its hint lines) from typst
/// diagnostics, keeping every document-specific message untouched.
fn filter_typst_diagnostics(stderr: &str) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut skipping = false;

    for line in stderr.lines() {
        let plain = strip_ansi_codes(line);
        let plain = plain.trim_end();
        if plain.trim_start().starts_with("warning:") && plain.contains(HTML_PREVIEW_WARNING) {
            skipping = true;
            continue;
        }
        if skipping {
            // Hints, source spans and blank separators belonging to the skipped
            // warning are indented or empty; the next diagnostic starts at
            // column zero.
            if plain.trim().is_empty() || plain.starts_with(char::is_whitespace) {
                continue;
            }
            skipping = false;
        }
        kept.push(line);
    }

    while kept.first().is_some_and(|line| line.trim().is_empty()) {
        kept.remove(0);
    }
    while kept.last().is_some_and(|line| line.trim().is_empty()) {
        kept.pop();
    }

    kept.join("\n")
}

pub(super) fn strip_ansi_codes(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' && matches!(chars.peek(), Some('[')) {
            chars.next();
            for next in chars.by_ref() {
                if next == 'm' {
                    break;
                }
            }
            continue;
        }
        out.push(ch);
    }

    out
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
    fn run_typst_diagnostics_failure_includes_status_and_stderr() {
        let dir = tempfile::tempdir().unwrap();
        let typst = dir.path().join("typst");
        write_executable(
            &typst,
            "#!/bin/sh\nprintf 'simulated failure\\n' >&2\nexit 23\n",
        );

        let err = run_typst_diagnostics(&typst, "run typst", &[], dir.path(), |stderr| {
            format!("typst failed:\n{stderr}")
        })
        .unwrap_err()
        .to_string();

        assert!(err.contains("typst failed"));
        assert!(err.contains("simulated failure"));
        assert!(err.contains("exit status"));
        assert!(err.contains("23"));
    }

    #[cfg(unix)]
    #[test]
    fn run_typst_diagnostics_returns_warnings_from_successful_runs() {
        let dir = tempfile::tempdir().unwrap();
        let typst = dir.path().join("typst");
        write_executable(
            &typst,
            "#!/bin/sh\nprintf 'warning: align was ignored during HTML export\\n' >&2\nexit 0\n",
        );

        let diagnostics = run_typst_diagnostics(&typst, "run typst", &[], dir.path(), |stderr| {
            format!("typst failed:\n{stderr}")
        })
        .unwrap();

        assert!(diagnostics.contains("align was ignored during HTML export"));
    }

    #[test]
    fn filter_typst_diagnostics_drops_the_html_preview_banner() {
        let stderr = "warning: html export is under active development and incomplete\n \
= hint: its behaviour may change at any time\n = hint: see \
https://github.com/typst/typst/issues/5512 for more information\n\nwarning: align was \
ignored during HTML export\n   \u{250c}\u{2500} paper.typ:2:2\n";

        let filtered = filter_typst_diagnostics(stderr);

        assert!(!filtered.contains("under active development"));
        assert!(!filtered.contains("hint:"));
        assert!(filtered.contains("align was ignored during HTML export"));
        assert!(filtered.contains("paper.typ:2:2"));
    }

    #[test]
    fn filter_typst_diagnostics_is_empty_when_only_the_banner_is_reported() {
        let stderr = "warning: html export is under active development and incomplete\n \
= hint: do not rely on this feature for production use cases\n";

        assert!(filter_typst_diagnostics(stderr).is_empty());
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
            INPUT_SITE_ROOT,
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
