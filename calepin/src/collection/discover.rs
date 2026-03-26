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
    /// Translation links: language code -> relative path to translated page.
    #[serde(default)]
    pub translations: Option<HashMap<String, String>>,
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
    /// Standalone documents are rendered but excluded from navigation.
    pub standalone: bool,
    /// Language code (from frontmatter `lang:`, or the default language).
    pub lang: Option<String>,
}

/// Discover all documents referenced in the collection config.
pub fn discover_documents(config: &Metadata, base_dir: &Path, output_ext: &str) -> Result<Vec<DocumentInfo>> {
    let default_lang = config.default_language().map(|s| s.to_string());
    let page_paths = super::config::collect_document_paths(config, base_dir);
    let mut pages = Vec::new();

    for rel_path in &page_paths {
        let source = PathBuf::from(rel_path);
        let abs_path = base_dir.join(&source);

        if !abs_path.exists() {
            eprintln!("Warning: document not found: {}", abs_path.display());
            continue;
        }

        let meta = extract_frontmatter(&abs_path)
            .with_context(|| format!("Failed to read frontmatter: {}", rel_path))?;

        let lang = meta.lang.clone().or_else(|| default_lang.clone());

        let output_rel = &source;
        let output = output_rel.with_extension(output_ext);
        let url = format!("/{}", output.display());

        pages.push(DocumentInfo {
            source,
            output,
            url,
            meta,
            standalone: false,
            lang,
        });
    }

    Ok(pages)
}

/// Discover standalone documents (rendered but not in nav).
pub fn discover_standalone_documents(config: &Metadata, base_dir: &Path, output_ext: &str) -> Result<Vec<DocumentInfo>> {
    let default_lang = config.default_language().map(|s| s.to_string());
    let paths = super::config::collect_standalone_paths(config, base_dir);
    let mut pages = Vec::new();

    for rel_path in &paths {
        let source = PathBuf::from(rel_path);
        let abs_path = base_dir.join(&source);

        if !abs_path.exists() {
            eprintln!("Warning: standalone document not found: {}", abs_path.display());
            continue;
        }

        let meta = extract_frontmatter(&abs_path)
            .with_context(|| format!("Failed to read frontmatter: {}", rel_path))?;

        let lang = meta.lang.clone().or_else(|| default_lang.clone());

        let output = source.with_extension(output_ext);
        let url = format!("/{}", output.display());

        pages.push(DocumentInfo {
            source,
            output,
            url,
            meta,
            standalone: true,
            lang,
        });
    }

    Ok(pages)
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
            standalone: false,
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
/// Preambles are ignored; returns default metadata.
fn extract_frontmatter(_path: &Path) -> Result<DocumentMeta> {
    Ok(DocumentMeta::default())
}

