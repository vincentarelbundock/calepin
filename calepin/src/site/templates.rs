use std::path::Path;

use anyhow::{Context, Result};
use minijinja::Environment;

use super::icons;

// Built-in templates embedded at compile time
const BASE_HTML: &str = include_str!("built_in/templates/base.html");
const PAGE_HTML: &str = include_str!("built_in/templates/page.html");
const LISTING_HTML: &str = include_str!("built_in/templates/listing.html");
const NAVBAR_HTML: &str = include_str!("built_in/templates/navbar.html");
const SIDEBAR_LEFT_HTML: &str = include_str!("built_in/templates/sidebar_left.html");
const SIDEBAR_RIGHT_HTML: &str = include_str!("built_in/templates/sidebar_right.html");
const SEARCH_HTML: &str = include_str!("built_in/templates/search.html");

/// Initialize MiniJinja with built-in templates, then overlay user templates from `_templates/`.
pub fn init_jinja(base_dir: &Path) -> Result<Environment<'static>> {
    let mut env = Environment::new();

    // Disable auto-escaping — calepin output is trusted HTML
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);

    // Register built-in templates
    env.add_template("base.html", BASE_HTML)
        .context("Failed to register base.html")?;
    env.add_template("page.html", PAGE_HTML)
        .context("Failed to register page.html")?;
    env.add_template("listing.html", LISTING_HTML)
        .context("Failed to register listing.html")?;
    env.add_template("navbar.html", NAVBAR_HTML)
        .context("Failed to register navbar.html")?;
    env.add_template("sidebar_left.html", SIDEBAR_LEFT_HTML)
        .context("Failed to register sidebar_left.html")?;
    env.add_template("sidebar_right.html", SIDEBAR_RIGHT_HTML)
        .context("Failed to register sidebar_right.html")?;
    env.add_template("search.html", SEARCH_HTML)
        .context("Failed to register search.html")?;

    // Register custom Jinja function for icons
    env.add_function("icon", |kwargs: minijinja::value::Kwargs| -> Result<minijinja::Value, minijinja::Error> {
        let name: &str = kwargs.get("name")
            .map_err(|_| minijinja::Error::new(minijinja::ErrorKind::MissingArgument, "icon() requires a 'name' argument"))?;
        kwargs.assert_all_used()?;
        Ok(minijinja::Value::from_safe_string(icons::get_icon_svg(name)))
    });

    // Overlay user templates from _templates/
    let user_templates = base_dir.join("_templates");
    if user_templates.is_dir() {
        let pattern = user_templates.join("**").join("*.html");
        let pattern_str = pattern.display().to_string();

        for entry in glob::glob(&pattern_str).unwrap_or_else(|_| glob::glob("").unwrap()) {
            if let Ok(path) = entry {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    // Compute template name relative to _templates/
                    let rel = path.strip_prefix(&user_templates)
                        .unwrap_or(&path);
                    let name = rel.display().to_string();
                    // Leak the content string so it lives for 'static
                    let content: &'static str = Box::leak(content.into_boxed_str());
                    if let Err(e) = env.add_template(Box::leak(name.into_boxed_str()), content) {
                        eprintln!("Warning: failed to parse user template {}: {}", rel.display(), e);
                    }
                }
            }
        }
    }

    Ok(env)
}
