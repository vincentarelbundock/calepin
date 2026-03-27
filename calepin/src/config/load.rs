//! Project config loading: TOML parsing, built-in defaults, and validation.

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use super::targets::resolve_inheritance;

// ---------------------------------------------------------------------------
// Project config types
// ---------------------------------------------------------------------------

/// A language declaration in `[[languages]]`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct LanguageConfig {
    /// Display name (e.g., "English", "Francais").
    pub language: String,
    /// Language code / abbreviation (e.g., "en", "fr").
    pub abbreviation: String,
    /// Whether this is the default language.
    #[serde(default)]
    pub default: bool,
    /// Icon: path to an image or Unicode flag emoji.
    #[serde(default)]
    pub icon: Option<String>,
}

/// A navigation/content item used by both `[[contents]]` (sidebar) and
/// `[[navbar.*]]` (navbar).
///
/// # TOML syntax
///
/// ```toml
/// [[contents]]
/// text = "Python API"
/// href = "man/python/index.qmd"
/// include = "man/python"              # directory -> recursive nested sections
///
/// [[contents]]
/// include = ["intro.qmd", "cli.qmd"]  # explicit flat pages
///
/// [[contents]]
/// text = "Guides"
/// include = [
///   "guides/install.qmd",
///   {text = "Config", href = "guides/config.qmd", icon = "settings"},
/// ]
/// exclude = ["**/draft_*.qmd"]
/// ```
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct ContentSection {
    /// Display text (section header, navbar label).
    #[serde(default)]
    pub text: Option<String>,

    /// Link target (clickable section header, navbar link).
    #[serde(default)]
    pub href: Option<String>,

    /// Monochrome icon path (rendered via CSS mask-image, adapts to text color).
    #[serde(default)]
    pub icon: Option<String>,

    /// Image path (rendered as `<img>`, for logos and photos).
    #[serde(default)]
    pub image: Option<String>,

    /// Dark-mode image path, or `"invert"` for CSS filter.
    #[serde(default)]
    pub image_dark: Option<String>,

    /// Width for icon/image (e.g. "80px", "2em").
    #[serde(default)]
    pub width: Option<String>,

    /// Height for icon/image (e.g. "28px", "1.5em").
    #[serde(default)]
    pub height: Option<String>,

    /// Children: paths, globs, directory, or items with metadata.
    ///
    /// Accepts:
    /// - A single string: directory path (recursive nested sections) or glob
    /// - A list of strings and/or `{text, href, image}` tables
    #[serde(default, deserialize_with = "deserialize_include")]
    pub include: Vec<IncludeEntry>,

    /// Glob patterns to exclude (applied after include).
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Nested children (for navbar dropdowns with inline items).
    #[serde(default)]
    pub children: Vec<ContentSection>,

    /// Language code for this section (sidebar only).
    #[serde(default)]
    pub lang: Option<String>,

    /// Standalone section (rendered but not in navigation).
    #[serde(default)]
    pub standalone: bool,

    // --- Backwards-compatible aliases (deprecated) ---

    /// Alias for `text` (deprecated, use `text`).
    #[serde(default)]
    pub title: Option<String>,

    /// Alias for `href` (deprecated, use `href`).
    #[serde(default)]
    pub index: Option<String>,

    /// Alias for `include` with explicit paths (deprecated, use `include`).
    #[serde(default)]
    pub pages: Vec<DocumentEntry>,

    /// Alias for `include` with a directory (deprecated, use `include`).
    #[serde(default)]
    pub dir: Option<String>,
}

impl ContentSection {
    /// Resolved display text: `text` field, falling back to `title` alias.
    pub fn display_text(&self) -> Option<&str> {
        self.text.as_deref().or(self.title.as_deref())
    }

    /// Resolved href: `href` field, falling back to `index` alias.
    pub fn display_href(&self) -> Option<&str> {
        self.href.as_deref().or(self.index.as_deref())
    }

    /// Collect all include entries, merging `include`, `pages`, `dir`, and
    /// `children` aliases into a single list.
    pub fn resolved_include(&self) -> Vec<IncludeEntry> {
        let mut result = self.include.clone();

        // Merge `pages` (deprecated alias)
        for entry in &self.pages {
            match entry {
                DocumentEntry::Path(p) => result.push(IncludeEntry::Path(p.clone())),
                DocumentEntry::Named { title, page } => {
                    result.push(IncludeEntry::Item {
                        text: Some(title.clone()),
                        href: Some(page.clone()),
                        image: None,
                        image_dark: None,
                    });
                }
            }
        }

        // Merge `dir` (deprecated alias)
        if let Some(ref dir) = self.dir {
            result.push(IncludeEntry::Path(dir.clone()));
        }

        result
    }
}

