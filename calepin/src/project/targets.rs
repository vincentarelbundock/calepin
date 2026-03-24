//! Target resolution, validation, and inheritance.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Target types
// ---------------------------------------------------------------------------

/// A named output profile.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// Inherit all fields from another target.
    pub inherits: Option<String>,
    /// Rendering engine: html, latex, typst, or markdown.
    #[serde(default, alias = "base")]
    pub engine: String,
    /// Document template name (default: "page").
    pub template: Option<String>,
    /// Output file extension (no dot).
    pub extension: Option<String>,
    /// Default extension for generated figures.
    #[serde(rename = "fig-extension")]
    pub fig_extension: Option<String>,
    /// Preview behavior: "serve", "open", or "none".
    pub preview: Option<String>,
    /// Optional compilation step.
    pub compile: Option<CompileConfig>,
    /// Whether to embed images as base64 data URIs (HTML only).
    #[serde(rename = "embed-resources")]
    pub embed_resources: Option<bool>,
    /// Arbitrary key-value pairs passed to templates as target_vars.
    pub vars: Option<toml::Value>,
    /// Post-processing commands run after rendering.
    #[serde(default)]
    pub post: Vec<String>,
}

/// Compilation configuration (e.g., .tex to .pdf).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompileConfig {
    /// Shell command. {input} and {output} are replaced with file paths.
    pub command: Option<String>,
    /// Extension of the compiled artifact.
    pub extension: Option<String>,
}

impl Target {
    /// Template name. Always set after resolution against the built-in config.
    pub fn template_name(&self) -> &str {
        self.template.as_deref().unwrap_or("page")
    }

    /// Output file extension. Always set after resolution against the built-in config.
    pub fn output_extension(&self) -> &str {
        self.extension.as_deref().unwrap_or(&self.engine)
    }

    /// Default figure extension. Always set after resolution against the built-in config.
    pub fn fig_ext(&self) -> &str {
        self.fig_extension.as_deref().unwrap_or("png")
    }

