//! Project configuration: calepin.toml parsing, target resolution, and project root detection.

mod content;
mod defaults;
mod targets;

// Re-export all public types and functions so existing `crate::project::` paths keep working.
pub use content::{DocumentNode, expand_contents, expand_contents_for_lang, expand_glob_pub};
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
    // -- Bare top-level keys (defaultable) --

    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default, alias = "preview-port")]
    pub preview_port: Option<u16>,
    #[serde(default)]
    pub csl: Option<String>,
    #[serde(default)]
    pub dpi: Option<f64>,
    #[serde(default)]
    pub timeout: Option<u64>,
    #[serde(default)]
    pub math: Option<String>,
    #[serde(default, alias = "embed-resources")]
    pub embed_resources: Option<bool>,

    // -- Top-level sections (defaultable) --

    #[serde(default)]
    pub highlight: Option<HighlightDefaults>,
    #[serde(default)]
    pub toc: Option<TocDefaults>,
    #[serde(default)]
    pub labels: Option<LabelsDefaults>,
    #[serde(default)]
    pub execute: Option<ExecuteDefaults>,
    #[serde(default)]
    pub figure: Option<FigureDefaults>,
    #[serde(default)]
    pub callout: Option<CalloutDefaults>,
    #[serde(default)]
    pub layout: Option<LayoutDefaults>,
    #[serde(default)]
    pub shortcodes: Option<ShortcodesConfig>,
    #[serde(default)]
    pub formats: Option<FormatsConfig>,

    // -- Identity --

    #[serde(default)]
    pub identity: Option<IdentityConfig>,

    // -- Project / collection fields --

    /// Output directory for rendered files, relative to the project root.
    #[serde(default)]
    pub output: Option<String>,
    /// Which `[targets.*]` to use for collection rendering.
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub bibliography: Vec<String>,
    /// Enable global cross-reference resolution across pages.
    #[serde(default, alias = "global-crossref")]
    pub global_crossref: bool,
    /// Extra directories to copy into the output directory as-is.
    #[serde(default, rename = "static")]
    pub static_dirs: Vec<String>,

    // -- Collection --

    /// Languages supported by this collection.
    #[serde(default)]
    pub languages: Vec<Language>,
    /// Table of contents: ordered list of sections and pages.
    #[serde(default)]
    pub contents: Vec<ContentSection>,
    /// Arbitrary variables passed to all templates as `{{ var.key }}`.
    #[serde(default)]
    pub var: Option<toml::Value>,
    /// Named output profiles.
    #[serde(default)]
    pub targets: HashMap<String, Target>,
    /// Post-processing commands run after site build.
    #[serde(default)]
    pub post: Vec<PostCommand>,
}

impl ProjectConfig {
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

    /// Extract a flat `Defaults` from this config's top-level fields.
    /// Flattens [shortcodes] and [formats] sections into individual fields.
    pub fn as_defaults(&self) -> Defaults {
        Defaults {
            format: self.format.clone(),
            lang: self.lang.clone(),
            preview_port: self.preview_port,
            csl: self.csl.clone(),
            dpi: self.dpi,
            timeout: self.timeout,
            math: self.math.clone(),
            embed_resources: self.embed_resources,
            highlight: self.highlight.clone(),
            toc: self.toc.clone(),
            labels: self.labels.clone(),
            execute: self.execute.clone(),
            figure: self.figure.clone(),
            callout: self.callout.clone(),
            layout: self.layout.clone(),
            // Flatten [shortcodes.*]
            video: self.shortcodes.as_ref().and_then(|s| s.video.clone()),
            placeholder: self.shortcodes.as_ref().and_then(|s| s.placeholder.clone()),
            lipsum: self.shortcodes.as_ref().and_then(|s| s.lipsum.clone()),
            // Flatten [formats.*]
            latex: self.formats.as_ref().and_then(|f| f.latex.clone()),
            typst: self.formats.as_ref().and_then(|f| f.typst.clone()),
            revealjs: self.formats.as_ref().and_then(|f| f.revealjs.clone()),
        }
    }
}

