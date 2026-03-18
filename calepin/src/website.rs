//! Website builder: renders a collection of .qmd files into a _site/ directory
//! with navigation, sidebar, and shared layout.
//!
//! Driven by a YAML config file (typically `_calepin.yaml`) that defines
//! the page hierarchy, navbar, and site metadata.
//!
//! Shared types and functions are `pub(crate)` so that `site_builder` can reuse them.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

// ---------------------------------------------------------------------------
// Config types
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub(crate) struct WebsiteConfig {
    pub title: String,
    pub subtitle: Option<String>,
    pub favicon: Option<String>,
    pub navbar: NavbarConfig,
    pub pages: Vec<PageEntry>,
    pub format_overrides: Vec<String>,
    pub resources: Vec<String>,
    pub builder: Option<String>,
    pub raw_yaml: saphyr::YamlOwned,
}

#[allow(dead_code)]
pub(crate) struct NavbarConfig {
    pub logo: Option<String>,
    pub right: Vec<NavItem>,
}

#[allow(dead_code)]
pub(crate) struct NavItem {
    pub text: String,
    pub href: String,
}

#[derive(Clone)]
#[allow(dead_code)]
pub(crate) enum PageEntry {
    Page { text: Option<String>, href: String },
    Section { title: String, pages: Vec<PageEntry> },
}

/// A flattened page reference with display text and href.
pub(crate) struct FlatPage {
    pub text: Option<String>,
    pub href: String,
}

// ---------------------------------------------------------------------------
// Config parsing
// ---------------------------------------------------------------------------

/// Safe key lookup on a YamlOwned value (returns None instead of panicking).
fn yaml_get<'a>(val: &'a saphyr::YamlOwned, key: &str) -> Option<&'a saphyr::YamlOwned> {
    val.as_mapping_get(key)
}

/// Convenience: chain two key lookups.
fn yaml_get2<'a>(val: &'a saphyr::YamlOwned, k1: &str, k2: &str) -> Option<&'a saphyr::YamlOwned> {
    yaml_get(val, k1).and_then(|v| yaml_get(v, k2))
}

static YAML_BAD: saphyr::YamlOwned = saphyr::YamlOwned::BadValue;

pub(crate) fn parse_config(path: &Path) -> Result<WebsiteConfig> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config: {}", path.display()))?;
    use saphyr::LoadableYamlNode;
    let docs = saphyr::YamlOwned::load_from_str(&content)?;
    let yaml = docs.into_iter().next().unwrap_or(saphyr::YamlOwned::BadValue);

    let website = yaml_get(&yaml, "website").unwrap_or(&YAML_BAD);
    let title = yaml_get(website, "title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled")
        .to_string();
    let subtitle = yaml_get(website, "subtitle").and_then(|v| v.as_str()).map(|s| s.to_string());
    let favicon = yaml_get(website, "favicon").and_then(|v| v.as_str()).map(|s| s.to_string());

    // Navbar
    let navbar_val = yaml_get(website, "navbar").unwrap_or(&YAML_BAD);
    let logo = yaml_get(navbar_val, "logo").and_then(|v| v.as_str()).map(|s| s.to_string());
    let right = parse_nav_items(yaml_get(navbar_val, "right").unwrap_or(&YAML_BAD));

    // Pages
    let pages = parse_page_entries(yaml_get(website, "pages").unwrap_or(&YAML_BAD));

    // Format overrides (from format.html section)
    let mut format_overrides = Vec::new();
    if let Some(html_fmt) = yaml_get2(&yaml, "format", "html").and_then(|v| v.as_mapping()) {
        for (k, v) in html_fmt {
            if let Some(key) = k.as_str() {
                // For map values (e.g. highlight-style: {light: x, dark: y}),
                // flatten to dot-notation so apply_overrides builds nested YAML.
                if let Some(map) = v.as_mapping() {
                    for (mk, mv) in map {
                        if let (Some(mkey), Some(mval)) = (mk.as_str(), mv.as_str()) {
                            format_overrides.push(format!("{}.{}={}", key, mkey, mval));
                        }
                    }
                    continue;
                }
                let val = if let Some(b) = v.as_bool() { b.to_string() }
                    else if let Some(n) = v.as_integer() { n.to_string() }
                    else if let Some(f) = v.as_floating_point() { f.to_string() }
                    else if let Some(s) = v.as_str() { s.to_string() }
                    else { continue };
                format_overrides.push(format!("{}={}", key, val));
            }
        }
    }

    // Resources
    let resources = yaml_get2(&yaml, "project", "resources")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    // Builder plugin name
    let builder = yaml_get2(&yaml, "calepin", "builder")
        .and_then(|v| v.as_str())
        .or_else(|| yaml_get(website, "builder").and_then(|v| v.as_str()))
        .map(|s| s.to_string());

    Ok(WebsiteConfig {
        title,
        subtitle,
        favicon,
        navbar: NavbarConfig {
            logo,
            right,
        },
        pages,
        format_overrides,
        resources,
        builder,
        raw_yaml: yaml,
    })
}

