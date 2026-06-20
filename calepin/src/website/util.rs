use std::path::Path;

use crate::utils::html::escape as html_escape;

pub(super) fn xml_escape(value: &str) -> String {
    html_escape(value).replace('\'', "&apos;")
}

pub(super) fn clean_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn absolute_site_url(base_url: &str, href: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let href = href.trim_start_matches('/');
    if href.is_empty() {
        base.to_string()
    } else {
        format!("{base}/{href}")
    }
}

/// True when `value` is a relative route that cannot escape the output
/// directory once joined onto it.
pub(super) fn is_safe_output_route(value: &str) -> bool {
    let path = Path::new(value);
    !path.is_absolute()
        && path.components().all(|component| {
            matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
}

pub(super) fn is_absolute_or_special_url(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with('#')
        || value.starts_with("data:")
        || value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("//")
        || value.starts_with("mailto:")
        || value.starts_with("tel:")
}
