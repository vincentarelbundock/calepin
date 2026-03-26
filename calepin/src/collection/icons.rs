/// Returns inline SVG for a named icon.
/// Icons are loaded from `assets/website/icons/{name}.svg` (embedded at compile time).
/// Aliases (e.g., "x" -> "twitter", "mail" -> "email") are resolved before lookup.
pub fn resolve_icon_svg(name: &str) -> String {
    let canonical = resolve_alias(name);
    let path = format!("website/icons/{}.svg", canonical);
    crate::render::elements::BUILTIN_ASSETS
        .get_file(&path)
        .and_then(|f| f.contents_utf8())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| format!(r#"<span class="icon-text">[{}]</span>"#, name))
}

fn resolve_alias(name: &str) -> &str {
    match name {
        "x" => "twitter",
        "mail" | "envelope" => "email",
        "website" => "globe",
        other => other,
    }
}
