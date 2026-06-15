use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::utils::http::timeout_agent;
use crate::utils::path::normalize_path;

use super::{HealthCheck, HealthStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
struct LinkOccurrence {
    source: PathBuf,
    line: usize,
    target: String,
}

const LINK_CHECK_SKIP_DIRS: &[&str] = &[".calepin", ".git", "target", "node_modules", ".venv"];

pub(super) fn link_check(
    root: &Path,
    check_links_depth: Option<usize>,
    check_external_links: bool,
) -> HealthCheck {
    match check_links(root, check_links_depth, check_external_links) {
        Ok(LinkSummary {
            files,
            links,
            broken,
            broken_local,
            broken_external,
        }) => {
            let status = if broken_local > 0 {
                HealthStatus::Error
            } else if broken_external > 0 {
                HealthStatus::Warning
            } else {
                HealthStatus::Ok
            };
            let message = if broken_local == 0 && broken_external == 0 {
                format!("checked {links} literal link(s) in {files} Typst file(s)")
            } else {
                let mut suffix = Vec::new();
                if broken_local > 0 {
                    suffix.push(format!("{broken_local} local"));
                }
                if broken_external > 0 {
                    suffix.push(format!("{broken_external} external"));
                }
                format!(
                    "{} broken link(s) among {links} literal link(s) in {files} Typst file(s)",
                    suffix.join(" and "),
                )
            };
            let hint = (!broken.is_empty()).then(|| {
                if broken_local > 0 {
                    "fix missing local link targets or rebuild generated linked outputs".to_string()
                } else {
                    "fix missing external link targets".to_string()
                }
            });
            HealthCheck::new("links", status, message)
                .with_path(root.display().to_string())
                .with_optional_hint(hint)
                .with_details(broken)
        }
        Err(error) => HealthCheck::warn("links", format!("failed to check links: {error}"))
            .with_path(root.display().to_string()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LinkSummary {
    files: usize,
    links: usize,
    broken: Vec<String>,
    broken_local: usize,
    broken_external: usize,
}

fn check_links(
    root: &Path,
    check_links_depth: Option<usize>,
    check_external_links: bool,
) -> Result<LinkSummary> {
    let typ_files = collect_typst_files(root, check_links_depth)?;
    let mut links = 0usize;
    let mut broken = Vec::new();
    let mut broken_local = 0usize;
    let mut broken_external = 0usize;
    let external_agent = check_external_links.then(external_link_agent);

    for file in &typ_files {
        let source = fs::read_to_string(file)
            .with_context(|| format!("failed to read {}", file.display()))?;
        for link in extract_literal_links(file, &source) {
            links += 1;
            if is_external_http_link(&link.target) {
                if let Some(client) = external_agent.as_ref() {
                    if let Some(message) = validate_external_link(root, &link, client) {
                        broken.push(message);
                        broken_external += 1;
                    }
                }
            } else if let Some(message) = validate_local_link(root, &link) {
                broken.push(message);
                broken_local += 1;
            }
        }
    }

    Ok(LinkSummary {
        files: typ_files.len(),
        links,
        broken,
        broken_local,
        broken_external,
    })
}

fn collect_typst_files(root: &Path, max_depth: Option<usize>) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect_typst_files_in(root, root, 0, max_depth, &mut out)?;
    out.sort();
    Ok(out)
}

fn collect_typst_files_in(
    root: &Path,
    dir: &Path,
    depth: usize,
    max_depth: Option<usize>,
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        if rel.components().any(|component| {
            component
                .as_os_str()
                .to_str()
                .is_some_and(|name| LINK_CHECK_SKIP_DIRS.contains(&name))
        }) {
            continue;
        }
        if path.is_dir() {
            if max_depth.is_none_or(|limit| depth < limit) {
                collect_typst_files_in(root, &path, depth + 1, max_depth, out)?;
            }
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("typ") {
            out.push(path);
        }
    }
    Ok(())
}

fn extract_literal_links(source_path: &Path, source: &str) -> Vec<LinkOccurrence> {
    let mut links = Vec::new();
    let mut offset = 0usize;
    while let Some(relative) = source[offset..].find("#link") {
        let start = offset + relative;
        let after_name = start + "#link".len();
        if source[after_name..]
            .chars()
            .next()
            .is_some_and(is_identifier_char)
        {
            offset = after_name;
            continue;
        }
        let open = skip_ws(source, after_name);
        if !source[open..].starts_with('(') {
            offset = after_name;
            continue;
        }
        let value_start = skip_ws(source, open + 1);
        if !source[value_start..].starts_with('"') {
            offset = value_start;
            continue;
        }
        match parse_string_literal(source, value_start) {
            Some((target, end)) => {
                links.push(LinkOccurrence {
                    source: source_path.to_path_buf(),
                    line: line_number(source, start),
                    target,
                });
                offset = end;
            }
            None => {
                offset = value_start + 1;
            }
        }
    }
    links
}

fn validate_local_link(root: &Path, link: &LinkOccurrence) -> Option<String> {
    let target = link.target.trim();
    if target.is_empty() || is_external_or_special_link(target) {
        return None;
    }
    let path_part = target
        .split_once(['#', '?'])
        .map(|(path, _)| path)
        .unwrap_or(target)
        .trim();
    if path_part.is_empty() {
        return None;
    }

    let base = if path_part.starts_with('/') {
        root.to_path_buf()
    } else {
        link.source.parent().unwrap_or(root).to_path_buf()
    };
    let candidate = normalize_path(&base.join(path_part.trim_start_matches('/')));
    if !candidate.starts_with(root) {
        return Some(format!(
            "{}:{} `{}` escapes the project root",
            display_rel(root, &link.source),
            link.line,
            link.target
        ));
    }
    if link_target_exists(&candidate) {
        return None;
    }

    Some(format!(
        "{}:{} missing local link target `{}`",
        display_rel(root, &link.source),
        link.line,
        link.target
    ))
}

fn external_link_agent() -> ureq::Agent {
    timeout_agent(Duration::from_secs(8))
}

fn is_external_http_link(target: &str) -> bool {
    target.starts_with("http://") || target.starts_with("https://")
}

fn validate_external_link(
    root: &Path,
    link: &LinkOccurrence,
    client: &ureq::Agent,
) -> Option<String> {
    let target = link.target.trim();
    if target.is_empty() {
        return None;
    }

    let normalized = target
        .split_once('#')
        .map(|(path, _)| path)
        .unwrap_or(target)
        .trim();
    if normalized.is_empty() {
        return None;
    }

    external_link_error(client, normalized).map(|detail| {
        format!(
            "{}:{} external link `{}` is not reachable ({detail})",
            display_rel(root, &link.source),
            link.line,
            link.target
        )
    })
}

fn external_link_error(client: &ureq::Agent, url: &str) -> Option<String> {
    match client.head(url).call() {
        Ok(response) if response.status() < 400 => None,
        Ok(response) => {
            fallback_get_error(client, url, Some(format!("HTTP {}", response.status())))
        }
        Err(error) => fallback_get_error(client, url, Some(error.to_string())),
    }
}

fn fallback_get_error(
    client: &ureq::Agent,
    url: &str,
    head_error: Option<String>,
) -> Option<String> {
    match client.get(url).call() {
        Ok(response) if response.status() < 400 => None,
        Ok(response) => Some(format!("HTTP {}", response.status())),
        Err(error) => match head_error {
            Some(head_error) => Some(format!("{head_error}; fallback GET failed: {error}")),
            None => Some(error.to_string()),
        },
    }
}

fn link_target_exists(candidate: &Path) -> bool {
    if candidate.exists() {
        return true;
    }
    if candidate
        .extension()
        .and_then(|extension| extension.to_str())
        == Some("html")
    {
        let mut typ_source = candidate.to_path_buf();
        typ_source.set_extension("typ");
        return typ_source.is_file();
    }
    if candidate.extension().is_none() {
        return candidate.join("index.typ").is_file() || candidate.join("index.html").is_file();
    }
    false
}

fn is_external_or_special_link(target: &str) -> bool {
    target.starts_with('#')
        || target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("//")
        || target.starts_with("mailto:")
        || target.starts_with("tel:")
        || target.starts_with("data:")
}

fn parse_string_literal(source: &str, quote: usize) -> Option<(String, usize)> {
    let mut out = String::new();
    let mut escaped = false;
    let mut index = quote + 1;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        if escaped {
            out.push(match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some((out, index + ch.len_utf8()));
        } else {
            out.push(ch);
        }
        index += ch.len_utf8();
    }
    None
}

fn skip_ws(value: &str, mut index: usize) -> usize {
    while index < value.len() {
        let Some(ch) = value[index..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'
}

fn line_number(source: &str, offset: usize) -> usize {
    source[..offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

fn display_rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_literal_typst_links() {
        let links = extract_literal_links(
            Path::new("doc.typ"),
            r#"
#link("guide.html")[Guide]
#link(dynamic)[Dynamic]
#link(
  "../assets/logo.svg"
)[Logo]
"#,
        );

        assert_eq!(
            links
                .iter()
                .map(|link| (link.line, link.target.as_str()))
                .collect::<Vec<_>>(),
            vec![(2, "guide.html"), (4, "../assets/logo.svg")]
        );
    }

    #[test]
    fn link_check_accepts_html_target_with_typst_source() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("index.typ"), r#"#link("guide.html")[Guide]"#).unwrap();
        fs::write(root.join("guide.typ"), "= Guide\n").unwrap();

        let summary = check_links(root, None, false).unwrap();

        assert_eq!(summary.links, 1);
        assert!(summary.broken.is_empty());
    }

    #[test]
    fn link_check_reports_missing_local_targets() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("index.typ"), r#"#link("missing.html")[Missing]"#).unwrap();

        let summary = check_links(root, None, false).unwrap();

        assert_eq!(summary.links, 1);
        assert_eq!(summary.broken.len(), 1);
        assert!(summary.broken[0].contains("index.typ:1"));
        assert!(summary.broken[0].contains("missing.html"));
    }

    #[test]
    fn link_check_skips_external_links_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(
            root.join("index.typ"),
            r#"#link("https://example.com")[External]"#,
        )
        .unwrap();

        let summary = check_links(root, None, false).unwrap();

        assert_eq!(summary.links, 1);
        assert!(summary.broken.is_empty());
        assert_eq!(summary.broken_local, 0);
        assert_eq!(summary.broken_external, 0);
    }

    #[test]
    fn link_check_reports_reachable_external_links_when_enabled() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 512];
                let _ = stream.read(&mut buf);
                let response =
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
                let _ = stream.write_all(response);
            }
        });

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(
            root.join("index.typ"),
            format!(r#"#link("http://{}/ok")[External]"#, addr),
        )
        .unwrap();

        let summary = check_links(root, None, true).unwrap();
        let _ = handle.join();

        assert_eq!(summary.links, 1);
        assert!(summary.broken.is_empty());
        assert_eq!(summary.broken_local, 0);
        assert_eq!(summary.broken_external, 0);
    }

    #[test]
    fn link_check_respects_depth_limit() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("index.typ"), "= Root").unwrap();
        let nested = root.join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("sub.typ"), r#"#link("missing.typ")[Missing]"#).unwrap();

        let summary = check_links(root, Some(0), false).unwrap();
        assert_eq!(summary.files, 1);
        assert_eq!(summary.links, 0);
        assert!(summary.broken.is_empty());

        let summary = check_links(root, Some(1), false).unwrap();
        assert_eq!(summary.files, 2);
        assert_eq!(summary.links, 1);
        assert_eq!(summary.broken_local, 1);
        assert_eq!(summary.broken.len(), 1);
        assert!(summary.broken[0].contains("nested/sub.typ"));
    }
}