/// An entry in the `include` field: either a path/glob string or a table
/// with metadata.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum IncludeEntry {
    /// A path string (file, glob, or directory).
    Path(String),
    /// An item with metadata.
    Item {
        text: Option<String>,
        href: Option<String>,
        image: Option<String>,
        image_dark: Option<String>,
    },
}

impl<'de> serde::Deserialize<'de> for IncludeEntry {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;

        struct IncludeEntryVisitor;

        impl<'de> de::Visitor<'de> for IncludeEntryVisitor {
            type Value = IncludeEntry;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a string path/glob or a table with text/href/image fields")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<IncludeEntry, E> {
                Ok(IncludeEntry::Path(v.to_string()))
            }

            fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<IncludeEntry, E> {
                Ok(IncludeEntry::Path(v))
            }

            fn visit_map<M>(self, map: M) -> std::result::Result<IncludeEntry, M::Error>
            where
                M: de::MapAccess<'de>,
            {
                #[derive(serde::Deserialize)]
                struct ItemFields {
                    text: Option<String>,
                    href: Option<String>,
                    // Accept `page` as alias for `href`
                    page: Option<String>,
                    image: Option<String>,
                    image_dark: Option<String>,
                    // Accept `title` as alias for `text`
                    title: Option<String>,
                }

                let fields = ItemFields::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(IncludeEntry::Item {
                    text: fields.text.or(fields.title),
                    href: fields.href.or(fields.page),
                    image: fields.image,
                    image_dark: fields.image_dark,
                })
            }
        }

        deserializer.deserialize_any(IncludeEntryVisitor)
    }
}

/// Custom deserializer for `include`: accepts a single string or a list.
fn deserialize_include<'de, D>(deserializer: D) -> std::result::Result<Vec<IncludeEntry>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    struct IncludeVisitor;

    impl<'de> de::Visitor<'de> for IncludeVisitor {
        type Value = Vec<IncludeEntry>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a string, or a list of strings and/or tables")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Vec<IncludeEntry>, E> {
            Ok(vec![IncludeEntry::Path(v.to_string())])
        }

        fn visit_string<E: de::Error>(self, v: String) -> std::result::Result<Vec<IncludeEntry>, E> {
            Ok(vec![IncludeEntry::Path(v)])
        }

        fn visit_seq<S>(self, seq: S) -> std::result::Result<Vec<IncludeEntry>, S::Error>
        where
            S: de::SeqAccess<'de>,
        {
            Vec::<IncludeEntry>::deserialize(de::value::SeqAccessDeserializer::new(seq))
        }
    }

    deserializer.deserialize_any(IncludeVisitor)
}

/// A single document entry: either a bare path string or a `{title, page}` table.
/// Kept for backwards compatibility with `pages` field.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum DocumentEntry {
    /// Bare string path (possibly a glob).
    Path(String),
    /// Explicit title override + path.
    Named { title: String, page: String },
}

impl DocumentEntry {
    /// The file path, regardless of variant.
    #[allow(dead_code)]
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

/// Navbar configuration uses the same `ContentSection` type for items.
/// The three positions (left, middle, right) each contain a list of items.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct NavbarConfig {
    #[serde(default)]
    pub left: Vec<ContentSection>,
    #[serde(default)]
    pub middle: Vec<ContentSection>,
    #[serde(default)]
    pub right: Vec<ContentSection>,
}

/// A post-processing command run after the site build completes.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PostCommand {
    /// Shell command to run. Supports `{output}` and `{root}` placeholders.
    pub command: String,
    /// Restrict this command to specific target names.
    #[serde(default)]
    pub targets: Vec<String>,
}

// ---------------------------------------------------------------------------
// Built-in TOML constants
// ---------------------------------------------------------------------------

pub const SHARED_TOML: &str = include_str!("shared.toml");
pub const DOCUMENT_TOML: &str = include_str!("document.toml");
pub const COLLECTION_TOML: &str = include_str!("collection.toml");

