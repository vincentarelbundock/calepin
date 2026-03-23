use std::path::Path;

use anyhow::{Context, Result};
use minijinja::Environment;

use super::icons;

/// Initialize MiniJinja by loading template files from `templates/{target_name}/`.
///
/// Files use flat namespacing: `{% extends "base.html" %}`
/// and `{% include "search.html" %}` work by filename alone.
///
/// Falls back to built-in templates embedded in the binary when no project
/// templates are found, or to fill in templates the project doesn't override.
///
/// Returns Ok(None) if no templates are found at all (triggers orchestrator path).
pub fn init_jinja(base_dir: &Path, target_name: &str) -> Result<Option<Environment<'static>>> {
    let mut env = Environment::new();

    // Disable auto-escaping -- calepin output is trusted
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);

    // Register custom Jinja function for icons
    env.add_function("icon", |kwargs: minijinja::value::Kwargs| -> Result<minijinja::Value, minijinja::Error> {
        let name: &str = kwargs.get("name")
            .map_err(|_| minijinja::Error::new(minijinja::ErrorKind::MissingArgument, "icon() requires a 'name' argument"))?;
        kwargs.assert_all_used()?;
        Ok(minijinja::Value::from_safe_string(icons::resolve_icon_svg(name)))
    });

    let mut count = 0;

    // Load project templates from templates/{target_name}/ (any extension)
    let dir = base_dir.join(format!("templates/{}", target_name));
    if dir.is_dir() {
        let pattern = dir.join("**").join("*.*");
        let pattern_str = pattern.display().to_string();
        for entry in glob::glob(&pattern_str).unwrap_or_else(|_| glob::glob("").unwrap()) {
            if let Ok(path) = entry {
                if !path.is_file() { continue; }
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read template: {}", path.display()))?;
                let rel = path.strip_prefix(&dir).unwrap_or(&path);
                let name = rel.display().to_string();
                let content: &'static str = Box::leak(content.into_boxed_str());
                let name: &'static str = Box::leak(name.into_boxed_str());
                env.add_template(name, content)
                    .with_context(|| format!("Failed to parse template: {}", rel.display()))?;
                count += 1;
            }
        }
    }

    // Fall back to built-in templates for any names not already loaded
    let builtin_path = format!("templates/{}", target_name);
    if let Some(builtin_dir) = crate::render::elements::BUILTIN_PROJECT.get_dir(&builtin_path) {
        for file in builtin_dir.files() {
            if let Some(content) = file.contents_utf8() {
                let name = file.path().file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                if name.is_empty() { continue; }
                // Skip if already loaded from project (project wins)
                if env.get_template(name).is_ok() { continue; }
                let content: &'static str = Box::leak(content.to_string().into_boxed_str());
                let name: &'static str = Box::leak(name.to_string().into_boxed_str());
                let _ = env.add_template(name, content);
                count += 1;
            }
        }
    }

    if count == 0 {
        return Ok(None);
    }

    Ok(Some(env))
}
