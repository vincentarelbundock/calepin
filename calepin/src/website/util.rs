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
