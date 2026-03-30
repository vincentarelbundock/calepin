// Book builds: render the master file from the page tree.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::context::PageNode;

/// Render the book master file from the page tree.
/// Fragment files are already written; this produces the master file
/// that references them via \include{} or equivalent.
pub(crate) fn render_book(
    meta: &crate::config::Metadata,
    page_tree: &[PageNode],
    base_dir: &Path,
    output: &Path,
    format: &str,
    output_ext: &str,
    target_name: &str,
    quiet: bool,
) -> Result<()> {
    // Build template context
    let cfg_ctx = minijinja::context! {
        title => meta.title.clone(),
        subtitle => meta.subtitle.clone(),
        author => { let names = meta.author_names(); if names.is_empty() { None } else { Some(names.join(", ")) } },
        url => meta.url.clone(),
    };

    let var_ctx = crate::config::build_jinja_vars(&meta.var);

    let ctx = minijinja::context! {
        cfg => cfg_ctx,
        var => var_ctx,
        pages => page_tree,
        format => format,
        base => format,
    };

    // Collect template sources for the loader
    let mut templates = std::collections::HashMap::new();

    let dir = crate::paths::templates_dir(&base_dir).join(target_name);
    for dir in &[dir] {
        if !dir.is_dir() { continue; }
        let pattern = dir.join("**").join("*.*");
        let pattern_str = pattern.display().to_string();
        for entry in crate::util::safe_glob(&pattern_str) {
            if let Ok(path) = entry {
                if !path.is_file() { continue; }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let rel = path.strip_prefix(dir).unwrap_or(&path);
                    let name = rel.display().to_string();
                    templates.entry(name).or_insert(content);
                }
            }
        }
    }

    // Load the book page template (built-in or sidecar override)
    let ext = crate::paths::resolve_extension(format);
    let page_tpl_name = format!("page.{}", ext);
    if !templates.contains_key(&page_tpl_name) {
        // Fall back to built-in {target}/page template
        let builtin_path = format!("{}/page.{}", target_name, ext);
        if let Some(file) = crate::render::elements::BUILTIN_TEMPLATES.get_file(&builtin_path) {
            if let Some(s) = file.contents_utf8() {
                templates.insert(page_tpl_name.clone(), s.to_string());
            }
        }
    }

    // Load built-in preamble if not overridden
    let preamble_name = format!("preamble.{}", ext);
    if !templates.contains_key(&preamble_name) {
        let builtin_path = format!("{}/preamble.{}", target_name, ext);
        if let Some(file) = crate::render::elements::BUILTIN_TEMPLATES.get_file(&builtin_path) {
            if let Some(s) = file.contents_utf8() {
                templates.insert(preamble_name, s.to_string());
            }
        }
    }

    let mut env = minijinja::Environment::new();
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);
    let sources = std::sync::Arc::new(templates);
    env.set_loader(move |name: &str| {
        Ok(sources.get(name).cloned())
    });

    let tpl = env.get_template(&page_tpl_name)?;
    let rendered = tpl.render(&ctx)
        .with_context(|| format!("Failed to render book template for target {}", target_name))?;

    // Write the master file
    let master_name = format!("book.{}", output_ext);
    let master_path = output.join(&master_name);
    fs::write(&master_path, &rendered)?;

    if !quiet {
        eprintln!("  Master: {}", master_path.display());
    }

    // Run post commands if configured (e.g., "typst compile {input}")
    let target_def = meta.targets.get(target_name);
    let post_cmds = target_def.map(|t| &t.post);

    if let Some(cmds) = post_cmds {
        if !cmds.is_empty() {
            let target_ext = target_def.map(|t| t.output_extension()).unwrap_or("pdf");
            let output_filename = format!("book.{}", target_ext);

            for cmd in cmds {
                let expanded = cmd
                    .replace("{input}", &master_name)
                    .replace("{output}", &output_filename)
                    .replace("{root}", &base_dir.display().to_string());

                if !quiet {
                    eprintln!("  \x1b[36mpost:\x1b[0m {}", expanded);
                }

                let texinputs = format!(
                    "{}:{}:",
                    output.display(),
                    base_dir.display(),
                );
                let status = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&expanded)
                    .current_dir(output)
                    .env("TEXINPUTS", &texinputs)
                    .status()
                    .with_context(|| format!("Failed to run post command: {}", expanded))?;

                if !status.success() {
                    anyhow::bail!("Post command failed: {}", expanded);
                }
            }

            if !quiet {
                eprintln!("  Output: {}", output.join(&output_filename).display());
            }
        }
    }

    Ok(())
}
