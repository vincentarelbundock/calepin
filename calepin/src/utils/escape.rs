/// Format-specific escaping for code strings.
pub fn escape_code_for_format(s: &str, format: &str) -> String {
    match format {
        "typst" => s.replace('\\', "\\\\").replace('"', "\\\""),
        _ => s.to_string(),
    }
}
