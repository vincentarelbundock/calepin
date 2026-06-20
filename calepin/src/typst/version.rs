use std::ffi::OsString;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use crate::typst::run::run_typst_capture;

pub const REQUIRED_TYPST_VERSION: &str = "0.15.0";
const REQUIRED_TYPST_VERSION_PARTS: (u64, u64, u64) = (0, 15, 0);

pub fn assert_supported_typst(typst: &Path) -> Result<()> {
    let version = typst_version(typst)?;
    if version_is_too_old(&version) {
        return Err(anyhow!(
            "Calepin requires Typst {} or newer, but {} reports Typst {}. Update Typst from https://github.com/typst/typst#installation",
            REQUIRED_TYPST_VERSION,
            typst.display(),
            version
        ));
    }
    Ok(())
}

pub fn typst_version(typst: &Path) -> Result<String> {
    let args = vec![OsString::from("--version")];
    let cwd = std::env::current_dir().context("failed to resolve current directory")?;
    let stdout = run_typst_capture(
        typst,
        "check typst version",
        &args,
        &cwd,
        |stderr| format!("failed to check typst version:\n{stderr}"),
        "typst --version output was not UTF-8",
    )?;
    parse_typst_version(&stdout)
}

fn parse_typst_version(output: &str) -> Result<String> {
    output
        .split_whitespace()
        .find(|token| version_parts(token).is_some())
        .map(|token| token.trim_start_matches('v').to_string())
        .ok_or_else(|| anyhow!("failed to parse typst version from `{}`", output.trim()))
}

pub fn version_is_too_old(version: &str) -> bool {
    parsed_version(version).is_some_and(|version| {
        version.parts < REQUIRED_TYPST_VERSION_PARTS
            || (version.parts == REQUIRED_TYPST_VERSION_PARTS && version.is_prerelease)
    })
}

fn version_parts(token: &str) -> Option<(u64, u64, u64)> {
    parsed_version(token).map(|version| version.parts)
}

struct ParsedVersion {
    parts: (u64, u64, u64),
    is_prerelease: bool,
}

fn parsed_version(token: &str) -> Option<ParsedVersion> {
    let version = token.trim_start_matches('v');
    let is_prerelease = version.contains('-');
    let version = version.split('-').next()?;
    let mut parts = version.split('.');
    let parts = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    Some(ParsedVersion {
        parts,
        is_prerelease,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typst_version_output() {
        assert_eq!(
            parse_typst_version("typst 0.15.0 (abc123)").unwrap(),
            "0.15.0"
        );
    }

    #[test]
    fn parses_prefixed_version_token() {
        assert_eq!(parse_typst_version("typst v0.15.0").unwrap(), "0.15.0");
    }

    #[test]
    fn parses_prerelease_version_token() {
        assert_eq!(
            parse_typst_version("typst 0.15.0-rc.1 (abc123)").unwrap(),
            "0.15.0-rc.1"
        );
    }

    #[test]
    fn compares_versions() {
        assert!(version_is_too_old("0.14.2"));
        assert!(version_is_too_old("0.15.0-rc.1"));
        assert!(!version_is_too_old("0.15.0"));
    }

    #[test]
    fn rejects_output_without_version() {
        assert!(parse_typst_version("typst dev").is_err());
    }
}
