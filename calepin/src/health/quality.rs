use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::typst::paths::slash_path;
use crate::utils::path::normalize_path;

use super::source::{
    collect_typst_files, display_rel, find_matching_delimiter, is_external_or_special_target,
    is_identifier_char, is_left_identifier_boundary, line_number, mask_raw_spans,
    parse_string_literal, skip_ws,
};
use super::{HealthCheck, HealthStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImageOccurrence {
    source: PathBuf,
    line: usize,
    target: String,
    alt: AltState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AltState {
    Present,
    Empty,
    Missing,
}

pub(super) fn image_check(root: &Path, depth: Option<usize>) -> HealthCheck {
    match check_images(root, depth) {
        Ok(ImageSummary {
            files,
            images,
            missing,
            alt_warnings,
            details,
        }) => {
            let status = if missing > 0 {
                HealthStatus::Error
            } else if alt_warnings > 0 {
                HealthStatus::Warning
            } else {
                HealthStatus::Ok
            };
            let message = if missing == 0 && alt_warnings == 0 {
                format!("checked {images} literal image(s) in {files} Typst file(s)")
            } else {
                format!(
                    "{missing} missing image target(s), {alt_warnings} alt text warning(s) among {images} literal image(s)"
                )
            };
            let hint = if missing > 0 {
                Some("fix missing image files or paths".to_string())
            } else if alt_warnings > 0 {
                Some("add non-empty `alt:` text to content images".to_string())
            } else {
                None
            };
            HealthCheck::new("images", status, message)
                .with_path(root.display().to_string())
                .with_optional_hint(hint)
                .with_details(details)
        }
        Err(error) => HealthCheck::warn("images", format!("failed to check images: {error}"))
            .with_path(root.display().to_string()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImageSummary {
    files: usize,
    images: usize,
    missing: usize,
    alt_warnings: usize,
    details: Vec<String>,
}

fn check_images(root: &Path, depth: Option<usize>) -> Result<ImageSummary> {
    let typ_files = collect_typst_files(root, depth)?;
    let mut images = 0usize;
    let mut missing = 0usize;
    let mut alt_warnings = 0usize;
    let mut details = Vec::new();

    for file in &typ_files {
        let source = fs::read_to_string(file)
            .with_context(|| format!("failed to read {}", file.display()))?;
        let source = mask_raw_spans(&source);
        for image in extract_literal_images(file, &source) {
            images += 1;
            if let Some(message) = validate_local_image(root, &image) {
                missing += 1;
                details.push(message);
            }
            match image.alt {
                AltState::Present => {}
                AltState::Empty => {
                    alt_warnings += 1;
                    details.push(format!(
                        "{}:{} image `{}` has empty alt text",
                        display_rel(root, &image.source),
                        image.line,
                        image.target
                    ));
                }
                AltState::Missing => {
                    alt_warnings += 1;
                    details.push(format!(
                        "{}:{} image `{}` is missing alt text",
                        display_rel(root, &image.source),
                        image.line,
                        image.target
                    ));
                }
            }
        }
    }

    Ok(ImageSummary {
        files: typ_files.len(),
        images,
        missing,
        alt_warnings,
        details,
    })
}

fn extract_literal_images(source_path: &Path, source: &str) -> Vec<ImageOccurrence> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while let Some(relative) = source[offset..].find("#image") {
        let start = offset + relative;
        let after_name = start + "#image".len();
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
        let Some(close) = find_matching_delimiter(source, open, '(', ')') else {
            offset = open + 1;
            continue;
        };
        let args = &source[open + 1..close];
        let value_start = skip_ws(args, 0);
        if args[value_start..].starts_with('"') {
            if let Some((target, _end)) = parse_string_literal(args, value_start) {
                out.push(ImageOccurrence {
                    source: source_path.to_path_buf(),
                    line: line_number(source, start),
                    target,
                    alt: alt_argument_state(args),
                });
            }
        }
        offset = close + 1;
    }
    out
}

fn alt_argument_state(args: &str) -> AltState {
    let Some(value_start) = named_argument_value_start(args, "alt") else {
        return AltState::Missing;
    };
    if args[value_start..].starts_with("none") {
        return AltState::Missing;
    }
    if args[value_start..].starts_with('"') {
        return match parse_string_literal(args, value_start) {
            Some((value, _)) if value.trim().is_empty() => AltState::Empty,
            Some(_) => AltState::Present,
            None => AltState::Missing,
        };
    }
    AltState::Present
}

fn validate_local_image(root: &Path, image: &ImageOccurrence) -> Option<String> {
    let target = image.target.trim();
    if target.is_empty() || is_external_or_special_target(target) {
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
        image.source.parent().unwrap_or(root).to_path_buf()
    };
    let candidate = normalize_path(&base.join(path_part.trim_start_matches('/')));
    if !candidate.starts_with(root) {
        return Some(format!(
            "{}:{} image `{}` escapes the project root",
            display_rel(root, &image.source),
            image.line,
            image.target
        ));
    }
    if candidate.is_file() {
        return None;
    }

    Some(format!(
        "{}:{} missing image target `{}`",
        display_rel(root, &image.source),
        image.line,
        image.target
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PageRoute {
    source: PathBuf,
    line: usize,
    key: &'static str,
    value: String,
    href: String,
}

pub(super) fn slug_check(root: &Path, depth: Option<usize>) -> HealthCheck {
    match check_page_routes(root, depth) {
        Ok(RouteSummary {
            files,
            routes,
            problems,
        }) => {
            let status = if problems.is_empty() {
                HealthStatus::Ok
            } else {
                HealthStatus::Error
            };
            let message = if problems.is_empty() {
                format!("checked {routes} explicit page route(s) in {files} Typst file(s)")
            } else {
                format!(
                    "{} duplicate or invalid page route(s) among {routes} explicit page route(s)",
                    problems.len()
                )
            };
            HealthCheck::new("slugs", status, message)
                .with_path(root.display().to_string())
                .with_optional_hint(
                    (!problems.is_empty()).then(|| {
                        "give each page a unique `slug` or `url` output route".to_string()
                    }),
                )
                .with_details(problems)
        }
        Err(error) => HealthCheck::warn("slugs", format!("failed to check slugs: {error}"))
            .with_path(root.display().to_string()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RouteSummary {
    files: usize,
    routes: usize,
    problems: Vec<String>,
}

fn check_page_routes(root: &Path, depth: Option<usize>) -> Result<RouteSummary> {
    let typ_files = collect_typst_files(root, depth)?;
    let mut routes_by_href: BTreeMap<String, Vec<PageRoute>> = BTreeMap::new();
    let mut problems = Vec::new();
    let mut routes = 0usize;

    for file in &typ_files {
        let source = fs::read_to_string(file)
            .with_context(|| format!("failed to read {}", file.display()))?;
        let source = mask_raw_spans(&source);
        for route in extract_page_routes(root, file, &source) {
            routes += 1;
            if !is_safe_output_route(route_value_for_safety(&route)) {
                problems.push(format!(
                    "{}:{} `{}` route `{}` escapes the output directory",
                    display_rel(root, &route.source),
                    route.line,
                    route.key,
                    route.value
                ));
                continue;
            }
            routes_by_href
                .entry(route.href.clone())
                .or_default()
                .push(route);
        }
    }

    for (href, routes) in routes_by_href {
        if routes.len() <= 1 {
            continue;
        }
        let sources = routes
            .iter()
            .map(|route| {
                format!(
                    "{}:{} `{}` = `{}`",
                    display_rel(root, &route.source),
                    route.line,
                    route.key,
                    route.value
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        problems.push(format!("duplicate page route `{href}` from {sources}"));
    }

    Ok(RouteSummary {
        files: typ_files.len(),
        routes,
        problems,
    })
}

fn extract_page_routes(root: &Path, source_path: &Path, source: &str) -> Vec<PageRoute> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while let Some(relative) = source[offset..].find("#metadata") {
        let start = offset + relative;
        let after_name = start + "#metadata".len();
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
        let Some(close) = find_matching_delimiter(source, open, '(', ')') else {
            offset = open + 1;
            continue;
        };
        let label_start = skip_ws(source, close + 1);
        if !source[label_start..].starts_with("<website-metadata>") {
            offset = close + 1;
            continue;
        }
        let args = &source[open + 1..close];
        let line = line_number(source, start);
        if let Some(url) = named_string_argument(args, "url") {
            out.push(PageRoute {
                source: source_path.to_path_buf(),
                line,
                key: "url",
                href: output_href_with_extension(&url, "html"),
                value: url,
            });
        } else if let Some(slug) = named_string_argument(args, "slug") {
            let rel_parent = source_path
                .strip_prefix(root)
                .unwrap_or(source_path)
                .parent()
                .unwrap_or_else(|| Path::new(""));
            let mut href = rel_parent.join(&slug);
            href.set_extension("html");
            out.push(PageRoute {
                source: source_path.to_path_buf(),
                line,
                key: "slug",
                href: slash_path(&href),
                value: slug,
            });
        }
        offset = label_start + "<website-metadata>".len();
    }
    out
}

fn route_value_for_safety(route: &PageRoute) -> &str {
    if route.key == "url" {
        route
            .value
            .trim()
            .trim_start_matches('/')
            .trim_start_matches("./")
    } else {
        route.value.as_str()
    }
}

fn output_href_with_extension(url: &str, extension: &str) -> String {
    let url = url.trim().trim_start_matches('/').trim_start_matches("./");
    if url.ends_with('/') {
        return format!("{url}index.{extension}");
    }
    let mut path = PathBuf::from(url);
    path.set_extension(extension);
    slash_path(&path)
}

fn named_string_argument(args: &str, name: &str) -> Option<String> {
    let value_start = named_argument_value_start(args, name)?;
    if !args[value_start..].starts_with('"') {
        return None;
    }
    parse_string_literal(args, value_start).map(|(value, _)| value)
}

fn named_argument_value_start(args: &str, name: &str) -> Option<usize> {
    let mut offset = 0usize;
    while let Some(relative) = args[offset..].find(name) {
        let start = offset + relative;
        let after_name = start + name.len();
        if is_left_identifier_boundary(args, start)
            && !args[after_name..]
                .chars()
                .next()
                .is_some_and(is_identifier_char)
        {
            let colon = skip_ws(args, after_name);
            if args[colon..].starts_with(':') {
                return Some(skip_ws(args, colon + 1));
            }
        }
        offset = after_name;
    }
    None
}

fn is_safe_output_route(value: &str) -> bool {
    let path = Path::new(value);
    !path.is_absolute()
        && path.components().all(|component| {
            matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_check_reports_missing_targets_and_alt_text() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(
            root.join("index.typ"),
            r#"
#image("missing.png")
#image("present.png", alt: "Diagram")
"#,
        )
        .unwrap();
        fs::write(root.join("present.png"), "").unwrap();

        let summary = check_images(root, None).unwrap();

        assert_eq!(summary.images, 2);
        assert_eq!(summary.missing, 1);
        assert_eq!(summary.alt_warnings, 1);
        assert!(summary
            .details
            .iter()
            .any(|detail| detail.contains("missing.png")));
    }

    #[test]
    fn slug_check_reports_duplicate_output_routes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(
            root.join("one.typ"),
            r#"#metadata((slug: "same")) <website-metadata>"#,
        )
        .unwrap();
        fs::write(
            root.join("two.typ"),
            r#"#metadata((url: "same.html")) <website-metadata>"#,
        )
        .unwrap();

        let summary = check_page_routes(root, None).unwrap();

        assert_eq!(summary.routes, 2);
        assert_eq!(summary.problems.len(), 1);
        assert!(summary.problems[0].contains("duplicate page route `same.html`"));
    }

    #[test]
    fn slug_check_allows_same_slug_in_different_directories() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("blog")).unwrap();
        fs::write(
            root.join("guide.typ"),
            r#"#metadata((slug: "index")) <website-metadata>"#,
        )
        .unwrap();
        fs::write(
            root.join("blog").join("post.typ"),
            r#"#metadata((slug: "index")) <website-metadata>"#,
        )
        .unwrap();

        let summary = check_page_routes(root, None).unwrap();

        assert_eq!(summary.routes, 2);
        assert!(summary.problems.is_empty());
    }
}