    /// Preview behavior: "serve", "open", or "none".
    pub fn preview_mode(&self) -> &str {
        self.preview.as_deref().unwrap_or("none")
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

impl Target {
    /// Validate a target's fields. Returns a descriptive error on failure.
    pub fn validate(&self) -> Result<()> {
        match self.engine.as_str() {
            "html" | "latex" | "typst" | "markdown" => {}
            other => bail!("engine must be one of: html, latex, typst, markdown (got '{}')", other),
        }
        if let Some(ref ext) = self.extension {
            validate_extension(ext, "extension")?;
        }
        if let Some(ref ext) = self.fig_extension {
            validate_extension(ext, "fig-extension")?;
        }
        if let Some(ref compile) = self.compile {
            if let Some(ref cmd) = compile.command {
                if !cmd.contains("{input}") {
                    bail!("compile.command must contain {{input}} placeholder");
                }
            }
            if let Some(ref ext) = compile.extension {
                validate_extension(ext, "compile.extension")?;
            }
        }
        if let Some(ref mode) = self.preview {
            match mode.as_str() {
                "serve" | "open" | "none" => {}
                other => bail!("preview must be one of: serve, open, none (got '{}')", other),
            }
        }
        Ok(())
    }
}

fn validate_extension(ext: &str, field: &str) -> Result<()> {
    if ext.is_empty() || !ext.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) {
        bail!("{} must be lowercase alphanumeric (got '{}')", field, ext);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Inheritance resolution
// ---------------------------------------------------------------------------

/// Resolve `inherits` chains across all targets. Supports chained inheritance.
/// Detects cycles.
pub fn resolve_inheritance(targets: &mut HashMap<String, Target>) -> Result<()> {
    let names: Vec<String> = targets.keys().cloned().collect();
    for name in &names {
        let mut resolved = resolve_one(name, targets, &mut Vec::new())?;
        resolved.inherits = None;
        targets.insert(name.clone(), resolved);
    }
    Ok(())
}

/// Recursively resolve a single target's inheritance chain.
fn resolve_one(
    name: &str,
    targets: &HashMap<String, Target>,
    chain: &mut Vec<String>,
) -> Result<Target> {
    if chain.contains(&name.to_string()) {
        bail!("circular inheritance: {} -> {}", chain.join(" -> "), name);
    }

    let target = targets.get(name)
        .or_else(|| super::builtin_config().targets.get(name))
        .ok_or_else(|| anyhow::anyhow!(
            "target '{}' not found (referenced in inherits chain: {})",
            name, chain.join(" -> "),
        ))?
        .clone();

    if let Some(ref parent_name) = target.inherits {
        chain.push(name.to_string());
        let parent = resolve_one(parent_name, targets, chain)?;
        chain.pop();
        Ok(merge_targets(&parent, &target))
    } else {
        Ok(target)
    }
}

/// Merge a child target on top of a parent. Child fields override parent fields.
fn merge_targets(parent: &Target, child: &Target) -> Target {
    Target {
        inherits: None,
        engine: if child.engine.is_empty() { parent.engine.clone() } else { child.engine.clone() },
        template: child.template.clone().or_else(|| parent.template.clone()),
        extension: child.extension.clone().or_else(|| parent.extension.clone()),
        fig_extension: child.fig_extension.clone().or_else(|| parent.fig_extension.clone()),
        preview: child.preview.clone().or_else(|| parent.preview.clone()),
        compile: child.compile.clone().or_else(|| parent.compile.clone()),
        embed_resources: child.embed_resources.or(parent.embed_resources),
        vars: child.vars.clone().or_else(|| parent.vars.clone()),
        post: if child.post.is_empty() { parent.post.clone() } else { child.post.clone() },
    }
}

// ---------------------------------------------------------------------------
// Target resolution
// ---------------------------------------------------------------------------

/// Resolve a target by name.
///
/// Lookup order:
///   1. Project config (`calepin.toml` found on disk)
///   2. Built-in config (embedded default `calepin.toml`)
///   3. Alias resolution (e.g., "tex" -> "latex" target)
pub fn resolve_target(name: &str, config: Option<&super::ProjectConfig>) -> Result<Target> {
    // 1. Project config -- merge with built-in defaults for this base
    if let Some(cfg) = config {
        if let Some(target) = cfg.targets.get(name) {
            return Ok(merge_with_builtin(target));
        }
    }

    // 2. Built-in config (always fully specified)
    if let Some(target) = super::builtin_config().targets.get(name) {
        return Ok(target.clone());
    }

    // 3. Aliases: map to a canonical target name and retry
    let canonical = match name {
        "tex" => "latex",
        "typ" => "typst",
        "md" => "markdown",
        _ => bail!(
            "Unknown target '{}'. Define it in _calepin.toml under [targets.{}].",
            name, name,
        ),
    };

    super::builtin_config().targets.get(canonical)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Unknown target '{}'", name))
}

/// Fill unset fields in a user target from the built-in target for the same base.
fn merge_with_builtin(user: &Target) -> Target {
    let builtin = super::builtin_config().targets.get(&user.engine);
    Target {
        inherits: None,
        engine: user.engine.clone(),
        template: user.template.clone().or_else(|| builtin.and_then(|b| b.template.clone())),
        extension: user.extension.clone().or_else(|| builtin.and_then(|b| b.extension.clone())),
        fig_extension: user.fig_extension.clone().or_else(|| builtin.and_then(|b| b.fig_extension.clone())),
        preview: user.preview.clone().or_else(|| builtin.and_then(|b| b.preview.clone())),
        compile: user.compile.clone(),
        embed_resources: user.embed_resources.or(builtin.and_then(|b| b.embed_resources)),
        vars: user.vars.clone(),
        post: user.post.clone(),
    }
}

/// Convert a target's vars (toml::Value) into a minijinja-compatible Value.
#[allow(dead_code)]
pub fn target_vars_to_jinja(vars: Option<&toml::Value>) -> minijinja::Value {
    match vars {
        Some(v) => toml_to_jinja(v),
        None => minijinja::Value::from(()),
    }
}

fn toml_to_jinja(v: &toml::Value) -> minijinja::Value {
    match v {
        toml::Value::String(s) => minijinja::Value::from(s.as_str()),
        toml::Value::Integer(i) => minijinja::Value::from(*i),
        toml::Value::Float(f) => minijinja::Value::from(*f),
        toml::Value::Boolean(b) => minijinja::Value::from(*b),
        toml::Value::Array(arr) => {
            let items: Vec<minijinja::Value> = arr.iter().map(toml_to_jinja).collect();
            minijinja::Value::from(items)
        }
        toml::Value::Table(map) => {
            let mut m = std::collections::BTreeMap::new();
            for (k, v) in map {
                m.insert(k.as_str(), toml_to_jinja(v));
            }
            minijinja::Value::from_serialize(&m)
        }
        toml::Value::Datetime(d) => minijinja::Value::from(d.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Output path resolution
// ---------------------------------------------------------------------------

/// Compute the output path for a target render.
pub fn resolve_target_output_path(
    input: &Path,
    target_name: &str,
    ext: &str,
    project_root: Option<&Path>,
    output_dir: Option<&str>,
) -> PathBuf {
    if let (Some(root), Some(out)) = (project_root, output_dir) {
        let abs_input = if input.is_relative() {
            std::env::current_dir().unwrap_or_default().join(input)
        } else {
            input.to_path_buf()
        };
        let relative = abs_input.strip_prefix(root)
            .unwrap_or(&abs_input);
        root.join(out).join(target_name).join(relative).with_extension(ext)
    } else {
        input.with_extension(ext)
    }
}
