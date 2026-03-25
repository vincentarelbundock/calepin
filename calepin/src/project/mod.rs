//! Project configuration: calepin.toml parsing, target resolution, and project root detection.

mod content;
mod defaults;
mod targets;

// Re-export all public types and functions so existing `crate::project::` paths keep working.
pub use content::{DocumentNode, expand_contents, expand_contents_for_lang, expand_glob_pub};
pub use defaults::*;
pub use targets::{Target, CompileConfig, resolve_target, resolve_target_output_path, target_vars_to_jinja_from_meta, resolve_inheritance};

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Project config types
// ---------------------------------------------------------------------------

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

/// Load a project config and return it as `Metadata`.
/// Parses TOML into Value::Table, then through parse_metadata().
pub fn load_project_metadata(path: &Path) -> Result<crate::metadata::Metadata> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let tv: toml::Value = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    let table = match tv {
        toml::Value::Table(map) => crate::value::table_from_toml(map),
        _ => crate::value::Table::new(),
    };

    let mut meta = crate::metadata::parse_metadata(&table)?;
    let project_root = path.parent().unwrap_or(Path::new("."));

    // Resolve target inheritance and validate
    resolve_inheritance(&mut meta.targets)
        .map_err(|e| anyhow::anyhow!("in {}: {}", path.display(), e))?;
    for (name, target) in &meta.targets {
        target.validate().map_err(|e| {
            anyhow::anyhow!("Invalid target '{}' in {}: {}", name, path.display(), e)
        })?;
    }

    // Validate CSL
    if let Some(ref csl) = meta.csl {
        validate_csl(csl, project_root)
            .map_err(|e| anyhow::anyhow!("Invalid csl in {}: {}", path.display(), e))?;
    }

    Ok(meta)
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