// ---------------------------------------------------------------------------
// Config loading
// ---------------------------------------------------------------------------

/// Load a project config and return it as `Metadata`.
/// Parses TOML into Value::Table, then through parse_metadata().
pub fn load_project_metadata(path: &Path) -> Result<super::Metadata> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let tv: toml::Value = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    let table = match tv {
        toml::Value::Table(map) => crate::value::table_from_toml(map),
        _ => crate::value::Table::new(),
    };

    let mut meta = super::parse_metadata(&table)?;
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

/// Parse a built-in TOML string (concatenation of shared + specific) into Metadata.
fn parse_builtin(toml_str: &str) -> super::Metadata {
    let tv: toml::Value = toml::from_str(toml_str)
        .expect("built-in TOML must be valid");
    let table = match tv {
        toml::Value::Table(map) => crate::value::table_from_toml(map),
        _ => crate::value::Table::new(),
    };
    let mut meta = super::parse_metadata(&table)
        .expect("built-in TOML must produce valid metadata");
    resolve_inheritance(&mut meta.targets)
        .expect("built-in TOML inheritance must be valid");
    meta
}

/// Get the built-in defaults (shared + document + collection targets).
pub fn builtin_metadata() -> &'static super::Metadata {
    use std::sync::LazyLock;
    static META: LazyLock<crate::config::Metadata> = LazyLock::new(|| {
        parse_builtin(&format!("{}\n{}\n{}", SHARED_TOML, DOCUMENT_TOML, COLLECTION_TOML))
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
    fn parse_toml(toml_str: &str) -> super::super::Metadata {
        let tv: toml::Value = toml::from_str(toml_str).unwrap();
        let table = match tv {
            toml::Value::Table(map) => crate::value::table_from_toml(map),
            _ => crate::value::Table::new(),
        };
        super::super::parse_metadata(&table).unwrap()
    }

    #[test]
    fn test_parse_minimal_config() {
        let meta = parse_toml(r#"
[targets.web]
writer = "html"
"#);
        assert_eq!(meta.targets.len(), 1);
        let web = &meta.targets["web"];
        assert_eq!(web.writer, "html");
        assert_eq!(web.template_name(), "page");
        assert_eq!(web.output_extension(), "html");
    }

    #[test]
    fn test_parse_full_config() {
        let meta = parse_toml(r#"
[targets.article]
writer = "latex"
template = "article"
extension = "pdf"
fig-extension = "pdf"
compile = "tectonic {input}"

[targets.article.vars]
documentclass = "article"
fontsize = "11pt"
toc = false
"#);
        let article = &meta.targets["article"];
        assert_eq!(article.writer, "latex");
        assert_eq!(article.template_name(), "article");
        assert_eq!(article.output_extension(), "pdf");
        assert_eq!(article.fig_ext(), "pdf");
        assert_eq!(article.compile.as_deref(), Some("tectonic {input}"));
    }

    #[test]
    fn test_invalid_engine_rejected() {
        let meta = parse_toml(r#"
[targets.bad]
writer = "word"
"#);
        let target = &meta.targets["bad"];
        assert!(target.validate().is_err());
    }

    #[test]
    fn test_implicit_target_resolution() {
        let target = super::super::resolve_target("html", &HashMap::new()).unwrap();
        assert_eq!(target.writer, "html");
        assert_eq!(target.template_name(), "page");

        let target = super::super::resolve_target("latex", &HashMap::new()).unwrap();
        assert_eq!(target.writer, "latex");

        assert!(super::super::resolve_target("unknown", &HashMap::new()).is_err());
    }

    #[test]
    fn test_output_path_with_project_root() {
        let path = super::super::resolve_target_output_path(
            Path::new("/project/book/ch1.qmd"),
            "web", "html",
            Some(Path::new("/project")), Some("output"),
        );
        assert_eq!(path, PathBuf::from("/project/output/web/book/ch1.html"));
    }

    #[test]
    fn test_output_path_without_project_root() {
        let path = super::super::resolve_target_output_path(
            Path::new("/docs/paper.qmd"),
            "html", "html", None, None,
        );
        assert_eq!(path, PathBuf::from("/docs/paper.html"));
    }

    #[test]
    fn test_output_path_no_output_dir() {
        let path = super::super::resolve_target_output_path(
            Path::new("/project/ch1.qmd"),
            "web", "html",
            Some(Path::new("/project")), None,
        );
        assert_eq!(path, PathBuf::from("/project/ch1.html"));
    }

    #[test]
    fn test_output_path_custom_output_dir() {
        let path = super::super::resolve_target_output_path(
            Path::new("/project/ch1.qmd"),
            "web", "html",
            Some(Path::new("/project")), Some("build"),
        );
        assert_eq!(path, PathBuf::from("/project/build/web/ch1.html"));
    }

    #[test]
    fn test_output_path_subdirectory_preserved() {
        let path = super::super::resolve_target_output_path(
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
writer = "html"
"#);
        assert_eq!(meta.output.as_deref(), Some("output"));
    }

    #[test]
    fn test_config_output_field_absent() {
        let meta = parse_toml(r#"
[targets.web]
writer = "html"
"#);
        assert_eq!(meta.output, None);
    }

    #[test]
    fn test_collection_target() {
        let meta = parse_toml(r#"
target = "website"
[targets.website]
writer = "html"
"#);
        assert_eq!(meta.target.as_deref(), Some("website"));
    }

    #[test]
    fn test_contents_new_syntax() {
        let meta = parse_toml(r#"
[[contents]]
include = ["install.qmd", "cli.qmd"]

[[contents]]
text = "Guide"
href = "guide/index.qmd"
include = [
  "guide/basics.qmd",
  {text = "Figures & Images", href = "guide/figures.qmd"},
]
"#);
        assert_eq!(meta.contents.len(), 2);
        assert!(meta.contents[0].display_text().is_none());
        assert_eq!(meta.contents[0].resolved_include().len(), 2);
        assert_eq!(meta.contents[1].display_text(), Some("Guide"));
        assert_eq!(meta.contents[1].display_href(), Some("guide/index.qmd"));
        assert_eq!(meta.contents[1].resolved_include().len(), 2);
    }

    #[test]
    fn test_contents_backwards_compat() {
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
"#);
        assert_eq!(meta.contents.len(), 2);
        assert!(meta.contents[0].display_text().is_none());
        assert_eq!(meta.contents[0].resolved_include().len(), 2);
        assert_eq!(meta.contents[1].display_text(), Some("Guide"));
        assert_eq!(meta.contents[1].display_href(), Some("guide/index.qmd"));
        let includes = meta.contents[1].resolved_include();
        assert_eq!(includes.len(), 2);
    }

    #[test]
    fn test_include_single_string() {
        let meta = parse_toml(r#"
[[contents]]
text = "API"
include = "man/python"
"#);
        assert_eq!(meta.contents[0].include.len(), 1);
        match &meta.contents[0].include[0] {
            IncludeEntry::Path(p) => assert_eq!(p, "man/python"),
            _ => panic!("expected Path"),
        }
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
fig_width = 8.0
device = "svg"

[layout]
valign = "center"

[video]
width = "80%"

[lipsum]
paragraphs = 3

[targets.test_latex]
writer = "latex"

[targets.test_typst]
writer = "typst"

[labels]
abstract_title = "Summary"
"#);
        assert_eq!(meta.target.as_deref(), Some("html"));
        assert_eq!(meta.csl.as_deref(), Some("apa"));
        assert_eq!(meta.dpi, Some(300.0));
        assert_eq!(meta.highlight.as_ref().and_then(|h| h.light.as_deref()), Some("github"));
        assert_eq!(meta.toc.as_ref().and_then(|t| t.enabled), Some(true));
        assert_eq!(meta.toc.as_ref().and_then(|t| t.depth), Some(4));
        assert_eq!(meta.execute.as_ref().and_then(|e| e.cache), Some(false));
        assert_eq!(meta.figure.as_ref().and_then(|f| f.fig_width), Some(8.0));
        assert_eq!(meta.layout.as_ref().and_then(|l| l.valign.as_deref()), Some("center"));
        assert_eq!(meta.video.as_ref().and_then(|v| v.width.as_deref()), Some("80%"));
        assert_eq!(meta.lipsum.as_ref().and_then(|l| l.paragraphs), Some(3));
        assert_eq!(meta.labels.as_ref().and_then(|l| l.abstract_title.as_deref()), Some("Summary"));
        assert_eq!(meta.lang.as_deref(), Some("en"));
        assert_eq!(meta.csl.as_deref(), Some("apa"));
    }

}
