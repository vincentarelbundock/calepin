use std::path::Path;

use anyhow::{Context, Result};
use minijinja::Environment;

use super::icons;

/// Initialize MiniJinja by loading `.html` files from `templates/html/`
/// (page layouts) and `components/html/` (reusable partials).
///
/// Both directories use flat namespacing: `{% extends "base.html" %}`
/// and `{% include "search.html" %}` work regardless of which directory
/// the file came from.
pub fn init_jinja(base_dir: &Path) -> Result<Environment<'static>> {
    let mut env = Environment::new();

    // Disable auto-escaping -- calepin output is trusted HTML
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);

    // Register custom Jinja function for icons
    env.add_function("icon", |kwargs: minijinja::value::Kwargs| -> Result<minijinja::Value, minijinja::Error> {
        let name: &str = kwargs.get("name")
            .map_err(|_| minijinja::Error::new(minijinja::ErrorKind::MissingArgument, "icon() requires a 'name' argument"))?;
        kwargs.assert_all_used()?;
        Ok(minijinja::Value::from_safe_string(icons::get_icon_svg(name)))
    });

    // Load .html files from both templates/html/ and components/html/
    let dirs = [
        base_dir.join("templates/html"),
        base_dir.join("components/html"),
    ];

    let mut count = 0;
    for dir in &dirs {
        if !dir.is_dir() {
            continue;
        }
        let pattern = dir.join("**").join("*.html");
        let pattern_str = pattern.display().to_string();
        for entry in glob::glob(&pattern_str).unwrap_or_else(|_| glob::glob("").unwrap()) {
            if let Ok(path) = entry {
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read template: {}", path.display()))?;
                let rel = path.strip_prefix(dir).unwrap_or(&path);
                let name = rel.display().to_string();
                // Leak strings so they live for 'static (MiniJinja requirement)
                let content: &'static str = Box::leak(content.into_boxed_str());
                let name: &'static str = Box::leak(name.into_boxed_str());
                env.add_template(name, content)
                    .with_context(|| format!("Failed to parse template: {}", rel.display()))?;
                count += 1;
            }
        }
    }

    anyhow::ensure!(
        count > 0,
        "No .html files found in templates/html/ or components/html/. \
         At least base.html and page.html are required."
    );

    Ok(env)
}
