use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};

pub(super) use crate::utils::path::{normalize_path, slash_path};

pub(super) fn rel_posix(src_dir: &Path, path: &Path) -> String {
    path.strip_prefix(src_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(super) fn source_relative_path<'a>(src_dir: &Path, input_path: &'a Path) -> &'a Path {
    relative_or_self(src_dir, input_path)
}

pub(super) fn relative_or_self<'a>(base: &Path, path: &'a Path) -> &'a Path {
    path.strip_prefix(base).unwrap_or(path)
}

pub(super) fn canonicalize_within_root(root: &Path, path: &Path, what: &str) -> Result<PathBuf> {
    let normalized_root = root
        .canonicalize()
        .with_context(|| format!("failed to resolve source directory {}", root.display()))?;
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        normalized_root.join(path)
    };
    let resolved = absolute
        .canonicalize()
        .with_context(|| format!("failed to resolve local path {}", path.display()))?;
    if !resolved.starts_with(&normalized_root) {
        bail!("{what}");
    }
    Ok(resolved)
}

pub(super) fn join_normalized_under_root(root: &Path, value: &Path, what: &str) -> Result<PathBuf> {
    let root = normalize_path(root);
    if value.as_os_str().is_empty()
        || value.is_absolute()
        || value
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        bail!("{what}: {}", value.display());
    }
    let candidate = normalize_path(&root.join(value));
    if !candidate.starts_with(&root) {
        bail!("{what}: {}", value.display());
    }
    Ok(candidate)
}

pub(super) fn output_path_for_source_file(
    src_dir: &Path,
    out_dir: &Path,
    input_path: &Path,
) -> PathBuf {
    out_dir.join(source_relative_path(src_dir, input_path))
}

pub(super) fn ensure_path_within_root<'a>(
    root: &Path,
    path: &'a Path,
    what: &str,
) -> Result<&'a Path> {
    if !path.starts_with(root)
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        bail!(
            "invalid {what} {} for output directory {}",
            path.display(),
            root.display()
        )
    }
    Ok(path)
}

pub(super) fn ensure_relative_path<'a>(path: &'a Path, what: &str) -> Result<&'a Path> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        bail!("invalid {what}: {}", path.display())
    }
    Ok(path)
}

pub(super) fn wildcard_match(pattern: &str, value: &str) -> bool {
    let pattern = glob_tokens(pattern);
    let value = value.as_bytes();
    let mut previous = vec![false; value.len() + 1];
    previous[0] = true;
    for token in pattern {
        let mut current = vec![false; value.len() + 1];
        if matches!(token, GlobToken::SegmentStar | GlobToken::GlobStar) {
            current[0] = previous[0];
        }
        for j in 1..=value.len() {
            current[j] = match token {
                GlobToken::Literal(byte) => previous[j - 1] && byte == value[j - 1],
                GlobToken::AnyChar => previous[j - 1] && value[j - 1] != b'/',
                GlobToken::SegmentStar => previous[j] || (value[j - 1] != b'/' && current[j - 1]),
                GlobToken::GlobStar => previous[j] || current[j - 1],
            };
        }
        previous = current;
    }
    previous[value.len()]
}

#[derive(Clone, Copy)]
enum GlobToken {
    Literal(u8),
    AnyChar,
    SegmentStar,
    GlobStar,
}

fn glob_tokens(pattern: &str) -> Vec<GlobToken> {
    let bytes = pattern.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'*' => {
                let start = index;
                while index < bytes.len() && bytes[index] == b'*' {
                    index += 1;
                }
                if index - start >= 2 {
                    tokens.push(GlobToken::GlobStar);
                } else {
                    tokens.push(GlobToken::SegmentStar);
                }
            }
            b'?' => {
                tokens.push(GlobToken::AnyChar);
                index += 1;
            }
            byte => {
                tokens.push(GlobToken::Literal(byte));
                index += 1;
            }
        }
    }
    tokens
}
