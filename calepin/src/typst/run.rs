use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Context, Result};

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
    process::validate_executable(typst, action, Some(&tools::TYPST))?;
    let output = Command::new(typst)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| process::spawn_error(typst, action, error, Some(&tools::TYPST)))?;
    if !output.status.success() {
        return Err(anyhow!(
            "{}",
            failure(&String::from_utf8_lossy(&output.stderr))
        ));
    }
    String::from_utf8(output.stdout).context(utf8_context)
}

pub fn run_typst_status(
    typst: &Path,
    action: &str,
    args: &[OsString],
    cwd: &Path,
    failure: impl FnOnce(&str) -> String,
) -> Result<()> {
    process::validate_executable(typst, action, Some(&tools::TYPST))?;
    let output = Command::new(typst)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| process::spawn_error(typst, action, error, Some(&tools::TYPST)))?;
    if !output.status.success() {
        return Err(anyhow!(
            "{}",
            failure(&String::from_utf8_lossy(&output.stderr))
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalepinMode {
    Render,
}

impl CalepinMode {
    fn as_str(self) -> &'static str {
        match self {
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
    fn as_str(self) -> &'static str {
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
