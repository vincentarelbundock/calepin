use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};

use crate::utils::http::timeout_agent;

use super::source::{
    collect_typst_files, display_rel, is_identifier_char, line_number, mask_raw_spans,
    parse_string_literal, resolve_local_reference_target, skip_ws,
};
use super::{HealthCheck, HealthStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
struct LinkOccurrence {
    source: PathBuf,
    line: usize,
    target: String,
}

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
        let source = mask_raw_spans(&source);
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
    let candidate = resolve_local_reference_target(root, &link.source, &link.target)?;
    let candidate = match candidate {
        Ok(candidate) => candidate,
        Err(_) => {
            return Some(format!(
                "{}:{} `{}` escapes the project root",
                display_rel(root, &link.source),
                link.line,
                link.target
            ));
        }
    };
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
        Ok(response) if response.status().as_u16() < 400 => None,
        Ok(response) => {
            fallback_get_error(client, url, Some(format!("HTTP {}", response.status().as_u16())))
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
        Ok(response) if response.status().as_u16() < 400 => None,
        Ok(response) => Some(format!("HTTP {}", response.status().as_u16())),
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
    fn link_check_ignores_links_inside_raw_text() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(
            root.join("index.typ"),
            r#"`#link("missing.html")`

```typ
#link("also-missing.html")
```
"#,
        )
        .unwrap();

        let summary = check_links(root, None, false).unwrap();

        assert_eq!(summary.links, 0);
        assert!(summary.broken.is_empty());
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
