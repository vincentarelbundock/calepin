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
    /// Load config from file. Tries `_calepin.yaml`, then `_site.yml`.
    pub fn load(config_path: Option<&Path>, base_dir: &Path) -> Result<(Self, PathBuf)> {
        if let Some(path) = config_path {
            let text = fs::read_to_string(path)
                .with_context(|| format!("Failed to read config: {}", path.display()))?;
            let mut config: SiteConfig = serde_saphyr::from_str(&text)
                .with_context(|| format!("Failed to parse config: {}", path.display()))?;
            config.brand = parse_brand_from_text(&text);
            return Ok((config, path.to_path_buf()));
        }

        // Try default names
        for name in &["_calepin.yaml", "_calepin.yml", "_site.yml", "_site.yaml"] {
            let path = base_dir.join(name);
            if path.exists() {
                let text = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read config: {}", path.display()))?;
                let mut config: SiteConfig = serde_saphyr::from_str(&text)
                    .with_context(|| format!("Failed to parse config: {}", path.display()))?;
                config.brand = parse_brand_from_text(&text);
                return Ok((config, path));
            }
        }

        anyhow::bail!(
            "No site config found. Create _calepin.yaml or _site.yml in {}",
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

/// Extract `brand:` from raw YAML text and parse into a Brand.
fn parse_brand_from_text(text: &str) -> Option<crate::brand::Brand> {
    use saphyr::LoadableYamlNode;
    let docs = saphyr::YamlOwned::load_from_str(text).ok()?;
    let root = docs.into_iter().next()?;
    let map = root.as_mapping()?;
    let brand_key = saphyr::YamlOwned::Value(saphyr::ScalarOwned::String("brand".to_string()));
    let brand_val = map.get(&brand_key)?;
    crate::brand::parse_brand_from_yaml(brand_val)
}