/// Project identity: title, author, URL, branding.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct IdentityConfig {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub author: Option<toml::Value>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub favicon: Option<String>,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default, alias = "logo-dark")]
    pub logo_dark: Option<String>,
    /// Path to the master template that assembles rendered page fragments.
    #[serde(default)]
    pub orchestrator: Option<String>,
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
engine = "html"
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
engine = "latex"
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
    fn test_invalid_engine_rejected() {
        let toml = r#"
[targets.bad]
engine = "word"
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        let target = &config.targets["bad"];
        assert!(target.validate().is_err());
    }

    #[test]
    fn test_unknown_fields_rejected() {
        let toml = r#"
[targets.web]
engine = "html"
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

        let target = resolve_target("latex", None).unwrap();
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
engine = "html"
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.output.as_deref(), Some("output"));
    }

    #[test]
    fn test_config_output_field_absent() {
        let toml = r#"
[targets.web]
engine = "html"
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.output, None);
    }

    #[test]
    fn test_collection_target() {
        let toml = r#"
target = "website"

[targets.website]
engine = "html"
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

    #[test]
    fn test_identity_parsing() {
        let toml = r#"
[identity]
title = "My Site"
subtitle = "A test site"
author = "Test Author"
url = "https://example.com"
favicon = "icon.svg"
logo = "logo.svg"
logo-dark = "logo-dark.svg"
orchestrator = "master.html"
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        let id = config.identity.unwrap();
        assert_eq!(id.title.as_deref(), Some("My Site"));
        assert_eq!(id.subtitle.as_deref(), Some("A test site"));
        assert_eq!(id.url.as_deref(), Some("https://example.com"));
        assert_eq!(id.logo_dark.as_deref(), Some("logo-dark.svg"));
        assert_eq!(id.orchestrator.as_deref(), Some("master.html"));
    }

    #[test]
    fn test_new_sections_parsing() {
        let toml = r#"
format = "html"
lang = "en"
csl = "apa"
dpi = 300.0
math = "mathjax"

[highlight]
light = "github"
dark = "nord"

[toc]
enabled = true
depth = 4

[execute]
cache = false
echo = false

[figure]
width = 8.0
device = "svg"

[callout]
appearance = "minimal"

[layout]
valign = "center"

[shortcodes.video]
width = "80%"

[shortcodes.lipsum]
paragraphs = 3

[formats.latex]
documentclass = "book"

[formats.typst]
fontsize = "12pt"

[labels]
abstract_title = "Summary"
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.format.as_deref(), Some("html"));
        assert_eq!(config.csl.as_deref(), Some("apa"));
        assert_eq!(config.dpi, Some(300.0));

        let hl = config.highlight.unwrap();
        assert_eq!(hl.light.as_deref(), Some("github"));

        let toc = config.toc.unwrap();
        assert_eq!(toc.enabled, Some(true));
        assert_eq!(toc.depth, Some(4));

        let exec = config.execute.unwrap();
        assert_eq!(exec.cache, Some(false));

        let fig = config.figure.unwrap();
        assert_eq!(fig.width, Some(8.0));

        let callout = config.callout.unwrap();
        assert_eq!(callout.appearance.as_deref(), Some("minimal"));

        let layout = config.layout.unwrap();
        assert_eq!(layout.valign.as_deref(), Some("center"));

        let sc = config.shortcodes.unwrap();
        assert_eq!(sc.video.unwrap().width.as_deref(), Some("80%"));
        assert_eq!(sc.lipsum.unwrap().paragraphs, Some(3));

        let fm = config.formats.unwrap();
        assert_eq!(fm.latex.unwrap().documentclass.as_deref(), Some("book"));
        assert_eq!(fm.typst.unwrap().fontsize.as_deref(), Some("12pt"));

        let labels = config.labels.unwrap();
        assert_eq!(labels.abstract_title.as_deref(), Some("Summary"));
    }

    #[test]
    fn test_as_defaults_flattens() {
        let toml = r#"
csl = "apa"

[shortcodes.video]
width = "50%"

[formats.latex]
fontsize = "12pt"
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        let defs = config.as_defaults();
        assert_eq!(defs.csl.as_deref(), Some("apa"));
        assert_eq!(defs.video.as_ref().and_then(|v| v.width.as_deref()), Some("50%"));
        assert_eq!(defs.latex.as_ref().and_then(|l| l.fontsize.as_deref()), Some("12pt"));
    }
}
