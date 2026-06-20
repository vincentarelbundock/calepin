use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::utils::path;
use anyhow::{Context, Result};

pub const COMMON_SKIP_DIRS: &[&str] = &[".calepin", ".git", "target", "node_modules", ".venv"];
pub const TEXT_PLAIN_UTF8: &str = "text/plain; charset=utf-8";
pub const APPLICATION_JSON_UTF8: &str = "application/json; charset=utf-8";
pub const CACHE_CONTROL_NO_STORE: &str = "no-store";
pub const ACCESS_CONTROL_ANY_ORIGIN: &str = "*";

pub fn raw_http_response_head(
    status: &str,
    content_type: &str,
    content_len: usize,
    access_control: bool,
) -> String {
    let cors = if access_control {
        format!("Access-Control-Allow-Origin: {ACCESS_CONTROL_ANY_ORIGIN}\r\n")
    } else {
        String::new()
    };
    format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {content_len}\r\n\
         {cors}\
         Cache-Control: {CACHE_CONTROL_NO_STORE}\r\n\
         Connection: close\r\n\
         \r\n"
    )
}

pub fn request_relative_path(
    target: &str,
    base_path_prefix: Option<&str>,
    allow_empty: bool,
) -> Option<PathBuf> {
    let target = path::strip_query_and_fragment(target);
    let decoded = percent_decode(target)?;
    if decoded.contains('\\') {
        return None;
    }
    let decoded = strip_base_path_prefix(&decoded, base_path_prefix);
    let trimmed = path::strip_leading_url_prefix(&decoded);

    let mut relative = PathBuf::new();
    for component in Path::new(trimmed).components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    if relative.as_os_str().is_empty() && !allow_empty {
        None
    } else {
        Some(relative)
    }
}

pub fn resolve_request_path(
    root: &Path,
    target: &str,
    base_path_prefix: Option<&str>,
    allow_empty: bool,
) -> Option<PathBuf> {
    Some(root.join(request_relative_path(
        target,
        base_path_prefix,
        allow_empty,
    )?))
}

pub fn resolve_existing_file(
    root: &Path,
    target: &str,
    base_path_prefix: Option<&str>,
) -> Option<PathBuf> {
    let canonical = resolve_request_path(root, target, base_path_prefix, false)
        .and_then(|path| path.canonicalize().ok())?;
    path::is_within_root(root, &canonical)
        .then_some(canonical)
        .filter(|path| path.is_file())
}

pub fn path_stays_under_root(root: &Path, path: &Path) -> bool {
    path::is_within_root(root, path)
}

pub fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("gif") => "image/gif",
        Some("html") => "text/html; charset=utf-8",
        Some("ico") => "image/x-icon",
        Some("jpeg" | "jpg") => "image/jpeg",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("mp4") => "video/mp4",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

pub fn path_has_common_skip_dir(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|name| COMMON_SKIP_DIRS.contains(&name))
    })
}

pub fn collect_files_by<ShouldDescend, IncludeFile>(
    root: &Path,
    dir: &Path,
    out: &mut Vec<PathBuf>,
    mut should_descend: ShouldDescend,
    mut include_file: IncludeFile,
) -> Result<()>
where
    ShouldDescend: FnMut(&Path, &Path) -> bool,
    IncludeFile: FnMut(&Path, &Path) -> bool,
{
    collect_files_by_inner(
        root,
        dir,
        out,
        &mut should_descend,
        &mut include_file,
        &mut HashSet::new(),
    )
}

fn collect_files_by_inner<ShouldDescend, IncludeFile>(
    root: &Path,
    dir: &Path,
    out: &mut Vec<PathBuf>,
    should_descend: &mut ShouldDescend,
    include_file: &mut IncludeFile,
    visited_dirs: &mut HashSet<PathBuf>,
) -> Result<()>
where
    ShouldDescend: FnMut(&Path, &Path) -> bool,
    IncludeFile: FnMut(&Path, &Path) -> bool,
{
    let canonical = path::canonical_root(dir).unwrap_or_else(|| dir.to_path_buf());
    if !visited_dirs.insert(canonical) {
        return Ok(());
    }

    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let path = entry?.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);

        if is_symlink(&path) {
            collect_included_file(rel, &path, out, include_file);
            continue;
        }

        if path.is_dir() {
            if should_descend(rel, &path) {
                collect_files_by_inner(root, &path, out, should_descend, include_file, visited_dirs)?;
            }
        } else {
            collect_included_file(rel, &path, out, include_file);
        }
    }
    Ok(())
}

