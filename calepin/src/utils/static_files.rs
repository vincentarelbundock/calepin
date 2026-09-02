use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::utils::path;
use anyhow::{Context, Result};

pub const COMMON_SKIP_DIRS: &[&str] = &[".calepin", ".git", "target", "node_modules", ".venv"];
pub const TEXT_PLAIN_UTF8: &str = "text/plain; charset=utf-8";
pub const APPLICATION_JSON_UTF8: &str = "application/json; charset=utf-8";
pub const CACHE_CONTROL_NO_STORE: &str = "no-store";

/// Build a raw HTTP/1.1 response head. `allowed_origin`, when set, echoes
/// back a specific `Access-Control-Allow-Origin`; pass `None` to omit CORS
/// entirely. Never pass a wildcard `*` here: any page open in the browser
/// could then read the response cross-origin.
pub fn raw_http_response_head(
    status: &str,
    content_type: &str,
    content_len: usize,
    allowed_origin: Option<&str>,
) -> String {
    let cors = match allowed_origin {
        Some(origin) => format!("Access-Control-Allow-Origin: {origin}\r\n"),
        None => String::new(),
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

/// Project files that must never be handed out by a local dev/watch server,
/// even under an otherwise-servable root: version control internals, secret
/// files, and Calepin's own executable-path config (which can point at
/// arbitrary local binaries). `.calepin/` itself stays servable, since the
/// HTML asset server relies on it for generated figures.
pub fn is_sensitive_served_path(root: &Path, path: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(root) else {
        return true;
    };
    if rel == Path::new(".calepin").join("config.toml") {
        return true;
    }
    rel.components().any(|component| {
        component.as_os_str().to_str().is_some_and(|name| {
            name == ".git" || name == ".env" || (name.starts_with('.') && name != ".calepin")
        })
    })
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
    let trimmed = path::strip_leading_url_prefix(decoded);

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

/// Guess a file's `Content-Type` from its extension via `mime_guess`, rather
/// than a small hand-maintained table (which used to fall back to
/// `application/octet-stream` for common web assets like `.xml`, `.txt`,
/// `.woff2`, `.wasm`, `.mjs`, and `.webmanifest`). Text-ish types get an
/// explicit `charset=utf-8`, matching the previous table's behavior.
pub fn content_type(path: &Path) -> String {
    let essence = mime_guess::from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_string();
    if essence.starts_with("text/")
        || essence == "application/json"
        || essence == "application/javascript"
    {
        format!("{essence}; charset=utf-8")
    } else {
        essence
    }
}

pub fn path_has_common_skip_dir(path: &Path) -> bool {
    path_has_skip_dir(path, &[])
}

pub fn path_has_skip_dir(path: &Path, extra_skip_dirs: &[&Path]) -> bool {
    extra_skip_dirs.iter().any(|skip| path.starts_with(skip))
        || path.components().any(|component| {
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
                collect_files_by_inner(
                    root,
                    &path,
                    out,
                    should_descend,
                    include_file,
                    visited_dirs,
                )?;
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
    fn content_type_covers_extensions_the_old_hand_written_table_missed() {
        for (extension, expected_essence) in [
            ("xml", "text/xml"),
            ("txt", "text/plain"),
            ("woff2", "font/woff2"),
            ("wasm", "application/wasm"),
            ("mjs", "application/javascript"),
            ("webmanifest", "application/manifest+json"),
        ] {
            let guessed = content_type(Path::new(&format!("asset.{extension}")));
            assert!(
                guessed.starts_with(expected_essence),
                "extension {extension}: got {guessed}"
            );
            assert_ne!(guessed, "application/octet-stream", "extension {extension}");
        }
    }

    #[test]
    fn is_sensitive_served_path_blocks_dotfiles_git_env_and_executable_config() {
        let root = Path::new("/project");
        assert!(is_sensitive_served_path(
            root,
            &root.join(".calepin/config.toml")
        ));
        assert!(is_sensitive_served_path(root, &root.join(".git/config")));
        assert!(is_sensitive_served_path(root, &root.join(".env")));
        assert!(is_sensitive_served_path(root, &root.join(".hidden")));
        assert!(is_sensitive_served_path(
            root,
            &root.join("sub/.secret/file")
        ));
    }

    #[test]
    fn is_sensitive_served_path_allows_calepin_generated_figures() {
        let root = Path::new("/project");
        assert!(!is_sensitive_served_path(
            root,
            &root.join(".calepin/paper/figures/fig.svg")
        ));
        assert!(!is_sensitive_served_path(root, &root.join("index.html")));
    }

    #[test]
    fn path_has_skip_dir_accepts_custom_generated_dirs() {
        assert!(path_has_skip_dir(
            Path::new("_calepin/paper/source.typ"),
            &[Path::new("_calepin")]
        ));
        assert!(!path_has_skip_dir(
            Path::new("assets/logo.svg"),
            &[Path::new("_calepin")]
        ));
        assert!(path_has_skip_dir(
            Path::new(".calepin/paper/source.typ"),
            &[]
        ));
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
