// Book builds: render the master file from the page tree.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::context::PageNode;

/// Render the book master file from the page tree.
/// Fragment files are already written; this produces the master file
/// that references them via \include{} or equivalent.
///
/// Build happens in a temporary directory so intermediate files (.aux,
/// .log, .toc, fragment .tex) don't pollute the output. Only the final
/// PDF is copied to `output`.
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

    // Render page titles through the writer so special characters are escaped
    // (e.g., "&" -> "\&" in LaTeX).
    let escaped_pages: Vec<super::context::PageNode> = page_tree.iter().map(|node| {
        let mut n = node.clone();
        n.title = crate::render::convert::render_inline(&n.title, writer);
        n.children = n.children.iter().map(|c| {
            let mut c = c.clone();
            c.title = crate::render::convert::render_inline(&c.title, writer);
            c
        }).collect();
        n
    }).collect();

    let ctx = minijinja::context! {
        cfg => minijinja::Value::from_serialize(&cfg_map),
        pages => escaped_pages,
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

    let master_name = format!("book.{}", ext);
    let output_filename = format!("book.{}", target.output_extension());

    // Create a temporary build directory. Copy fragments and figures
    // there, write the master file, run post commands, then copy only
    // the final output (e.g. book.pdf) back to `output`.
    let build_dir = tempfile::tempdir()
        .context("Failed to create temporary build directory")?;
    let build = build_dir.path();

    // Copy all rendered fragments and figure directories from output to build dir
    copy_tree(output, build)?;

    // Write the master file into the build dir
    let master_path = build.join(&master_name);
    fs::write(&master_path, &rendered)?;

    if !quiet {
        eprintln!("  Master: {}", master_name);
    }

    // Run post commands if configured (e.g., "latexmk -pdf book.tex")
    let cmds = &target.post;
    if !cmds.is_empty() {
        for cmd in cmds {
            let expanded = cmd
                .replace("{input}", &master_name)
                .replace("{output}", &output_filename)
                .replace("{root}", &base_dir.display().to_string());

            if !quiet {
                eprintln!("  \x1b[36mpost:\x1b[0m {}", expanded);
            }

            let texinputs = format!("{}:{}:", build.display(), base_dir.display());
            let child = std::process::Command::new("sh")
                .arg("-c")
                .arg(&expanded)
                .current_dir(build)
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

        // Copy the final output file back
        let built_file = build.join(&output_filename);
        if built_file.exists() {
            let dest = output.join(&output_filename);
            fs::copy(&built_file, &dest)
                .with_context(|| format!("Failed to copy {} to {}", built_file.display(), dest.display()))?;
            if !quiet {
                eprintln!("  Output: {}", dest.display());
            }
        }
    }

    // Clean up fragment files from output (keep only the final PDF and
    // figure directories which may be needed by other targets).
    clean_fragments(output, ext);

    Ok(())
}

/// Recursively copy a directory tree.
fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src)?;
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Remove .tex fragment files and LaTeX auxiliary files from the output
/// directory, leaving only the final PDF and figure directories.
fn clean_fragments(output: &Path, writer_ext: &str) {
    let pattern = format!("*.{}", writer_ext);
    for entry in walkdir::WalkDir::new(output)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext == writer_ext || matches!(ext, "aux" | "log" | "toc" | "out" | "fls" | "fdb_latexmk" | "synctex.gz") {
                let _ = fs::remove_file(path);
            }
        }
        // Also match the glob pattern for the writer extension
        if path.file_name().and_then(|n| n.to_str()).map(|n| n.ends_with(&pattern)).unwrap_or(false) {
            let _ = fs::remove_file(path);
        }
    }
}
