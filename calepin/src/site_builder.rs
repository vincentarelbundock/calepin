//! Generic site builder: renders pages via a builder plugin.
//!
//! The host parses the YAML config, renders all `.qmd` pages to HTML, then calls
//! the plugin's `build_site` function once. The plugin returns all files to write
//! and asset copy rules. The host executes them.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::render::template;

/// Convert a saphyr YamlOwned value to a serde_json::Value.
fn yaml_to_json(yaml: &saphyr::YamlOwned) -> serde_json::Value {
    if let Some(s) = yaml.as_str() {
        serde_json::Value::String(s.to_string())
    } else if let Some(b) = yaml.as_bool() {
        serde_json::Value::Bool(b)
    } else if let Some(n) = yaml.as_integer() {
        serde_json::json!(n)
    } else if let Some(f) = yaml.as_floating_point() {
        serde_json::json!(f)
    } else if yaml.is_null() {
        serde_json::Value::Null
    } else if let Some(seq) = yaml.as_sequence() {
        serde_json::Value::Array(seq.iter().map(yaml_to_json).collect())
    } else if let Some(map) = yaml.as_mapping() {
        let obj = map.iter()
            .filter_map(|(k, v)| k.as_str().map(|s| (s.to_string(), yaml_to_json(v))))
            .collect();
        serde_json::Value::Object(obj)
    } else {
        serde_json::Value::Null
    }
}
use crate::website::{
    collect_pages, page_display_title, parse_config, read_page_titles, render_page_bare,
    copy_dir_recursive,
};

// ---------------------------------------------------------------------------
// Protocol types (serialized as JSON to/from the plugin)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct SiteBuildContext {
    config: serde_json::Value,
    pages: Vec<RenderedPage>,
    syntax_css: String,
}

#[derive(Serialize)]
struct RenderedPage {
    stem: String,
    title: String,
    body_html: String,
    raw_source: String,
    is_index: bool,
    vars: HashMap<String, String>,
}

#[derive(Deserialize)]
struct SiteBuildResult {
    scaffold_command: Option<String>,
    output_dir: String,
    cleanup_dirs: Vec<String>,
    files: Vec<SiteFile>,
    copies: Vec<CopyRule>,
}

#[derive(Deserialize)]
struct SiteFile {
    path: String,
    content: String,
}

