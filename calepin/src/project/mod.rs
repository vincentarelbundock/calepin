//! Project configuration: calepin.toml parsing, target resolution, and project root detection.

mod content;
mod defaults;
mod targets;

// Re-export all public types and functions so existing `crate::project::` paths keep working.
pub use content::{DocumentNode, expand_contents, expand_contents_for_lang, collect_all_document_paths, expand_glob_pub};
pub use defaults::*;
pub use targets::{Target, CompileConfig, resolve_target, resolve_target_output_path, target_vars_to_jinja, resolve_inheritance};

use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Project config types
// ---------------------------------------------------------------------------

/// Top-level calepin.toml structure.
#[derive(Debug, Deserialize)]
pub struct ProjectConfig {
    /// Output directory for rendered files, relative to the project root.
    #[serde(default)]
    pub output: Option<String>,

    // -- Metadata (formerly [meta]) --

    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub author: Option<toml::Value>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub bibliography: Vec<String>,
    #[serde(default)]
    pub csl: Option<String>,
    #[serde(default)]
    pub highlight: Option<HighlightDefaults>,

    // -- Collection fields --

    /// Which `[targets.*]` to use for collection rendering.
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub favicon: Option<String>,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default, rename = "logo-dark")]
    pub logo_dark: Option<String>,
    /// Path to the master template that assembles rendered page fragments.
    #[serde(default)]
    pub orchestrator: Option<String>,

    /// Languages supported by this collection.
    #[serde(default)]
    pub languages: Vec<Language>,

    /// Table of contents: ordered list of sections and pages.
    #[serde(default)]
    pub contents: Vec<ContentSection>,

    /// Arbitrary variables passed to all templates as `{{ var.key }}`.
    #[serde(default)]
    pub var: Option<toml::Value>,

    /// Configurable defaults for rendering, figures, chunks, etc.
    #[serde(default)]
    pub defaults: Option<Defaults>,

    /// Named output profiles.
    #[serde(default)]
    pub targets: HashMap<String, Target>,

    /// Post-processing commands run after site build.
    #[serde(default)]
    pub post: Vec<PostCommand>,

    /// Enable global cross-reference resolution across pages.
    #[serde(default, rename = "global-crossref")]
    pub global_crossref: bool,

    /// Extra directories to copy into the output directory as-is.
    #[serde(default, rename = "static")]
    pub static_dirs: Vec<String>,
}

impl ProjectConfig {
    /// Whether this config describes a collection (has [[contents]]).
    #[allow(dead_code)]
    pub fn is_collection(&self) -> bool {
        !self.contents.is_empty()
    }

    /// The default language code, or None if no languages are configured.
    pub fn default_language(&self) -> Option<&str> {
        if self.languages.is_empty() {
            return None;
        }
        self.languages.iter()
            .find(|l| l.default)
            .or(self.languages.first())
            .map(|l| l.code.as_str())
    }
}

/// A language declaration in `[[languages]]`.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct Language {
    /// Language code (e.g., "en", "fr").
    pub code: String,
    /// Display name (e.g., "English", "Francais").
    pub name: String,
    /// Whether this is the default language.
    #[serde(default)]
    pub default: bool,
}

/// A section in the `[[contents]]` array of tables.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ContentSection {
    /// Section title (displayed in nav). None for top-level ungrouped pages.
    #[serde(default)]
    pub title: Option<String>,
    /// Pages in this section: bare path strings or `{title, page}` tables.
    #[serde(default)]
    pub pages: Vec<DocumentEntry>,
    /// If true, pages are rendered but excluded from navigation.
    #[serde(default)]
    pub standalone: bool,
    /// The section's own page (clickable section header in nav).
    #[serde(default)]
    pub index: Option<String>,
    /// Language code for this section.
    #[serde(default)]
    pub lang: Option<String>,
}

/// A single document entry: either a bare path string or a `{title, page}` table.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum DocumentEntry {
    /// Bare string path (possibly a glob).
    Path(String),
    /// Explicit title override + path.
    Named { title: String, page: String },
}

impl DocumentEntry {
    /// The file path, regardless of variant.
    pub fn path(&self) -> &str {
        match self {
            DocumentEntry::Path(p) => p,
            DocumentEntry::Named { page, .. } => page,
        }
    }

