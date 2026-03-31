use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use minijinja::Environment;

/// Initialize MiniJinja by loading template files from `templates/{target_name}/`.
///
/// Files use flat namespacing: `{% extends "base.html" %}`
/// and `{% include "search.html" %}` work by filename alone.
///
/// Returns Ok(None) if no templates are found at all (triggers orchestrator path).
pub fn load_templates(base_dir: &Path, target_name: &str) -> Result<Option<Environment<'static>>> {
    let mut templates: HashMap<String, String> = HashMap::new();

    let templates_dir = crate::paths::templates_dir(base_dir);
    let dir = templates_dir.join(target_name);
    if dir.is_dir() {
        let pattern = dir.join("**").join("*.*");
        let pattern_str = pattern.display().to_string();
        for entry in crate::util::safe_glob(&pattern_str) {
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
    // Always produces page-relative paths. current_depth is injected
    // per-page via the `_page_depth` context variable.
    env.add_function("link", |path: String, state: &minijinja::State| -> String {
        let depth: usize = state.lookup("_page_depth")
            .and_then(|v| v.as_usize())
            .unwrap_or(0);
        crate::utils::links::link(&path, depth)
    });

    Ok(Some(env))
}
