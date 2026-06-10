use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Context, Result};

use crate::utils::{process, tools};

pub const REQUIRED_TYPST_VERSION: &str = "0.14.2";
const REQUIRED_TYPST_VERSION_PARTS: (u64, u64, u64) = (0, 14, 2);

pub fn assert_supported_typst(typst: &Path) -> Result<()> {
    process::validate_executable(typst, "check typst version", Some(&tools::TYPST))?;
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
    let output = Command::new(typst)
        .arg("--version")
        .output()
        .map_err(|error| {
            process::spawn_error(typst, "check typst version", error, Some(&tools::TYPST))
        })?;
    if !output.status.success() {
        return Err(anyhow!(
            "failed to check typst version:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stdout =
        String::from_utf8(output.stdout).context("typst --version output was not UTF-8")?;
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
    version_parts(version).is_some_and(|parts| parts < REQUIRED_TYPST_VERSION_PARTS)
}

fn version_parts(token: &str) -> Option<(u64, u64, u64)> {
    let version = token.trim_start_matches('v').split('-').next()?;
    let mut parts = version.split('.');
    Some((
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_typst_version_output() {
        assert_eq!(
            parse_typst_version("typst 0.14.2 (abc123)").unwrap(),
            "0.14.2"
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
        assert!(version_is_too_old("0.14.1"));
        assert!(!version_is_too_old("0.14.2"));
        assert!(!version_is_too_old("0.15.0-rc.1"));
        assert!(!version_is_too_old("0.15.0"));
    }

    #[test]
    fn rejects_output_without_version() {
        assert!(parse_typst_version("typst dev").is_err());
    }
}
