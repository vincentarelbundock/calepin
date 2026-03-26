use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use minijinja::Environment;

use super::icons;

/// Initialize MiniJinja by loading template files from `partials/{target_name}/`.
///
/// Files use flat namespacing: `{% extends "base.html" %}`
/// and `{% include "search.html" %}` work by filename alone.
///
/// Falls back to built-in templates embedded in the binary when no project
/// templates are found, or to fill in templates the project doesn't override.
///
/// Returns Ok(None) if no templates are found at all (triggers orchestrator path).
pub fn init_jinja(base_dir: &Path, target_name: &str) -> Result<Option<Environment<'static>>> {
    let mut templates: HashMap<String, String> = HashMap::new();

    // Load project templates from partials/{target_name}/ (any extension)
    let dir = base_dir.join(format!("partials/{}", target_name));
    if dir.is_dir() {
        let pattern = dir.join("**").join("*.*");
        let pattern_str = pattern.display().to_string();
        for entry in glob::glob(&pattern_str).unwrap_or_else(|_| glob::glob("").unwrap()) {
            if let Ok(path) = entry {
                if !path.is_file() { continue; }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let rel = path.strip_prefix(&dir).unwrap_or(&path);
                    let name = rel.display().to_string();
                    templates.insert(name, content);
                }
            }
        }
    }

    // Fall back to built-in templates for any names not already loaded
    let builtin_path = target_name.to_string();
    if let Some(builtin_dir) = crate::render::elements::BUILTIN_PARTIALS.get_dir(&builtin_path) {
        for file in builtin_dir.files() {
            if let Some(content) = file.contents_utf8() {
                let name = file.path().file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                if !name.is_empty() {
                    templates.entry(name.to_string()).or_insert_with(|| content.to_string());
                }
            }
        }
    }

    if templates.is_empty() {
        return Ok(None);
    }

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

    let sources = Arc::new(templates);
    env.set_loader(move |name: &str| {
        Ok(sources.get(name).cloned())
    });

    Ok(Some(env))
}
