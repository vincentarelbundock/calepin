use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use minijinja::Environment;

/// Initialize MiniJinja by loading template files from `partials/{target_name}/`.
///
/// Files use flat namespacing: `{% extends "base.html" %}`
/// and `{% include "search.html" %}` work by filename alone.
///
/// Falls back to built-in templates embedded in the binary when no project
/// templates are found, or to fill in templates the project doesn't override.
///
/// Returns Ok(None) if no templates are found at all (triggers orchestrator path).
pub fn load_templates(base_dir: &Path, target_name: &str) -> Result<Option<Environment<'static>>> {
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

    // Fall back to built-in templates for any names not already loaded.
    // Recurse into subdirectories (e.g. icons/) preserving relative paths.
    let builtin_path = target_name.to_string();
    if let Some(builtin_dir) = crate::render::elements::BUILTIN_PARTIALS.get_dir(&builtin_path) {
        let prefix = std::path::Path::new(&builtin_path);
        load_builtin_dir_recursive(builtin_dir, prefix, &mut templates);
    }

    if templates.is_empty() {
        return Ok(None);
    }

    let mut env = Environment::new();

    // Disable auto-escaping -- calepin output is trusted
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);

    let sources = Arc::new(templates);
    env.set_loader(move |name: &str| {
        Ok(sources.get(name).cloned())
    });

    Ok(Some(env))
}

/// Recursively load built-in template files, preserving relative paths as template names.
fn load_builtin_dir_recursive(
    dir: &include_dir::Dir<'static>,
    prefix: &std::path::Path,
    templates: &mut HashMap<String, String>,
) {
    for file in dir.files() {
        if let Some(content) = file.contents_utf8() {
            let name = file.path().strip_prefix(prefix)
                .unwrap_or(file.path())
                .display()
                .to_string();
            if !name.is_empty() {
                templates.entry(name).or_insert_with(|| content.to_string());
            }
        }
    }
    for subdir in dir.dirs() {
        load_builtin_dir_recursive(subdir, prefix, templates);
    }
}
