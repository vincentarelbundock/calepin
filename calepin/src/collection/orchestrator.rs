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
    writer: &str,
    target_name: &str,
    target: &crate::config::Target,
    quiet: bool,
) -> Result<()> {
    // Build template context
    let authors: Vec<std::collections::BTreeMap<&str, &str>> = meta.authors.iter()
        .map(|a| std::collections::BTreeMap::from([("name", a.name.literal.as_str())]))
        .collect();
    let mut cfg_map: std::collections::BTreeMap<String, minijinja::Value> = std::collections::BTreeMap::new();
    let all_cfg = crate::config::build_jinja_vars(&meta.cfg);
    if let Ok(iter) = all_cfg.try_iter() {
        for key in iter {
            let key_str = key.to_string();
            if let Ok(val) = all_cfg.get_attr(&key_str) {
                cfg_map.insert(key_str, val);
            }
        }
    }
    if let Some(ref t) = meta.title { cfg_map.insert("title".into(), minijinja::Value::from(t.clone())); }
    if let Some(ref s) = meta.subtitle { cfg_map.insert("subtitle".into(), minijinja::Value::from(s.clone())); }
    if let Some(ref u) = meta.url { cfg_map.insert("url".into(), minijinja::Value::from(u.clone())); }
    cfg_map.insert("authors".into(), minijinja::Value::from_serialize(&authors));

    let ctx = minijinja::context! {
        cfg => minijinja::Value::from_serialize(&cfg_map),
        pages => page_tree,
        writer => writer,
        base => writer,
    };

    // Load templates (reuse shared loader)
    let env = super::templates::load_templates(base_dir, target_name)?
        .ok_or_else(|| anyhow::anyhow!(
            "No template files found for target '{}'. Book builds require a main template.",
            target_name
        ))?;

    let ext = crate::paths::resolve_extension(writer);
    let main_tpl_name = format!("main.{}", ext);
    let tpl = env.get_template(&main_tpl_name)?;
    let rendered = tpl.render(&ctx)
        .with_context(|| format!("Failed to render book template for target {}", target_name))?;

    // Write the master file using the writer extension (.typ/.tex),
    // not the final output extension (.pdf).
    let master_name = format!("book.{}", ext);
    let master_path = output.join(&master_name);
    fs::write(&master_path, &rendered)?;

    if !quiet {
        eprintln!("  Master: {}", master_path.display());
    }

    // Run post commands if configured (e.g., "typst compile {input}")
    let cmds = &target.post;
    {
        if !cmds.is_empty() {
            let output_filename = format!("book.{}", target.output_extension());

            for cmd in cmds {
                let expanded = cmd
                    .replace("{input}", &master_name)
                    .replace("{output}", &output_filename)
                    .replace("{root}", &base_dir.display().to_string());

                if !quiet {
                    eprintln!("  \x1b[36mpost:\x1b[0m {}", expanded);
                }

                let texinputs = format!("{}:{}:", output.display(), base_dir.display());
                let child = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&expanded)
                    .current_dir(output)
                    .env("TEXINPUTS", &texinputs)
                    .stdout(std::process::Stdio::inherit())
                    .stderr(std::process::Stdio::inherit())
                    .status()
                    .with_context(|| format!("Failed to run post command: {}", expanded))?;

                if !child.success() {
                    anyhow::bail!("Post command failed (exit {}): {}",
                        child.code().unwrap_or(-1), expanded);
                }
            }

            if !quiet {
                eprintln!("  Output: {}", output.join(&output_filename).display());
            }
        }
    }

    Ok(())
}
