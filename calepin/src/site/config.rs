use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

/// Top-level site configuration, parsed from `_calepin.yaml` or `_site.yml`.
#[derive(Debug, Deserialize)]
pub struct SiteConfig {
    #[serde(default)]
    pub project: ProjectConfig,
    #[serde(default)]
    pub website: WebsiteConfig,
    #[serde(default)]
    pub format: FormatConfig,
    #[serde(default)]
    pub execute: ExecuteConfig,
    /// Brand configuration, parsed separately from raw YAML.
    #[serde(skip)]
    pub brand: Option<crate::brand::Brand>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ExecuteConfig {
    #[serde(default)]
    pub cache: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ProjectConfig {
    #[serde(default)]
    pub resources: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct WebsiteConfig {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(rename = "site-url", default)]
    pub site_url: Option<String>,
    #[serde(default)]
    pub favicon: Option<String>,
    #[serde(default)]
    pub navbar: NavbarConfig,
    #[serde(default)]
    pub sidebar: SidebarConfig,
    #[serde(default)]
    pub pages: Vec<PageEntry>,
}

#[derive(Debug, Default, Deserialize)]
pub struct NavbarConfig {
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(rename = "logo-dark", default)]
    pub logo_dark: Option<String>,
    #[serde(rename = "logo-alt", default)]
    pub logo_alt: Option<String>,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub left: Vec<NavItem>,
    #[serde(default)]
    pub right: Vec<NavItem>,
    #[serde(default)]
    pub search: bool,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct NavItem {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub href: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct SidebarConfig {
    #[serde(rename = "collapse-level", default = "default_collapse_level")]
    pub collapse_level: usize,
}

fn default_collapse_level() -> usize {
    1
}

/// A page entry in the sidebar navigation tree.
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum PageEntry {
    /// A simple page reference: just a filename string like "intro.qmd"
    Simple(String),
    /// A page with explicit text/href
    Page {
        text: Option<String>,
        href: String,
        #[serde(default)]
        icon: Option<String>,
    },
    /// A section with nested pages
    Section {
        section: String,
        #[serde(default)]
        pages: Vec<PageEntry>,
    },
}

#[derive(Debug, Default, Deserialize)]
pub struct FormatConfig {
    #[serde(default)]
    pub html: Option<HtmlFormatConfig>,
}

#[derive(Debug, Default, Deserialize)]
pub struct HtmlFormatConfig {
    #[serde(default)]
    pub toc: Option<bool>,
    #[serde(rename = "highlight-style", default)]
    pub highlight_style: Option<HighlightStyle>,
    #[serde(rename = "code-copy", default)]
    pub code_copy: Option<bool>,
    #[serde(rename = "code-overflow", default)]
    pub code_overflow: Option<String>,
    #[serde(rename = "html-math-method", default)]
    pub html_math_method: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum HighlightStyle {
    Single(String),
    DualTheme {
        light: String,
        dark: String,
    },
}

impl SiteConfig {
    /// Load config from file. Tries `_calepin.toml`, `_calepin.yaml`, then `_site.yml`.
    pub fn load(config_path: Option<&Path>, base_dir: &Path) -> Result<(Self, PathBuf)> {
        if let Some(path) = config_path {
            let text = fs::read_to_string(path)
                .with_context(|| format!("Failed to read config: {}", path.display()))?;
            let mut config: SiteConfig = parse_site_config(&text, path)?;
            config.brand = parse_brand_from_text(&text, path);
            return Ok((config, path.to_path_buf()));
        }

        // Try default names
        for name in &["_calepin.toml", "_calepin.yaml", "_calepin.yml", "_site.yml", "_site.yaml"] {
            let path = base_dir.join(name);
            if path.exists() {
                let text = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read config: {}", path.display()))?;
                let mut config: SiteConfig = parse_site_config(&text, &path)?;
                config.brand = parse_brand_from_text(&text, &path);
                return Ok((config, path));
            }
        }

        anyhow::bail!(
            "No site config found. Create _calepin.toml or _calepin.yaml in {}",
            base_dir.display()
        );
    }

    /// Collect all .qmd files referenced in the pages tree (in order).
    pub fn collect_page_paths(&self) -> Vec<String> {
        let mut paths = Vec::new();
        collect_paths_recursive(&self.website.pages, &mut paths);
        paths
    }
}

fn collect_paths_recursive(entries: &[PageEntry], out: &mut Vec<String>) {
    for entry in entries {
        match entry {
            PageEntry::Simple(s) => {
                if s.ends_with(".qmd") {
                    out.push(s.clone());
                }
            }
            PageEntry::Page { href, .. } => {
                if href.ends_with(".qmd") {
                    out.push(href.clone());
                }
            }
            PageEntry::Section { pages, .. } => {
                collect_paths_recursive(pages, out);
            }
        }
    }
}

/// Parse site config text (TOML or YAML depending on file extension).
fn parse_site_config(text: &str, path: &Path) -> Result<SiteConfig> {
    let is_toml = path.extension().map_or(false, |e| e == "toml");
    if is_toml {
        toml::from_str(text)
            .with_context(|| format!("Failed to parse config: {}", path.display()))
    } else {
        // For YAML site configs, use the toml deserializer on the YAML
        // by first converting through our Value type, then to JSON, then deserializing.
        let table = crate::value::parse_minimal_yaml(text);
        let json_val = crate::value::to_json(&crate::value::Value::Table(table));
        serde_json::from_value(json_val)
            .with_context(|| format!("Failed to parse config: {}", path.display()))
    }
}

/// Extract `brand:` from config text and parse into a Brand.
fn parse_brand_from_text(text: &str, path: &Path) -> Option<crate::brand::Brand> {
    let is_toml = path.extension().map_or(false, |e| e == "toml");
    let table = if is_toml {
        let tv: toml::Value = toml::from_str(text).ok()?;
        match tv {
            toml::Value::Table(map) => crate::value::table_from_toml(map),
            _ => return None,
        }
    } else {
        crate::value::parse_minimal_yaml(text)
    };
    let brand_val = crate::value::table_get(&table, "brand")?;
    crate::brand::parse_brand_from_value(brand_val)
}
