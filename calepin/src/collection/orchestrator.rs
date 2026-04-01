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
    let authors: Vec<std::collections::BTreeMap<&str, &str>> = meta.authors.iter()
        .map(|a| std::collections::BTreeMap::from([("name", a.name.literal.as_str())]))
        .collect();
    let mut cfg_map: std::collections::BTreeMap<String, minijinja::Value> = std::collections::BTreeMap::new();
    // All config.toml keys
    let all_cfg = crate::config::build_jinja_vars(&meta.cfg);
    if let Ok(iter) = all_cfg.try_iter() {
        for key in iter {
            let key_str = key.to_string();
            if let Ok(val) = all_cfg.get_attr(&key_str) {
                cfg_map.insert(key_str, val);
            }
        }
    }
    // Standard fields (override if present)
    if let Some(ref t) = meta.title { cfg_map.insert("title".into(), minijinja::Value::from(t.clone())); }
    if let Some(ref s) = meta.subtitle { cfg_map.insert("subtitle".into(), minijinja::Value::from(s.clone())); }
    if let Some(ref u) = meta.url { cfg_map.insert("url".into(), minijinja::Value::from(u.clone())); }
    cfg_map.insert("authors".into(), minijinja::Value::from_serialize(&authors));

    let ctx = minijinja::context! {
        cfg => minijinja::Value::from_serialize(&cfg_map),
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

    // Templates are loaded from the sidecar (always populated)
    let ext = crate::paths::resolve_extension(format);
    let main_tpl_name = format!("main.{}", ext);

    let mut env = minijinja::Environment::new();
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);
    let sources = std::sync::Arc::new(templates);
    env.set_loader(move |name: &str| {
        Ok(sources.get(name).cloned())
    });

    let tpl = env.get_template(&main_tpl_name)?;
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
