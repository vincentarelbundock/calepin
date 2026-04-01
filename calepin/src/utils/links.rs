//! URL resolution helpers for site builds.
//!
//! All internal URLs flow through `link()` which produces root-relative
//! paths (prefixed with `base_path`, defaulting to `/`). `canonical_url()`
//! produces absolute URLs for meta tags and feeds.

#[allow(dead_code)]
/// Extract the path component from a site URL.
///
/// ```text
/// "https://example.com/docs/" -> "/docs/"
/// "https://example.com"       -> "/"
/// "/docs/"                    -> "/docs/"
/// ```
pub fn extract_base_path(url: Option<&str>) -> &str {
    let Some(url) = url else { return "/" };
    let url = url.trim();
    if url.is_empty() { return "/"; }

    // If it looks like a full URL, extract the path
    if let Some(rest) = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")) {
        // Find the first `/` after the host
        if let Some(slash) = rest.find('/') {
            let path = &rest[slash..];
            if path.is_empty() { return "/"; }
            return path;
        }
        return "/";
    }

    // Already a path
    if url.starts_with('/') {
        return url;
    }

    "/"
}

#[allow(dead_code)]
/// Normalize a base path: ensure it starts and ends with `/`.
pub fn normalize_base_path(path: &str) -> String {
    let path = path.trim().trim_matches('/');
    if path.is_empty() {
        "/".to_string()
    } else {
        format!("/{}/", path)
    }
}

/// Resolve an internal site path to a root-relative path.
///
/// - `path`: site-root-relative path (e.g., `assets/calepin.css`, `/guide/intro.html`)
/// - `base_path`: site base path (e.g., `/`, `/docs/`)
///
/// External URLs (http://, https://, #, data:, mailto:) pass through unchanged.
pub fn link(path: &str, base_path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://")
        || path.starts_with('#') || path.starts_with("data:")
        || path.starts_with("mailto:")
    {
        return path.to_string();
    }

    let path = path.strip_prefix('/').unwrap_or(path);
    let base = if base_path.is_empty() { "/" } else { base_path };

    if path.is_empty() {
        base.to_string()
    } else {
        format!("{}{}", base, path)
    }
}

/// Build an absolute canonical URL from a site URL and a path.
///
/// Used for `<link rel="canonical">`, Open Graph tags, sitemaps, and feeds.
/// Returns `None` if no site URL is configured.
#[allow(dead_code)]
pub fn canonical_url(path: &str, site_url: Option<&str>) -> Option<String> {
    let site_url = site_url?.trim().trim_end_matches('/');
    if site_url.is_empty() { return None; }
    let path = path.strip_prefix('/').unwrap_or(path);
    Some(format!("{}/{}", site_url, path))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_base_path() {
        assert_eq!(extract_base_path(Some("https://example.com/docs/")), "/docs/");
        assert_eq!(extract_base_path(Some("https://example.com")), "/");
        assert_eq!(extract_base_path(Some("https://example.com/")), "/");
        assert_eq!(extract_base_path(Some("/docs/")), "/docs/");
        assert_eq!(extract_base_path(None), "/");
        assert_eq!(extract_base_path(Some("")), "/");
    }

    #[test]
    fn test_normalize_base_path() {
        assert_eq!(normalize_base_path("/docs/"), "/docs/");
        assert_eq!(normalize_base_path("/docs"), "/docs/");
        assert_eq!(normalize_base_path("docs"), "/docs/");
        assert_eq!(normalize_base_path("/"), "/");
        assert_eq!(normalize_base_path(""), "/");
    }

    #[test]
    fn test_link_root_relative() {
        assert_eq!(link("assets/calepin.css", "/"), "/assets/calepin.css");
        assert_eq!(link("/assets/calepin.css", "/"), "/assets/calepin.css");
        assert_eq!(link("guide/intro.html", "/"), "/guide/intro.html");
        assert_eq!(link("index.html", "/"), "/index.html");
    }

    #[test]
    fn test_link_with_base_path() {
        assert_eq!(link("assets/calepin.css", "/docs/"), "/docs/assets/calepin.css");
        assert_eq!(link("/assets/calepin.css", "/docs/"), "/docs/assets/calepin.css");
        assert_eq!(link("index.html", "/docs/"), "/docs/index.html");
    }

    #[test]
    fn test_link_passthrough() {
        assert_eq!(link("https://cdn.example.com/lib.js", "/"), "https://cdn.example.com/lib.js");
        assert_eq!(link("#section", "/"), "#section");
        assert_eq!(link("data:image/png;base64,abc", "/"), "data:image/png;base64,abc");
    }

    #[test]
    fn test_canonical_url() {
        assert_eq!(canonical_url("guide/intro.html", Some("https://example.com/docs")), Some("https://example.com/docs/guide/intro.html".to_string()));
        assert_eq!(canonical_url("/guide/intro.html", Some("https://example.com")), Some("https://example.com/guide/intro.html".to_string()));
        assert_eq!(canonical_url("index.html", None), None);
    }

#[test]
    fn test_link_empty_path() {
        assert_eq!(link("", "/"), "/");
        assert_eq!(link("", "/docs/"), "/docs/");
    }
}
