use std::path::Path;

pub(super) fn absolute_site_url(base_url: &str, href: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let href = href.trim_start_matches('/');
    if href.is_empty() {
        base.to_string()
    } else {
        format!("{base}/{href}")
    }
}

pub(super) fn clean_url_prefix(value: &str) -> String {
    value
        .trim()
        .trim_matches('/')
        .trim_start_matches("./")
        .to_string()
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

pub(super) fn page_relative_url(current_href: &str, target: &str) -> String {
    if is_absolute_or_special_url(target) {
        return target.to_string();
    }

    let target = target.trim_start_matches("./");

    let current_dir = current_href
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let current_dir = current_dir
        .get(..current_dir.len().saturating_sub(1))
        .unwrap_or(&[]);
    let target_parts = target
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let common_len = current_dir
        .iter()
        .zip(target_parts.iter())
        .take_while(|(left, right)| left == right)
        .count();
    let up_levels = current_dir.len().saturating_sub(common_len);
    let remaining_target = target_parts.get(common_len..).unwrap_or(&[]).join("/");

    match (up_levels, remaining_target.is_empty()) {
        (0, false) => remaining_target,
        (0, true) => target.to_string(),
        (_, false) => format!("{}{}", "../".repeat(up_levels), remaining_target),
        (_, true) => "../".repeat(up_levels).trim_end_matches('/').to_string(),
    }
}