    /// The explicit title override, if any.
    #[allow(dead_code)]
    pub fn title(&self) -> Option<&str> {
        match self {
            DocumentEntry::Path(_) => None,
            DocumentEntry::Named { title, .. } => Some(title),
        }
    }
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

/// A post-processing command run after the site build completes.
#[derive(Debug, Clone, Deserialize)]
pub struct PostCommand {
    /// Shell command to run. Supports `{output}` and `{root}` placeholders.
    pub command: String,
    /// Restrict this command to specific target names.
    #[serde(default)]
    pub targets: Vec<String>,
}

// ---------------------------------------------------------------------------
// Config loading
// ---------------------------------------------------------------------------

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

    if ArchivedStyle::by_name(csl).is_some() {
        return Ok(());
    }

    let path = project_root.join(csl);
    if path.exists() {
        return Ok(());
    }

    bail!(
        "'{}' is not a known CSL style or an existing file.\n  \
         Run `calepin info csl` to see available styles.",
        csl
    );
}

/// Parse the built-in default config (cached via LazyLock).
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_minimal_config() {
        let toml = r#"
[targets.web]
base = "html"
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.targets.len(), 1);
        let web = &config.targets["web"];
        assert_eq!(web.engine, "html");
        assert_eq!(web.template_name(), "page");
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
        assert_eq!(article.engine, "latex");
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
        assert_eq!(target.engine, "html");
        assert_eq!(target.template_name(), "page");

        let target = resolve_target("tex", None).unwrap();
        assert_eq!(target.engine, "latex");

        assert!(resolve_target("unknown", None).is_err());
    }

    #[test]
    fn test_output_path_with_project_root() {
        let path = resolve_target_output_path(
            Path::new("/project/book/ch1.qmd"),
            "web",
            "html",
            Some(Path::new("/project")),
            Some("output"),
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
            None,
        );
        assert_eq!(path, PathBuf::from("/docs/paper.html"));
    }

    #[test]
    fn test_output_path_no_output_dir() {
        let path = resolve_target_output_path(
            Path::new("/project/ch1.qmd"),
            "web",
            "html",
            Some(Path::new("/project")),
            None,
        );
        assert_eq!(path, PathBuf::from("/project/ch1.html"));
    }

    #[test]
    fn test_output_path_custom_output_dir() {
        let path = resolve_target_output_path(
            Path::new("/project/ch1.qmd"),
            "web",
            "html",
            Some(Path::new("/project")),
            Some("build"),
        );
        assert_eq!(path, PathBuf::from("/project/build/web/ch1.html"));
    }

    #[test]
    fn test_output_path_subdirectory_preserved() {
        let path = resolve_target_output_path(
            Path::new("/project/code/diagrams.qmd"),
            "website",
            "html",
            Some(Path::new("/project")),
            Some("output"),
        );
        assert_eq!(path, PathBuf::from("/project/output/website/code/diagrams.html"));
    }

    #[test]
    fn test_config_output_field() {
        let toml = r#"
output = "output"

[targets.web]
base = "html"
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.output.as_deref(), Some("output"));
    }

    #[test]
    fn test_config_output_field_absent() {
        let toml = r#"
[targets.web]
base = "html"
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.output, None);
    }

    #[test]
    fn test_collection_target() {
        let toml = r#"
target = "website"

[targets.website]
base = "html"
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.target.as_deref(), Some("website"));
    }

    #[test]
    fn test_contents_parsing() {
        let toml = r#"
[[contents]]
pages = ["install.qmd", "cli.qmd"]

[[contents]]
title = "Guide"
index = "guide/index.qmd"
pages = [
  "guide/basics.qmd",
  {title = "Figures & Images", page = "guide/figures.qmd"},
]

[[contents]]
standalone = true
pages = ["index.qmd", "404.qmd"]
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.contents.len(), 3);

        assert!(config.contents[0].title.is_none());
        assert_eq!(config.contents[0].pages.len(), 2);

        assert_eq!(config.contents[1].title.as_deref(), Some("Guide"));
        assert_eq!(config.contents[1].index.as_deref(), Some("guide/index.qmd"));
        assert_eq!(config.contents[1].pages.len(), 2);
        assert_eq!(config.contents[1].pages[1].title(), Some("Figures & Images"));
        assert_eq!(config.contents[1].pages[1].path(), "guide/figures.qmd");

        assert!(config.contents[2].standalone);
    }
}
