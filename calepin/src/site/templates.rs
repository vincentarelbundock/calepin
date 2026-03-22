use std::path::Path;

use anyhow::{Context, Result};
use minijinja::Environment;

use super::icons;

/// Initialize MiniJinja by loading all `.html` files from `_calepin/templates/`.
///
/// There are no built-in templates. The site is fully defined by the templates
/// in the `_calepin/templates/` directory. Users customize the site by editing those files.
pub fn init_jinja(base_dir: &Path) -> Result<Environment<'static>> {
    let mut env = Environment::new();

    // Disable auto-escaping — calepin output is trusted HTML
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);

    // Register custom Jinja function for icons
    env.add_function("icon", |kwargs: minijinja::value::Kwargs| -> Result<minijinja::Value, minijinja::Error> {
        let name: &str = kwargs.get("name")
            .map_err(|_| minijinja::Error::new(minijinja::ErrorKind::MissingArgument, "icon() requires a 'name' argument"))?;
        kwargs.assert_all_used()?;
        Ok(minijinja::Value::from_safe_string(icons::get_icon_svg(name)))
    });

    // Load all templates from _calepin/templates/
    let templates_dir = base_dir.join("_calepin/templates");
    anyhow::ensure!(
        templates_dir.is_dir(),
        "No _calepin/templates/ directory found in {}. Site templates must be provided.",
        base_dir.display()
    );

    let pattern = templates_dir.join("**").join("*.html");
    let pattern_str = pattern.display().to_string();

    let mut count = 0;
    for entry in glob::glob(&pattern_str).unwrap_or_else(|_| glob::glob("").unwrap()) {
        if let Ok(path) = entry {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read template: {}", path.display()))?;
            let rel = path.strip_prefix(&templates_dir).unwrap_or(&path);
            let name = rel.display().to_string();
            // Leak strings so they live for 'static (MiniJinja requirement)
            let content: &'static str = Box::leak(content.into_boxed_str());
            let name: &'static str = Box::leak(name.into_boxed_str());
            env.add_template(name, content)
                .with_context(|| format!("Failed to parse template: {}", rel.display()))?;
            count += 1;
        }
    }

    anyhow::ensure!(
        count > 0,
        "_calepin/templates/ directory is empty. At least base.html and page.html are required."
    );

    Ok(env)
}
