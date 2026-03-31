use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use minijinja::Environment;

/// Initialize MiniJinja by loading template files from `templates/{target_name}/`.
///
/// If a sidecar exists and has a templates directory, loads ONLY from there.
/// If no sidecar templates exist, loads ONLY from built-in templates.
/// No mixing/layering between the two sources.
///
/// Files use flat namespacing: `{% extends "base.html" %}`
/// and `{% include "search.html" %}` work by filename alone.
///
/// Returns Ok(None) if no templates are found at all (triggers orchestrator path).
pub fn load_templates(base_dir: &Path, target_name: &str) -> Result<Option<Environment<'static>>> {
    let mut templates: HashMap<String, String> = HashMap::new();

    // Check if sidecar has a templates directory for this target
    let templates_dir = crate::paths::templates_dir(base_dir);
    let sidecar_tpl_dir = templates_dir.join(target_name);
    let sidecar_has_templates = sidecar_tpl_dir.is_dir();

    if sidecar_has_templates {
        // Sidecar exists with templates: load ONLY from filesystem
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
    } else {
        // No sidecar templates: load ONLY from built-in
        let chain = crate::paths::get_active_inheritance_chain();
        for chain_target in chain.iter().rev() {
            if let Some(dir) = crate::render::elements::BUILTIN_TEMPLATES.get_dir(chain_target.as_str()) {
                let prefix = std::path::Path::new(chain_target.as_str());
                load_builtin_dir(dir, prefix, &mut templates);
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

/// Recursively load templates from a built-in include_dir, stripping the prefix.
fn load_builtin_dir(
    dir: &include_dir::Dir<'static>,
    prefix: &std::path::Path,
    templates: &mut HashMap<String, String>,
) {
    for file in dir.files() {
        let rel = file.path().strip_prefix(prefix).unwrap_or(file.path());
        let name = rel.display().to_string();
        if let Some(content) = file.contents_utf8() {
            templates.insert(name, content.to_string());
        }
    }
    for subdir in dir.dirs() {
        load_builtin_dir(subdir, prefix, templates);
    }
}
