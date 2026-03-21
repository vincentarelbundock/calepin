use std::path::Path;

use anyhow::{Context, Result};
use tera::Tera;

use super::icons;

// Built-in templates embedded at compile time
const BASE_HTML: &str = include_str!("built_in/templates/base.html");
const PAGE_HTML: &str = include_str!("built_in/templates/page.html");
const LISTING_HTML: &str = include_str!("built_in/templates/listing.html");
const NAVBAR_HTML: &str = include_str!("built_in/templates/navbar.html");
const SIDEBAR_LEFT_HTML: &str = include_str!("built_in/templates/sidebar_left.html");
const SIDEBAR_RIGHT_HTML: &str = include_str!("built_in/templates/sidebar_right.html");
const SEARCH_HTML: &str = include_str!("built_in/templates/search.html");

/// Initialize Tera with built-in templates, then overlay user templates from `_templates/`.
pub fn init_tera(base_dir: &Path) -> Result<Tera> {
    let mut tera = Tera::default();

    // Register built-in templates
    tera.add_raw_templates(vec![
        ("base.html", BASE_HTML),
        ("page.html", PAGE_HTML),
        ("listing.html", LISTING_HTML),
        ("navbar.html", NAVBAR_HTML),
        ("sidebar_left.html", SIDEBAR_LEFT_HTML),
        ("sidebar_right.html", SIDEBAR_RIGHT_HTML),
        ("search.html", SEARCH_HTML),
    ])
    .context("Failed to register built-in templates")?;

    // Disable auto-escaping — calepin output is trusted HTML
    tera.autoescape_on(vec![]);

    // Register custom Tera function for icons
    tera.register_function("icon", IconFunction);

    // Overlay user templates from _templates/
    let user_templates = base_dir.join("_templates");
    if user_templates.is_dir() {
        let pattern = user_templates.join("**").join("*.html");
        let pattern_str = pattern.display().to_string();

        match Tera::parse(&pattern_str) {
            Ok(user_tera) => {
                tera.extend(&user_tera)
                    .context("Failed to extend Tera with user templates")?;
            }
            Err(e) => {
                eprintln!("Warning: failed to parse user templates: {}", e);
            }
        }
    }

    Ok(tera)
}

/// Tera function: {{ icon(name="github") }}
struct IconFunction;

impl tera::Function for IconFunction {
    fn call(&self, args: &std::collections::HashMap<String, tera::Value>) -> tera::Result<tera::Value> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("icon() requires a 'name' argument"))?;
        Ok(tera::Value::String(icons::get_icon_svg(name)))
    }

    fn is_safe(&self) -> bool {
        true
    }
}
