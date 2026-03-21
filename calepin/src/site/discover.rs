use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::config::{PageEntry, SiteConfig};

/// Metadata extracted from a page's YAML frontmatter.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PageMeta {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub date: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub r#abstract: Option<String>,
    #[serde(default)]
    pub listing: Option<ListingConfig>,
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
}

fn default_listing_type() -> String {
    "default".to_string()
}

/// A discovered page with its source path and extracted metadata.
#[derive(Debug, Clone)]
pub struct PageInfo {
    /// Relative path to the .qmd file (from site root)
    pub source: PathBuf,
    /// Output path relative to site root (e.g., "guide/intro.html")
    pub output: PathBuf,
    /// URL path (e.g., "/guide/intro.html")
    pub url: String,
    /// Metadata from frontmatter
    pub meta: PageMeta,
    /// Display text (from config or title)
    #[allow(dead_code)]
    pub nav_text: Option<String>,
}

/// Discover all pages referenced in the site config.
pub fn discover_pages(config: &SiteConfig, base_dir: &Path) -> Result<Vec<PageInfo>> {
    let page_paths = config.collect_page_paths();
    let mut pages = Vec::new();

    for rel_path in &page_paths {
        let source = PathBuf::from(rel_path);
        let abs_path = base_dir.join(&source);

        if !abs_path.exists() {
            eprintln!("Warning: page not found: {}", abs_path.display());
            continue;
        }

        let meta = extract_frontmatter(&abs_path)
            .with_context(|| format!("Failed to read frontmatter: {}", rel_path))?;

        let output = source.with_extension("html");
        let url = format!("/{}", output.display());

        // Find nav text from config
        let nav_text = find_nav_text(&config.website.pages, rel_path);

        pages.push(PageInfo {
            source,
            output,
            url,
            meta,
            nav_text,
        });
    }

    Ok(pages)
}

/// Discover additional pages from listing glob patterns.
pub fn discover_listing_pages(
    listing: &ListingConfig,
    base_dir: &Path,
    existing: &[PageInfo],
) -> Result<Vec<PageInfo>> {
    let pattern = base_dir.join(&listing.contents).display().to_string();
    let existing_sources: Vec<_> = existing.iter().map(|p| &p.source).collect();
    let mut pages = Vec::new();

    for entry in glob::glob(&pattern)? {
        let abs_path = entry?;
        let rel_path = abs_path
            .strip_prefix(base_dir)
            .unwrap_or(&abs_path)
            .to_path_buf();

        if existing_sources.iter().any(|s| **s == rel_path) {
            // Already in the main page list; still include in listing data
        }

        let meta = extract_frontmatter(&abs_path)?;
        let output = rel_path.with_extension("html");
        let url = format!("/{}", output.display());

        pages.push(PageInfo {
            source: rel_path,
            output,
            url,
            meta,
            nav_text: None,
        });
    }

    // Sort listing pages
    if let Some(sort) = &listing.sort {
        sort_pages(&mut pages, sort);
    }

    Ok(pages)
}

fn sort_pages(pages: &mut Vec<PageInfo>, sort_spec: &str) {
    let parts: Vec<&str> = sort_spec.split_whitespace().collect();
    let field = parts.first().copied().unwrap_or("date");
    let descending = parts.get(1).copied() == Some("desc");

    pages.sort_by(|a, b| {
        let va = get_sort_value(a, field);
        let vb = get_sort_value(b, field);
        if descending {
            vb.cmp(&va)
        } else {
            va.cmp(&vb)
        }
    });
}

fn get_sort_value(page: &PageInfo, field: &str) -> String {
    match field {
        "date" => page.meta.date.clone().unwrap_or_default(),
        "title" => page.meta.title.clone().unwrap_or_default(),
        _ => String::new(),
    }
}

/// Extract YAML frontmatter from a .qmd file.
fn extract_frontmatter(path: &Path) -> Result<PageMeta> {
    let text = fs::read_to_string(path)?;
    let trimmed = text.trim_start();

    if !trimmed.starts_with("---") {
        return Ok(PageMeta::default());
    }

    // Find the closing ---
    let after_first = &trimmed[3..];
    let end = after_first
        .find("\n---")
        .or_else(|| after_first.find("\r\n---"));

    match end {
        Some(pos) => {
            let yaml_str = &after_first[..pos];
            let meta: PageMeta = serde_saphyr::from_str(yaml_str).unwrap_or_default();
            Ok(meta)
        }
        None => Ok(PageMeta::default()),
    }
}

/// Find the display text for a page path in the config tree.
fn find_nav_text(entries: &[PageEntry], path: &str) -> Option<String> {
    for entry in entries {
        match entry {
            PageEntry::Simple(s) if s == path => return None,
            PageEntry::Page { href, text, .. } if href == path => return text.clone(),
            PageEntry::Section { pages, .. } => {
                if let Some(text) = find_nav_text(pages, path) {
                    return Some(text);
                }
            }
            _ => {}
        }
    }
    None
}
