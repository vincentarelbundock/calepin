// Orchestrator builds: LaTeX/Typst book assembly from rendered fragments.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::discover::DocumentInfo;
use super::contents::{DocumentNode, expand_contents};
use super::render;

/// A node in the orchestrator page tree, serializable to MiniJinja.
#[derive(Debug, Clone, serde::Serialize)]
struct OrchestratorNode {
    /// Display title (section title or page title from frontmatter)
    title: String,
    /// Relative path to the fragment file (without extension for LaTeX \include)
    path: Option<String>,
    /// Path with extension
    file: Option<String>,
    /// Child nodes (documents within a section)
    children: Vec<OrchestratorNode>,
}

/// Render the orchestrator template with the document tree.
/// Fragment files are already written; this produces the master file
/// that references them via \include{} or equivalent.
pub(crate) fn render_orchestrator(
    meta: &crate::config::Metadata,
    pages: &[DocumentInfo],
    results: &HashMap<String, render::CollectionRenderResult>,
    base_dir: &Path,
    output: &Path,
    orchestrator_path: &str,
    format: &str,
    output_ext: &str,
    target_name: &str,
    quiet: bool,
) -> Result<()> {
    // Build the document tree with titles and paths
    let document_tree = expand_contents(&meta.contents, base_dir);

    let document_map: HashMap<String, &DocumentInfo> = pages.iter()
        .map(|p| (p.source.display().to_string(), p))
        .collect();

    let nav_nodes = build_orchestrator_tree(&document_tree, &document_map, results, format);

    // Build template context
    let meta_ctx = minijinja::context! {
        title => meta.title.clone(),
        subtitle => meta.subtitle.clone(),
        author => { let names = meta.author_names(); if names.is_empty() { None } else { Some(names.join(", ")) } },
        url => meta.url.clone(),
    };

    let var_ctx = crate::config::build_jinja_vars(&meta.var);

    let label_defs = meta.labels.as_ref();

    let ctx = minijinja::context! {
        meta => meta_ctx,
        var => var_ctx,
        pages => nav_nodes,
        format => format,
        base => format,
        label_contents => label_defs.and_then(|l| l.contents.as_deref()).unwrap_or("Contents"),
    };

    // Collect all template sources into an owned map for the loader.
    let mut templates = std::collections::HashMap::new();

    // Load templates from the flat templates/{target}/ directory
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
                    // Don't overwrite target-specific with common (target loaded first)
                    templates.entry(name).or_insert(content);
                }
            }
        }
    }

    // Load the orchestrator template itself
    let tpl_path = base_dir.join(orchestrator_path);
    let tpl_source = fs::read_to_string(&tpl_path)
        .with_context(|| format!("Failed to read orchestrator template: {}", tpl_path.display()))?;
    templates.insert("orchestrator".to_string(), tpl_source);

    let mut env = minijinja::Environment::new();
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);
    let sources = std::sync::Arc::new(templates);
    env.set_loader(move |name: &str| {
        Ok(sources.get(name).cloned())
    });

    let tpl = env.get_template("orchestrator")?;
    let rendered = tpl.render(&ctx)
        .with_context(|| format!("Failed to render orchestrator template: {}", orchestrator_path))?;

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

                // Run from the output directory (so \include paths resolve),
                // but add both the output dir and the project root to TEXINPUTS
                // so image paths (relative to either) resolve correctly.
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

fn build_orchestrator_tree(
    nodes: &[DocumentNode],
    document_map: &HashMap<String, &DocumentInfo>,
    results: &HashMap<String, render::CollectionRenderResult>,
    format: &str,
) -> Vec<OrchestratorNode> {
    nodes.iter().map(|node| match node {
        DocumentNode::Document { path: source, title: override_title } => {
            let info = document_map.get(source.as_str());
            let title = override_title.clone()
                .or_else(|| results.get(source.as_str()).and_then(|r| r.title.clone()))
                .or_else(|| info.and_then(|p| p.meta.title.clone()))
                .unwrap_or_else(|| source.clone());
            let file = info.map(|p| p.output.display().to_string());
            // For LaTeX \include, strip the extension
            let path = file.as_ref().map(|f| {
                if format == "latex" {
                    f.strip_suffix(".tex").unwrap_or(f).to_string()
                } else {
                    f.clone()
                }
            });
            OrchestratorNode { title, path, file, children: vec![] }
        }
        DocumentNode::Section { title, documents, .. } => {
            OrchestratorNode {
                title: title.clone(),
                path: None,
                file: None,
                children: build_orchestrator_tree(documents, document_map, results, format),
            }
        }
    }).collect()
}
