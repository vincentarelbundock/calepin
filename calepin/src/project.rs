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
    /// Default citation style name (from hayagriva's archive).
    pub csl: Option<String>,

    /// Named output profiles.
    #[serde(default)]
    pub targets: HashMap<String, Target>,

    /// Default syntax highlighting themes.
    #[serde(default)]
    pub highlight: Option<HighlightDefaults>,
}

/// Default syntax highlighting theme configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HighlightDefaults {
    /// Theme for light mode.
    pub light: Option<String>,
    /// Theme for dark mode.
    pub dark: Option<String>,
}

/// A named output profile.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// Inherit all fields from another target. The parent's fields are used
    /// as defaults; any field set on this target overrides the parent.
    pub inherits: Option<String>,

    /// Rendering engine: html, latex, typst, or markdown.
    /// Required unless `inherits` is set.
    #[serde(default)]
    pub base: String,

    /// Document template name (default: "calepin").
    pub template: Option<String>,

    /// Output file extension (no dot). Defaults to base's default.
    pub extension: Option<String>,

    /// Default extension for generated figures.
    #[serde(rename = "fig-extension")]
    pub fig_extension: Option<String>,

    /// Preview behavior: "serve" (HTTP server), "open" (open file), or "none".
    pub preview: Option<String>,

    /// Optional compilation step.
    pub compile: Option<CompileConfig>,

    /// Arbitrary key-value pairs passed to templates as target_vars.
    pub vars: Option<toml::Value>,
}

/// Compilation configuration (e.g., .tex to .pdf).
/// When present on a target, the compile step runs automatically after rendering.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompileConfig {
    /// Shell command. {input} and {output} are replaced with file paths.
    pub command: Option<String>,

    /// Extension of the compiled artifact.
    pub extension: Option<String>,
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
        // preview
        if let Some(ref mode) = self.preview {
            match mode.as_str() {
                "serve" | "open" | "none" => {}
                other => bail!("preview must be one of: serve, open, none (got '{}')", other),
            }
        }
        Ok(())
    }
}