fn collect_included_file<IncludeFile>(
    rel: &Path,
    path: &Path,
    out: &mut Vec<PathBuf>,
    include_file: &mut IncludeFile,
) where
    IncludeFile: FnMut(&Path, &Path) -> bool,
{
    if path.is_file() && include_file(rel, path) {
        out.push(path.to_path_buf());
    }
}

fn is_symlink(path: &Path) -> bool {
    path.symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
}

fn strip_base_path_prefix<'a>(path: &'a str, base_path_prefix: Option<&str>) -> &'a str {
    let Some(prefix) = base_path_prefix else {
        return path;
    };
    let prefix = prefix.trim_end_matches('/');
    if prefix.is_empty() || prefix == "/" {
        return path;
    }
    if path == prefix {
        return "/";
    }
    path.strip_prefix(prefix)
        .filter(|rest| rest.starts_with('/'))
        .unwrap_or(path)
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = *bytes.get(index + 1)?;
            let low = *bytes.get(index + 2)?;
            decoded.push(hex_value(high)? << 4 | hex_value(low)?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(decoded).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn request_relative_path_accepts_root_relative_assets() {
        assert_eq!(
            request_relative_path(
                "/.calepin/paper/figures/fig%2Ddemo.svg?cache=1",
                None,
                false
            )
            .unwrap(),
            PathBuf::from(".calepin")
                .join("paper")
                .join("figures")
                .join("fig-demo.svg")
        );
    }

    #[test]
    fn request_relative_path_rejects_traversal() {
        assert!(request_relative_path("/../secret.txt", None, false).is_none());
        assert!(request_relative_path("/.calepin/%2e%2e/secret.txt", None, false).is_none());
        assert!(request_relative_path("/.calepin\\secret.txt", None, false).is_none());
    }

    #[test]
    fn request_relative_path_accepts_configured_base_url_prefix() {
        let root = Path::new("/tmp/site");

        assert_eq!(
            resolve_request_path(
                root,
                "/calepin/notebooks/guide.html",
                Some("/calepin"),
                true
            )
            .unwrap(),
            root.join("notebooks").join("guide.html")
        );
        assert_eq!(
            resolve_request_path(root, "/calepin", Some("/calepin"), true).unwrap(),
            root
        );
        assert_eq!(
            resolve_request_path(root, "/other/notebooks/guide.html", Some("/calepin"), true)
                .unwrap(),
            root.join("other").join("notebooks").join("guide.html")
        );
    }

    #[test]
    fn collect_files_by_applies_directory_and_file_filters() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("keep")).unwrap();
        fs::create_dir_all(dir.path().join("skip")).unwrap();
        fs::write(dir.path().join("keep").join("a.typ"), "").unwrap();
        fs::write(dir.path().join("keep").join("b.txt"), "").unwrap();
        fs::write(dir.path().join("skip").join("c.typ"), "").unwrap();

        let mut files = Vec::new();
        collect_files_by(
            dir.path(),
            dir.path(),
            &mut files,
            |rel, _| rel != Path::new("skip"),
            |_, path| path.extension().and_then(|ext| ext.to_str()) == Some("typ"),
        )
        .unwrap();

        assert_eq!(files, vec![dir.path().join("keep").join("a.typ")]);
    }

    #[cfg(unix)]
    #[test]
    fn collect_files_by_skips_symlink_directories_to_avoid_cycles() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let keep = dir.path().join("keep");
        fs::create_dir(&keep).unwrap();
        fs::write(keep.join("a.typ"), "").unwrap();
        symlink(dir.path(), dir.path().join("loop")).unwrap();

        let mut files = Vec::new();
        collect_files_by(
            dir.path(),
            dir.path(),
            &mut files,
            |_, _| true,
            |_, path| path.extension().and_then(|ext| ext.to_str()) == Some("typ"),
        )
        .unwrap();

        assert_eq!(files, vec![dir.path().join("keep").join("a.typ")]);
    }

    #[cfg(unix)]
    #[test]
    fn resolve_existing_file_accepts_paths_with_symlink_root() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let actual_root = dir.path().join("actual");
        let link_root = dir.path().join("link-root");
        fs::create_dir(&actual_root).unwrap();
        symlink(&actual_root, &link_root).unwrap();
        fs::write(actual_root.join("index.html"), "ok").unwrap();

        assert!(resolve_existing_file(&link_root, "/index.html", None).is_some());
    }
}