fn parse_nav_items(val: &saphyr::YamlOwned) -> Vec<NavItem> {
    let Some(seq) = val.as_sequence() else {
        return Vec::new();
    };
    seq.iter()
        .filter_map(|item| {
            let text = item.as_mapping_get("text")?.as_str()?.to_string();
            let href = item.as_mapping_get("href")?.as_str()?.to_string();
            Some(NavItem { text, href })
        })
        .collect()
}

pub(crate) fn parse_page_entries(val: &saphyr::YamlOwned) -> Vec<PageEntry> {
    let Some(seq) = val.as_sequence() else {
        return Vec::new();
    };
    seq.iter()
        .filter_map(|item| {
            if let Some(s) = item.as_str() {
                Some(PageEntry::Page {
                    text: None,
                    href: s.to_string(),
                })
            } else if let Some(title) = item.as_mapping_get("section").and_then(|v| v.as_str()) {
                let pages_val = item.as_mapping_get("pages")
                    .cloned()
                    .unwrap_or(saphyr::YamlOwned::BadValue);
                let pages = parse_page_entries(&pages_val);
                Some(PageEntry::Section { title: title.to_string(), pages })
            } else if let Some(href) = item.as_mapping_get("href").and_then(|v| v.as_str()) {
                let text = item.as_mapping_get("text").and_then(|v| v.as_str()).map(|s| s.to_string());
                Some(PageEntry::Page { text, href: href.to_string() })
            } else {
                None
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Page collection
// ---------------------------------------------------------------------------

/// Flatten the page hierarchy into an ordered list.
pub(crate) fn collect_pages(entries: &[PageEntry]) -> Vec<FlatPage> {
    let mut out = Vec::new();
    for entry in entries {
        match entry {
            PageEntry::Page { text, href } => {
                out.push(FlatPage {
                    text: text.clone(),
                    href: href.clone(),
                });
            }
            PageEntry::Section { pages, .. } => {
                out.extend(collect_pages(pages));
            }
        }
    }
    out
}

/// Read page titles from YAML front matter.
pub(crate) fn read_page_titles(pages: &[FlatPage], base_dir: &Path) -> HashMap<String, String> {
    let mut titles = HashMap::new();
    for page in pages {
        let qmd_path = base_dir.join(&page.href);
        if let Ok(text) = fs::read_to_string(&qmd_path) {
            if let Ok((meta, _)) = crate::parse::yaml::split_yaml(&text) {
                if let Some(title) = meta.title {
                    titles.insert(page.href.clone(), title);
                }
            }
        }
    }
    titles
}

/// Derive a display title from a filename: "front_matter.qmd" → "Front Matter"
pub(crate) fn title_from_filename(href: &str) -> String {
    let stem = Path::new(href)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(href);
    stem.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    format!("{}{}", upper, chars.collect::<String>())
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Get the display title for a page.
pub(crate) fn page_display_title(
    href: &str,
    explicit_text: Option<&str>,
    titles: &HashMap<String, String>,
) -> String {
    if let Some(text) = explicit_text {
        return text.to_string();
    }
    if let Some(title) = titles.get(href) {
        return title.clone();
    }
    title_from_filename(href)
}

// ---------------------------------------------------------------------------
// Core page rendering
// ---------------------------------------------------------------------------

/// Render a single .qmd page, returning (body_html, metadata, syntax_css).
/// Uses `DataTheme` scoping for light/dark CSS (Starlight/Astro compatibility).
pub(crate) fn render_page_bare(
    input: &Path,
    output_path: &Path,
    format_overrides: &[String],
) -> Result<(String, crate::types::Metadata, String)> {
    let result = crate::render_core(input, output_path, Some("html"), format_overrides)?;
    let syntax_css = result.element_renderer.syntax_css_with_scope(
        crate::filters::highlighting::ColorScope::DataTheme,
    );
    Ok((result.rendered, result.metadata, syntax_css))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
