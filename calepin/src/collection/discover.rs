use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::Metadata;

/// Metadata extracted from a document's frontmatter.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub date: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub r#abstract: Option<String>,
    #[serde(default)]
    pub listing: Option<ListingConfig>,
    /// Language code for this page (e.g., "en", "fr").
    #[serde(default)]
    pub lang: Option<String>,
    /// Config variables, available as `{{ cfg.* }}` in templates.
    #[serde(skip)]
    pub cfg: HashMap<String, crate::value::Value>,
    /// Template variant selections (the `[tpl]` table).
    #[serde(skip)]
    pub tpl: HashMap<String, String>,
}

/// Listing configuration in page frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingConfig {
    pub contents: String,
    #[serde(default = "default_listing_type")]
    pub r#type: String,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub fields: Vec<String>,
    /// Number of items per page (0 or absent = no pagination).
    #[serde(default, rename = "page-size")]
    pub page_size: usize,
}

fn default_listing_type() -> String {
    "default".to_string()
}

/// A discovered document with its source path and extracted metadata.
#[derive(Debug, Clone)]
pub struct DocumentInfo {
    /// Relative path to the .qmd file (from collection root)
    pub source: PathBuf,
    /// Output path relative to collection root (e.g., "guide/intro.html")
    pub output: PathBuf,
    /// URL path (e.g., "/guide/intro.html")
    pub url: String,
    /// Metadata from frontmatter
    pub meta: DocumentMeta,
    /// Language code (from frontmatter `lang:`, or the default language).
    pub lang: Option<String>,
}

/// Discover all .qmd documents in the project directory.
/// Finds every .qmd file under `base_dir`, excluding files matching `config.exclude` patterns
/// and files inside sidecar directories or the output directory.
pub fn discover_documents(config: &Metadata, base_dir: &Path, output_ext: &str) -> Result<Vec<DocumentInfo>> {
    let default_lang = config.default_language().map(|s| s.to_string());
    let output_name = config.output.clone().unwrap_or_else(|| format!("index{}", crate::paths::DEFAULT_OUTPUT_SUFFIX));
    let mut seen = std::collections::HashSet::new();
    let mut pages = Vec::new();

    // Walk all .qmd files under base_dir
    for entry in walkdir::WalkDir::new(base_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "qmd"))
    {
        let abs_path = entry.path().to_path_buf();
        let rel = abs_path.strip_prefix(base_dir)
            .unwrap_or(&abs_path)
            .to_path_buf();
        let rel_str = rel.display().to_string();

        // Skip sidecar directories (*_calepin/) and output/
        if rel.components().any(|c| {
            let s = c.as_os_str().to_string_lossy();
            s.ends_with("_calepin")
        }) || rel_str.starts_with(&format!("{}/", output_name)) {
            continue;
        }

        // Skip excluded patterns
        if is_excluded(&rel_str, &config.exclude) {
            continue;
        }

        if !seen.insert(rel_str.clone()) {
            continue;
        }

        let meta = extract_frontmatter(&abs_path)
            .with_context(|| format!("Failed to read frontmatter: {}", rel_str))?;

        let lang = meta.lang.clone()
            .or_else(|| detect_lang_from_path(&rel_str, &config.languages))
            .or_else(|| default_lang.clone());
        let output = rel.with_extension(output_ext);
        let url = format!("/{}", output.display());

        pages.push(DocumentInfo {
            source: rel,
            output,
            url,
            meta,
            lang,
        });
    }

    Ok(pages)
}

/// Detect language from directory path by matching against `[[languages]]` path prefixes.
/// E.g., `fr/guide.qmd` matches a language with `path = "fr"`.
fn detect_lang_from_path(rel_path: &str, languages: &[crate::config::LanguageConfig]) -> Option<String> {
    for lang in languages {
        if let Some(ref lang_path) = lang.path {
            let prefix = if lang_path == "." { "" } else { lang_path.as_str() };
            if prefix.is_empty() {
                // Root path: only match files not under any other language's path
                continue;
            }
            if rel_path.starts_with(prefix) && rel_path.as_bytes().get(prefix.len()) == Some(&b'/') {
                return Some(lang.abbreviation.clone());
            }
        }
    }
    // Check root-path languages last (path = "." matches anything not claimed above)
    for lang in languages {
        if let Some(ref lang_path) = lang.path {
            if lang_path == "." {
                return Some(lang.abbreviation.clone());
            }
        }
    }
    None
}

/// Check if a relative path matches any exclude glob pattern.
fn is_excluded(rel_path: &str, exclude_patterns: &[String]) -> bool {
    for pattern in exclude_patterns {
        if let Ok(glob) = glob::Pattern::new(pattern) {
            if glob.matches(rel_path) {
                return true;
            }
        }
    }
    false
}

