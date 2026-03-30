use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use minijinja::Environment;

use crate::utils::links::UrlMode;

/// Initialize MiniJinja by loading template files from `templates/{target_name}/`.
///
/// Files use flat namespacing: `{% extends "base.html" %}`
/// and `{% include "search.html" %}` work by filename alone.
///
/// Uses layered resolution: user templates override built-in templates.
/// Missing user templates fall through to built-in defaults.
///
/// Returns Ok(None) if no templates are found at all (triggers orchestrator path).
pub fn load_templates_with_url(base_dir: &Path, target_name: &str, base_path: &str, url_mode: UrlMode) -> Result<Option<Environment<'static>>> {
    let mut templates: HashMap<String, String> = HashMap::new();

    // 1. Load user templates (take priority)
    let user_templates_dir = crate::paths::templates_dir(base_dir);
    let dir = user_templates_dir.join(target_name);
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

    // 2. Load built-in templates as fallback (entry() won't overwrite user templates)
    let builtin_path = target_name.to_string();
    if let Some(builtin_dir) = crate::render::elements::BUILTIN_TEMPLATES.get_dir(&builtin_path) {
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

    // Register link(path) function for templates.
    // The base_path and url_mode are captured at load time; current_depth
    // is injected per-page via the `_page_depth` context variable.
    let bp = base_path.to_string();
    env.add_function("link", move |path: String, state: &minijinja::State| -> String {
        let depth: usize = state.lookup("_page_depth")
            .and_then(|v| v.as_usize())
            .unwrap_or(0);
        crate::utils::links::link(&path, &bp, url_mode, depth)
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