#[derive(Deserialize)]
struct CopyRule {
    from: String,
    to: Vec<String>,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Build a site from a YAML config file using a builder plugin.
///
/// Returns the output directory path (relative to base_dir).
pub fn build(config_path: &Path, quiet: bool) -> Result<String> {
    let config = parse_config(config_path)?;
    let base_dir = config_path.parent().unwrap_or(Path::new("."));

    // Determine builder plugin name (default: "astro")
    let builder_name = config.builder.as_deref().unwrap_or("astro");

    // Load the builder plugin
    let plugin = crate::plugins::load_plugin(builder_name)
        .with_context(|| format!("Builder plugin '{}' not found", builder_name))?;

    // Require index.qmd
    let index_qmd = base_dir.join("index.qmd");
    if !index_qmd.exists() {
        bail!("index.qmd not found. A website requires an index.qmd file.");
    }

    let all_pages = collect_pages(&config.pages);
    let titles = read_page_titles(&all_pages, base_dir);

    // Render all pages
    let mut rendered_pages = Vec::new();
    let mut last_syntax_css = String::new();

    for flat_page in &all_pages {
        // Skip non-.qmd files (e.g., .pdf) — they are linked in the sidebar
        // but not rendered. The plugin handles copying them via CopyRule.
        if !flat_page.href.ends_with(".qmd") {
            continue;
        }

        let stem = Path::new(&flat_page.href)
            .with_extension("")
            .to_string_lossy()
            .to_string();

        let qmd_path = base_dir.join(&flat_page.href);
        if !qmd_path.exists() {
            cwarn!("Page not found: {}", qmd_path.display());
            continue;
        }

        // We need a virtual output path so figures land in the right place.
        // We don't know output_dir yet from the plugin, but for figure generation
        // we use a temporary path. Figures are generated relative to this.
        let virtual_output = base_dir.join(format!("{}.html", stem));

        let (body, metadata, syntax_css) =
            render_page_bare(&qmd_path, &virtual_output, &config.format_overrides)?;

        let raw_source = fs::read_to_string(&qmd_path)
            .unwrap_or_default();

        let vars = template::build_html_vars(&metadata, &body);

        let title = page_display_title(
            &flat_page.href,
            flat_page.text.as_deref(),
            &titles,
        );

        let is_index = stem == "index";

        rendered_pages.push(RenderedPage {
            stem,
            title,
            body_html: body,
            raw_source,
            is_index,
            vars,
        });

        if !syntax_css.is_empty() {
            last_syntax_css = syntax_css;
        }
    }

    // Build context and call plugin
    let config_json = yaml_to_json(&config.raw_yaml);

    let ctx = SiteBuildContext {
        config: config_json,
        pages: rendered_pages,
        syntax_css: last_syntax_css,
    };

    let ctx_json = serde_json::to_string(&ctx)
        .context("Failed to serialize site build context")?;

    let result_json = plugin
        .call_build_site(&ctx_json)
        .context("Builder plugin returned no result")?;

    let result: SiteBuildResult = serde_json::from_str(&result_json)
        .context("Failed to parse builder plugin result")?;

    let output_dir = base_dir.join(&result.output_dir);

    // Scaffold if needed
    if !output_dir.join("package.json").exists() {
        if let Some(ref cmd) = result.scaffold_command {
            scaffold(base_dir, cmd, &output_dir, quiet)?;
        }
    }

    // Ensure output directory exists
    fs::create_dir_all(&output_dir)?;

    // Clean up directories
    for dir in &result.cleanup_dirs {
        let dir_path = output_dir.join(dir);
        if dir_path.is_dir() {
            let _ = fs::remove_dir_all(&dir_path);
            fs::create_dir_all(&dir_path)?;
        }
    }

    // Write all files
    for file in &result.files {
        let file_path = output_dir.join(&file.path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&file_path, &file.content)
            .with_context(|| format!("Failed to write: {}", file_path.display()))?;

        if !quiet {
            eprintln!("  \u{2192} {}", file_path.display());
        }
    }

    // Execute copy rules
    for rule in &result.copies {
        let src = base_dir.join(&rule.from);
        for dest_rel in &rule.to {
            let dst = output_dir.join(dest_rel);
            if src.is_dir() {
                copy_dir_recursive(&src, &dst)?;
            } else if src.is_file() {
                if let Some(parent) = dst.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&src, &dst)?;
            }
            // Silently skip if source doesn't exist (e.g., no dark logo variant)
        }
    }

    if !quiet {
        eprintln!("\u{2192} {}/", output_dir.display());
        eprintln!(
            "  To build: cd {} && npm install && npm run build",
            output_dir.display()
        );
    }

    Ok(result.output_dir)
}

// ---------------------------------------------------------------------------
// Scaffold
// ---------------------------------------------------------------------------

fn scaffold(base_dir: &Path, command: &str, output_dir: &Path, quiet: bool) -> Result<()> {
    if !quiet {
        eprintln!("Scaffolding project...");
    }

    let cwd = if base_dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        base_dir
    };
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    let output = Command::new(&shell)
        .args(["-lc", command])
        .current_dir(cwd)
        .output()
        .with_context(|| format!("Failed to run scaffold command: {}", command))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Scaffold command failed:\n{}", stderr);
    }

    // Remove .git directory if created by scaffold
    let git_dir = output_dir.join(".git");
    if git_dir.is_dir() {
        let _ = fs::remove_dir_all(&git_dir);
    }

    Ok(())
}