/// Resolve `inherits` chains across all targets. Supports chained inheritance
/// (A inherits B inherits C). A target can also inherit from a built-in target
/// name. Detects cycles.
fn resolve_inheritance(targets: &mut HashMap<String, Target>) -> Result<()> {
    let names: Vec<String> = targets.keys().cloned().collect();
    for name in &names {
        let mut resolved = resolve_one(name, targets, &mut Vec::new())?;
        resolved.inherits = None; // clear after resolution
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
        .or_else(|| builtin_config().targets.get(name))
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
        base: if child.base.is_empty() { parent.base.clone() } else { child.base.clone() },
        template: child.template.clone().or_else(|| parent.template.clone()),
        extension: child.extension.clone().or_else(|| parent.extension.clone()),
        fig_extension: child.fig_extension.clone().or_else(|| parent.fig_extension.clone()),
        preview: child.preview.clone().or_else(|| parent.preview.clone()),
        compile: child.compile.clone().or_else(|| parent.compile.clone()),
        vars: child.vars.clone().or_else(|| parent.vars.clone()),
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
    /// Template name. Always set after resolution against the built-in config.
    pub fn template_name(&self) -> &str {
        self.template.as_deref().unwrap_or("calepin")
    }

    /// Output file extension. Always set after resolution against the built-in config.
    pub fn output_extension(&self) -> &str {
        self.extension.as_deref().unwrap_or(&self.base)
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
// Project root detection and config loading
// ---------------------------------------------------------------------------

/// Walk up from `start_dir` looking for `calepin.toml` or `_calepin.toml`.
/// Returns the directory containing it (the project root).
pub fn find_project_root(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir.to_path_buf();
    loop {
        if dir.join("calepin.toml").exists() || dir.join("_calepin.toml").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Find the config file in a project root directory.
/// Prefers `calepin.toml` over `_calepin.toml`.
pub fn config_path(project_root: &Path) -> Option<PathBuf> {
    let p = project_root.join("calepin.toml");
    if p.exists() { return Some(p); }
    let p = project_root.join("_calepin.toml");
    if p.exists() { return Some(p); }
    None
}

/// Load and validate a project config from a calepin.toml file.
pub fn load_project_config(path: &Path) -> Result<ProjectConfig> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let mut config: ProjectConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;

    let project_root = path.parent().unwrap_or(Path::new("."));

    // Resolve inheritance before validation
    resolve_inheritance(&mut config.targets)
        .map_err(|e| anyhow::anyhow!("in {}: {}", path.display(), e))?;

    // Validate each target
    for (name, target) in &config.targets {
        target.validate().map_err(|e| {
            anyhow::anyhow!("Invalid target '{}' in {}: {}", name, path.display(), e)
        })?;
    }

    // Validate CSL
    if let Some(ref csl) = config.csl {
        validate_csl(csl, project_root)
            .map_err(|e| anyhow::anyhow!("Invalid csl in {}: {}", path.display(), e))?;
    }

    Ok(config)
}

/// Validate that a CSL value is either a known archive name or an existing file.
fn validate_csl(csl: &str, project_root: &Path) -> Result<()> {
    use hayagriva::archive::ArchivedStyle;

    // Known archive name
    if ArchivedStyle::by_name(csl).is_some() {
        return Ok(());
    }

    // File path (absolute or relative to project root)
    let path = project_root.join(csl);
    if path.exists() {
        return Ok(());
    }

    // File in assets/csl/
    let assets_path = project_root.join("assets/csl").join(format!("{}.csl", csl));
    if assets_path.exists() {
        return Ok(());
    }

    bail!(
        "'{}' is not a known CSL style or an existing file.\n  \
         Run `calepin info csl` to see available styles.",
        csl
    );
}

/// Parse the built-in default config (cached via LazyLock).
/// Discovered from the embedded project tree, not hardcoded.
pub fn builtin_config() -> &'static ProjectConfig {
    use std::sync::LazyLock;
    static CONFIG: LazyLock<ProjectConfig> = LazyLock::new(|| {
        let content = crate::render::elements::BUILTIN_PROJECT
            .get_file("calepin.toml")
            .and_then(|f| f.contents_utf8())
            .expect("built-in calepin.toml must exist");
        let mut config: ProjectConfig = toml::from_str(content)
            .expect("built-in calepin.toml must be valid");
        resolve_inheritance(&mut config.targets)
            .expect("built-in calepin.toml inheritance must be valid");
        config
    });
    &CONFIG
}

/// Resolve a target by name.
///
/// Lookup order:
///   1. Project config (`calepin.toml` found on disk)
///   2. Built-in config (embedded default `calepin.toml`)
///   3. Alias resolution (e.g., "tex" -> "latex" target)
///
/// User-defined targets inherit defaults from the built-in target for
/// their base. Fields the user omits are filled from the built-in.
pub fn resolve_target(name: &str, config: Option<&ProjectConfig>) -> Result<Target> {
    // 1. Project config -- merge with built-in defaults for this base
    if let Some(cfg) = config {
        if let Some(target) = cfg.targets.get(name) {
            return Ok(merge_with_builtin(target));
        }
    }

    // 2. Built-in config (always fully specified)
    if let Some(target) = builtin_config().targets.get(name) {
        return Ok(target.clone());
    }

    // 3. Aliases: map to a canonical target name and retry
    let canonical = match name {
        "tex" | "pdf" => "latex",
        "typ" => "typst",
        "md" => "markdown",
        _ => bail!(
            "Unknown target '{}'. Define it in calepin.toml under [targets.{}].",
            name, name,
        ),
    };

    builtin_config().targets.get(canonical)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Unknown target '{}'", name))
}

/// Fill unset fields in a user target from the built-in target for the same base.
fn merge_with_builtin(user: &Target) -> Target {
    let builtin = builtin_config().targets.get(&user.base);
    Target {
        inherits: None,
        base: user.base.clone(),
        template: user.template.clone().or_else(|| builtin.and_then(|b| b.template.clone())),
        extension: user.extension.clone().or_else(|| builtin.and_then(|b| b.extension.clone())),
        fig_extension: user.fig_extension.clone().or_else(|| builtin.and_then(|b| b.fig_extension.clone())),
        preview: user.preview.clone().or_else(|| builtin.and_then(|b| b.preview.clone())),
        compile: user.compile.clone(),
        vars: user.vars.clone(),
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
