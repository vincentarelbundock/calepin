//! Project configuration: calepin.toml parsing, target resolution, and project root detection.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Project config types
// ---------------------------------------------------------------------------

/// Top-level calepin.toml structure.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    /// Named output profiles.
    #[serde(default)]
    pub targets: HashMap<String, Target>,
}

/// A named output profile.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// Rendering engine: html, latex, typst, or markdown.
    pub base: String,

    /// Document template name (default: "calepin").
    pub template: Option<String>,

    /// Output file extension (no dot). Defaults to base's default.
    pub extension: Option<String>,

    /// Default extension for generated figures.
    #[serde(rename = "fig-extension")]
    pub fig_extension: Option<String>,

    /// Optional compilation step.
    pub compile: Option<CompileConfig>,

    /// Arbitrary key-value pairs passed to templates as target_vars.
    pub vars: Option<toml::Value>,
}

/// Compilation configuration (e.g., .tex to .pdf).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompileConfig {
    /// Shell command. {input} and {output} are replaced with file paths.
    pub command: Option<String>,

    /// Extension of the compiled artifact.
    pub extension: Option<String>,

    /// Whether --compile triggers this automatically.
    #[serde(default)]
    pub auto: bool,
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

impl Target {
    /// Validate a target's fields. Returns a descriptive error on failure.
    pub fn validate(&self) -> Result<()> {
        // base
        match self.base.as_str() {
            "html" | "latex" | "typst" | "markdown" => {}
            other => bail!("base must be one of: html, latex, typst, markdown (got '{}')", other),
        }
        // extension
        if let Some(ref ext) = self.extension {
            validate_extension(ext, "extension")?;
        }
        // fig-extension
        if let Some(ref ext) = self.fig_extension {
            validate_extension(ext, "fig-extension")?;
        }
        // compile
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
// Target resolution
// ---------------------------------------------------------------------------

impl Target {
    /// Template name (default: "calepin").
    pub fn template_name(&self) -> &str {
        self.template.as_deref().unwrap_or("calepin")
    }

    /// Output file extension. Defaults to base's canonical extension.
    pub fn output_extension(&self) -> &str {
        self.extension.as_deref().unwrap_or_else(|| base_extension(&self.base))
    }

    /// Default figure extension. Defaults to base's default.
    pub fn fig_ext(&self) -> &str {
        self.fig_extension.as_deref().unwrap_or_else(|| base_fig_extension(&self.base))
    }
}

/// Map a base name to its canonical file extension.
pub fn base_extension(base: &str) -> &str {
    match base {
        "html" => "html",
        "latex" => "tex",
        "typst" => "typ",
        "markdown" => "md",
        _ => base,
    }
}

/// Map a base name to its default figure extension.
fn base_fig_extension(base: &str) -> &str {
    match base {
        "html" => "svg",
        "latex" => "pdf",
        "typst" => "svg",
        "markdown" => "png",
        _ => "png",
    }
}

// ---------------------------------------------------------------------------
// Project root detection and config loading
// ---------------------------------------------------------------------------

/// Walk up from `start_dir` looking for `calepin.toml`.
/// Returns the directory containing it (the project root).
pub fn find_project_root(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir.to_path_buf();
    loop {
        if dir.join("calepin.toml").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Load and validate a project config from a calepin.toml file.
pub fn load_project_config(path: &Path) -> Result<ProjectConfig> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let config: ProjectConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;

    // Validate each target
    for (name, target) in &config.targets {
        target.validate().map_err(|e| {
            anyhow::anyhow!("Invalid target '{}' in {}: {}", name, path.display(), e)
        })?;
    }

    Ok(config)
}

/// Resolve a target by name from the project config.
/// If no config exists or the target isn't defined, creates an implicit target
/// when the name matches a base name or alias.
pub fn resolve_target(name: &str, config: Option<&ProjectConfig>) -> Result<Target> {
    // Check project config first
    if let Some(cfg) = config {
        if let Some(target) = cfg.targets.get(name) {
            return Ok(target.clone());
        }
    }

    // Implicit target: name matches a base or alias
    let base = match name {
        "html" => "html",
        "latex" | "tex" | "pdf" => "latex",
        "typst" | "typ" => "typst",
        "markdown" | "md" => "markdown",
        _ => bail!(
            "Unknown target '{}'. Define it in calepin.toml under [targets.{}], \
             or use a base format name (html, latex, typst, markdown).",
            name, name,
        ),
    };

    Ok(Target {
        base: base.to_string(),
        template: None,
        extension: None,
        fig_extension: None,
        compile: None,
        vars: None,
    })
}

/// Convert a target's vars (toml::Value) into a minijinja-compatible Value.
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
// Output path resolution for targets
// ---------------------------------------------------------------------------

/// Compute the output path for a target render.
///
/// When a project root exists and no -o override is given:
///   output/<target>/<relative_path>.<ext>
///
/// When no project root: input.with_extension(ext) (backward compatible).
pub fn resolve_target_output_path(
    input: &Path,
    target_name: &str,
    ext: &str,
    project_root: Option<&Path>,
) -> PathBuf {
    if let Some(root) = project_root {
        // Try stripping content/ prefix first, then project root
        let content_dir = root.join("content");
        let relative = input.strip_prefix(&content_dir)
            .or_else(|_| input.strip_prefix(root))
            .unwrap_or(input);
        root.join("output").join(target_name).join(relative).with_extension(ext)
    } else {
        input.with_extension(ext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let toml = r#"
[targets.web]
base = "html"
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.targets.len(), 1);
        let web = &config.targets["web"];
        assert_eq!(web.base, "html");
        assert_eq!(web.template_name(), "calepin");
        assert_eq!(web.output_extension(), "html");
    }

    #[test]
    fn test_parse_full_config() {
        let toml = r#"
[targets.article]
base = "latex"
template = "article"
extension = "tex"
fig-extension = "pdf"

[targets.article.compile]
command = "tectonic {input}"
extension = "pdf"
auto = true

[targets.article.vars]
documentclass = "article"
fontsize = "11pt"
toc = false
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        let article = &config.targets["article"];
        assert_eq!(article.base, "latex");
        assert_eq!(article.template_name(), "article");
        assert_eq!(article.output_extension(), "tex");
        assert_eq!(article.fig_ext(), "pdf");
        let compile = article.compile.as_ref().unwrap();
        assert_eq!(compile.command.as_deref(), Some("tectonic {input}"));
        assert!(compile.auto);
    }

    #[test]
    fn test_invalid_base_rejected() {
        let toml = r#"
[targets.bad]
base = "word"
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        let target = &config.targets["bad"];
        assert!(target.validate().is_err());
    }

    #[test]
    fn test_unknown_fields_rejected() {
        let toml = r#"
[targets.web]
base = "html"
unknown_field = "oops"
"#;
        let result: Result<ProjectConfig, _> = toml::from_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_implicit_target_resolution() {
        let target = resolve_target("html", None).unwrap();
        assert_eq!(target.base, "html");
        assert_eq!(target.template_name(), "calepin");

        let target = resolve_target("tex", None).unwrap();
        assert_eq!(target.base, "latex");

        assert!(resolve_target("unknown", None).is_err());
    }

    #[test]
    fn test_output_path_with_project_root() {
        let path = resolve_target_output_path(
            Path::new("/project/content/book/ch1.qmd"),
            "web",
            "html",
            Some(Path::new("/project")),
        );
        assert_eq!(path, PathBuf::from("/project/output/web/book/ch1.html"));
    }

    #[test]
    fn test_output_path_without_project_root() {
        let path = resolve_target_output_path(
            Path::new("/docs/paper.qmd"),
            "html",
            "html",
            None,
        );
        assert_eq!(path, PathBuf::from("/docs/paper.html"));
    }
}
