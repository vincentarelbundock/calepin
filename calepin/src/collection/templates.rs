use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use minijinja::Environment;

/// Initialize MiniJinja by loading template files from `templates/{target_name}/`.
///
/// Loads from the sidecar's filesystem templates (always populated).
///
/// Files use flat namespacing: `{% extends "base.html" %}`
/// and `{% include "search.html" %}` work by filename alone.
///
/// Returns Ok(None) if no templates are found at all (triggers orchestrator path).
pub fn load_templates(base_dir: &Path, target_name: &str) -> Result<Option<Environment<'static>>> {
    let mut templates: HashMap<String, String> = HashMap::new();

    // Load from sidecar templates directory
    let templates_dir = crate::paths::templates_dir(base_dir);
    let sidecar_tpl_dir = templates_dir.join(target_name);

    if sidecar_tpl_dir.is_dir() {
        let pattern = sidecar_tpl_dir.join("**").join("*.*");
        let pattern_str = pattern.display().to_string();
        for entry in crate::util::safe_glob(&pattern_str) {
            if let Ok(path) = entry {
                if !path.is_file() { continue; }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let rel = path.strip_prefix(&sidecar_tpl_dir).unwrap_or(&path);
                    let name = rel.display().to_string();
                    templates.insert(name, content);
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

    let sources = Arc::new(templates);
    env.set_loader(move |name: &str| {
        Ok(sources.get(name).cloned())
    });

    // Register link(path) function for templates.
    // Produces root-relative paths using the base_path from context.
    env.add_function("link", |path: String, state: &minijinja::State| -> String {
        let base_path: String = state.lookup("_base_path")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "/".to_string());
        crate::utils::links::link(&path, &base_path)
    });

    Ok(Some(env))
}