/// Get the built-in default metadata (cached).
pub fn builtin_metadata() -> &'static crate::metadata::Metadata {
    use std::sync::LazyLock;
    static META: LazyLock<crate::metadata::Metadata> = LazyLock::new(|| {
        let content = crate::render::elements::BUILTIN_PROJECT
            .get_file("calepin.toml")
            .and_then(|f| f.contents_utf8())
            .expect("built-in calepin.toml must exist");
        let tv: toml::Value = toml::from_str(content)
            .expect("built-in calepin.toml must be valid TOML");
        let table = match tv {
            toml::Value::Table(map) => crate::value::table_from_toml(map),
            _ => crate::value::Table::new(),
        };
        let mut meta = crate::metadata::parse_metadata(&table)
            .expect("built-in calepin.toml must produce valid metadata");
        resolve_inheritance(&mut meta.targets)
            .expect("built-in calepin.toml inheritance must be valid");
        meta
    });
    &META
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::collections::HashMap;

    /// Parse a TOML string into Metadata via Value::Table -> parse_metadata().
    fn parse_toml(toml_str: &str) -> crate::metadata::Metadata {
        let tv: toml::Value = toml::from_str(toml_str).unwrap();
        let table = match tv {
            toml::Value::Table(map) => crate::value::table_from_toml(map),
            _ => crate::value::Table::new(),
        };
        crate::metadata::parse_metadata(&table).unwrap()
    }

    #[test]
    fn test_parse_minimal_config() {
        let meta = parse_toml(r#"
[targets.web]
engine = "html"
"#);
        assert_eq!(meta.targets.len(), 1);
        let web = &meta.targets["web"];
        assert_eq!(web.engine, "html");
        assert_eq!(web.template_name(), "page");
        assert_eq!(web.output_extension(), "html");
    }

    #[test]
    fn test_parse_full_config() {
        let meta = parse_toml(r#"
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
"#);
        let article = &meta.targets["article"];
        assert_eq!(article.engine, "latex");
        assert_eq!(article.template_name(), "article");
        assert_eq!(article.output_extension(), "tex");
        assert_eq!(article.fig_ext(), "pdf");
        let compile = article.compile.as_ref().unwrap();
        assert_eq!(compile.command.as_deref(), Some("tectonic {input}"));
    }

    #[test]
    fn test_invalid_engine_rejected() {
        let meta = parse_toml(r#"
[targets.bad]
engine = "word"
"#);
        let target = &meta.targets["bad"];
        assert!(target.validate().is_err());
    }

    #[test]
    fn test_implicit_target_resolution() {
        let target = resolve_target("html", &HashMap::new()).unwrap();
        assert_eq!(target.engine, "html");
        assert_eq!(target.template_name(), "page");

        let target = resolve_target("latex", &HashMap::new()).unwrap();
        assert_eq!(target.engine, "latex");

        assert!(resolve_target("unknown", &HashMap::new()).is_err());
    }

    #[test]
    fn test_output_path_with_project_root() {
        let path = resolve_target_output_path(
            Path::new("/project/book/ch1.qmd"),
            "web", "html",
            Some(Path::new("/project")), Some("output"),
        );
        assert_eq!(path, PathBuf::from("/project/output/web/book/ch1.html"));
    }

    #[test]
    fn test_output_path_without_project_root() {
        let path = resolve_target_output_path(
            Path::new("/docs/paper.qmd"),
            "html", "html", None, None,
        );
        assert_eq!(path, PathBuf::from("/docs/paper.html"));
    }

    #[test]
    fn test_output_path_no_output_dir() {
        let path = resolve_target_output_path(
            Path::new("/project/ch1.qmd"),
            "web", "html",
            Some(Path::new("/project")), None,
        );
        assert_eq!(path, PathBuf::from("/project/ch1.html"));
    }

    #[test]
    fn test_output_path_custom_output_dir() {
        let path = resolve_target_output_path(
            Path::new("/project/ch1.qmd"),
            "web", "html",
            Some(Path::new("/project")), Some("build"),
        );
        assert_eq!(path, PathBuf::from("/project/build/web/ch1.html"));
    }

    #[test]
    fn test_output_path_subdirectory_preserved() {
        let path = resolve_target_output_path(
            Path::new("/project/code/diagrams.qmd"),
            "website", "html",
            Some(Path::new("/project")), Some("output"),
        );
        assert_eq!(path, PathBuf::from("/project/output/website/code/diagrams.html"));
    }

    #[test]
    fn test_config_output_field() {
        let meta = parse_toml(r#"
output = "output"
[targets.web]
engine = "html"
"#);
        assert_eq!(meta.output.as_deref(), Some("output"));
    }

    #[test]
    fn test_config_output_field_absent() {
        let meta = parse_toml(r#"
[targets.web]
engine = "html"
"#);
        assert_eq!(meta.output, None);
    }

    #[test]
    fn test_collection_target() {
        let meta = parse_toml(r#"
target = "website"
[targets.website]
engine = "html"
"#);
        assert_eq!(meta.target.as_deref(), Some("website"));
    }

    #[test]
    fn test_contents_parsing() {
        let meta = parse_toml(r#"
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
"#);
        assert_eq!(meta.contents.len(), 3);
        assert!(meta.contents[0].title.is_none());
        assert_eq!(meta.contents[0].pages.len(), 2);
        assert_eq!(meta.contents[1].title.as_deref(), Some("Guide"));
        assert_eq!(meta.contents[1].index.as_deref(), Some("guide/index.qmd"));
        assert_eq!(meta.contents[1].pages.len(), 2);
        assert_eq!(meta.contents[1].pages[1].title(), Some("Figures & Images"));
        assert_eq!(meta.contents[1].pages[1].path(), "guide/figures.qmd");
        assert!(meta.contents[2].standalone);
    }

    #[test]
    fn test_identity_parsing() {
        // Legacy [identity] section should flatten into top-level fields
        let meta = parse_toml(r#"
[identity]
title = "My Site"
subtitle = "A test site"
author = "Test Author"
url = "https://example.com"
favicon = "icon.svg"
logo = "logo.svg"
logo-dark = "logo-dark.svg"
orchestrator = "master.html"
"#);
        assert_eq!(meta.title.as_deref(), Some("My Site"));
        assert_eq!(meta.subtitle.as_deref(), Some("A test site"));
        assert_eq!(meta.url.as_deref(), Some("https://example.com"));
        assert_eq!(meta.logo_dark.as_deref(), Some("logo-dark.svg"));
        assert_eq!(meta.orchestrator.as_deref(), Some("master.html"));
    }

    #[test]
    fn test_sections_parsing() {
        let meta = parse_toml(r#"
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

[video]
width = "80%"

[lipsum]
paragraphs = 3

[latex]
documentclass = "book"

[typst]
fontsize = "12pt"

[labels]
abstract_title = "Summary"
"#);
        let d = &meta.defaults;
        assert_eq!(d.format.as_deref(), Some("html"));
        assert_eq!(d.csl.as_deref(), Some("apa"));
        assert_eq!(d.dpi, Some(300.0));
        assert_eq!(d.highlight.as_ref().and_then(|h| h.light.as_deref()), Some("github"));
        assert_eq!(d.toc.as_ref().and_then(|t| t.enabled), Some(true));
        assert_eq!(d.toc.as_ref().and_then(|t| t.depth), Some(4));
        assert_eq!(d.execute.as_ref().and_then(|e| e.cache), Some(false));
        assert_eq!(d.figure.as_ref().and_then(|f| f.width), Some(8.0));
        assert_eq!(d.callout.as_ref().and_then(|c| c.appearance.as_deref()), Some("minimal"));
        assert_eq!(d.layout.as_ref().and_then(|l| l.valign.as_deref()), Some("center"));
        assert_eq!(d.video.as_ref().and_then(|v| v.width.as_deref()), Some("80%"));
        assert_eq!(d.lipsum.as_ref().and_then(|l| l.paragraphs), Some(3));
        assert_eq!(d.latex.as_ref().and_then(|l| l.documentclass.as_deref()), Some("book"));
        assert_eq!(d.typst.as_ref().and_then(|t| t.fontsize.as_deref()), Some("12pt"));
        assert_eq!(d.labels.as_ref().and_then(|l| l.abstract_title.as_deref()), Some("Summary"));
        // lang and csl also appear on metadata directly
        assert_eq!(meta.lang.as_deref(), Some("en"));
        assert_eq!(meta.csl.as_deref(), Some("apa"));
    }

    #[test]
    fn test_legacy_shortcodes_formats() {
        // Legacy [shortcodes.*] and [formats.*] should still work
        let meta = parse_toml(r#"
csl = "apa"

[shortcodes.video]
width = "50%"

[formats.latex]
fontsize = "12pt"
"#);
        assert_eq!(meta.defaults.video.as_ref().and_then(|v| v.width.as_deref()), Some("50%"));
        assert_eq!(meta.defaults.latex.as_ref().and_then(|l| l.fontsize.as_deref()), Some("12pt"));
    }
}