/// Discover additional documents from listing glob patterns.
pub fn discover_listing_documents(
    listing: &ListingConfig,
    base_dir: &Path,
    _existing: &[DocumentInfo],
    output_ext: &str,
) -> Result<Vec<DocumentInfo>> {
    let pattern = base_dir.join(&listing.contents).display().to_string();
    let mut pages = Vec::new();

    for entry in glob::glob(&pattern)? {
        let abs_path = entry?;
        let rel_path = abs_path
            .strip_prefix(base_dir)
            .unwrap_or(&abs_path)
            .to_path_buf();

        let meta = extract_frontmatter(&abs_path)?;
        let lang = meta.lang.clone();
        let output = rel_path.with_extension(output_ext);
        let url = format!("/{}", output.display());

        pages.push(DocumentInfo {
            source: rel_path,
            output,
            url,
            meta,
            lang,
        });
    }

    // Sort listing pages
    if let Some(sort) = &listing.sort {
        sort_documents(&mut pages, sort);
    }

    Ok(pages)
}

fn sort_documents(pages: &mut Vec<DocumentInfo>, sort_spec: &str) {
    let parts: Vec<&str> = sort_spec.split_whitespace().collect();
    let field = parts.first().copied().unwrap_or("date");
    let descending = parts.get(1).copied() == Some("desc");

    pages.sort_by(|a, b| {
        let va = resolve_sort_value(a, field);
        let vb = resolve_sort_value(b, field);
        if descending {
            vb.cmp(&va)
        } else {
            va.cmp(&vb)
        }
    });
}

fn resolve_sort_value(page: &DocumentInfo, field: &str) -> String {
    match field {
        "date" => page.meta.date.clone().unwrap_or_default(),
        "title" => page.meta.title.clone().unwrap_or_default(),
        _ => String::new(),
    }
}

/// Extract frontmatter from a .qmd file.
/// Reads the TOML front matter (between `---` delimiters) and deserializes it.
fn extract_frontmatter(path: &Path) -> Result<DocumentMeta> {
    let content = std::fs::read_to_string(path)?;
    let (meta, _body) = crate::config::split_frontmatter(&content)?;
    let image = meta.cfg.get("image").and_then(|v| v.as_str()).map(String::from);
    Ok(DocumentMeta {
        title: meta.title,
        subtitle: meta.subtitle,
        date: meta.date,
        description: meta.abstract_text.clone(),
        image,
        r#abstract: meta.abstract_text,
        listing: None,
        lang: meta.lang,
        cfg: meta.cfg,
        tpl: meta.tpl,
    })
}

// ---------------------------------------------------------------------------
// Config loading
// ---------------------------------------------------------------------------

/// Load and validate a project config, returning it as `Metadata`.
/// Looks for `{stem}_calepin/config.toml` in `base_dir`.
pub fn load_config(config_path: Option<&Path>, base_dir: &Path) -> Result<(Metadata, PathBuf)> {
    if let Some(path) = config_path {
        let config = crate::config::load_project_metadata(path)?;
        return Ok((config, path.to_path_buf()));
    }

    if let Some(path) = crate::cli::find_project_config(base_dir) {
        let meta = crate::config::load_project_metadata(&path)?;
        return Ok((meta, path));
    }

    anyhow::bail!(
        "No config found. Create index_calepin/config.toml in {}",
        base_dir.display()
    )
}

/// Collect .qmd page paths listed in [[contents]], expanding globs.
/// These are the pages that appear in navigation (sidebar, prev/next).
pub fn collect_document_paths(meta: &Metadata, base_dir: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    for section in &meta.contents {
        if let Some(href) = section.display_href() {
            if href.ends_with(".qmd") {
                paths.push(href.to_string());
            }
        }
        for entry in &section.resolved_include() {
            let entry_path = match entry {
                crate::config::IncludeEntry::Path(p) => p.as_str(),
                crate::config::IncludeEntry::Item { href: Some(h), .. } => h.as_str(),
                _ => continue,
            };
            // Directory includes: scan recursively for .qmd files
            if base_dir.join(entry_path).is_dir() {
                let pattern = format!("{}/**/*.qmd", entry_path);
                for path in super::contents::expand_glob(&pattern, base_dir) {
                    paths.push(path);
                }
            } else {
                for path in super::contents::expand_glob(entry_path, base_dir) {
                    if path.ends_with(".qmd") {
                        paths.push(path);
                    }
                }
            }
        }
    }
    paths
}

