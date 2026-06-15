use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::feeds::FeedsConfig;

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default, deny_unknown_fields)]
pub(super) struct WebsiteConfig {
    pub(super) default_language: Option<String>,
    pub(super) languages: BTreeMap<String, LanguageConfig>,
    #[serde(rename = "executables")]
    pub(super) _executables: Option<toml::Value>,
    pub(super) theme: Option<RawThemeValue>,
    pub(super) title: Option<String>,
    pub(super) description: Option<String>,
    pub(super) base_url: Option<String>,
    pub(super) logo: Option<String>,
    pub(super) logo_alt: Option<String>,
    pub(super) favicon: Option<String>,
    #[serde(rename = "highlight-light")]
    pub(super) highlight_light: Option<PathBuf>,
    #[serde(rename = "highlight-dark")]
    pub(super) highlight_dark: Option<PathBuf>,
    /// Also render a PDF for every page; pages can override with `pdf` in
    /// their `<website-metadata>`.
    pub(super) pdf: Option<bool>,
    /// Minify generated HTML after theming and website metadata injection.
    pub(super) minify: Option<bool>,
    pub(super) search: Option<SearchEngine>,
    pub(super) generate_feeds: Option<bool>,
    pub(super) feeds: Option<FeedsConfig>,
    pub(super) robots: Option<RawRobotsConfig>,
    pub(super) pages: Option<PagesConfig>,
    #[serde(rename = "static")]
    pub(super) static_files: Option<StaticConfig>,
    pub(super) navbar: Option<NavbarConfig>,
    pub(super) sidebar: Option<SidebarConfig>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub(super) enum RawThemeValue {
    Enabled(String),
    Toggle(bool),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub(super) enum RawRobotsConfig {
    Toggle(bool),
    Config(RobotsConfig),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum SearchEngine {
    Pagefind,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct RobotsConfig {
    pub(super) enabled: bool,
}

impl Default for RobotsConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl WebsiteConfig {
    pub(super) fn theme_selection(
        &self,
        config_dir: &Path,
    ) -> Result<crate::theme::ThemeSelection> {
        match &self.theme {
            None => Ok(crate::theme::ThemeSelection::Default),
            Some(RawThemeValue::Toggle(false)) => Ok(crate::theme::ThemeSelection::Disabled),
            Some(RawThemeValue::Toggle(true)) => Ok(crate::theme::ThemeSelection::Default),
            Some(RawThemeValue::Enabled(value)) => {
                crate::theme::ThemeSelection::parse(value, config_dir)
            }
        }
    }

    pub(super) fn robots_enabled(&self) -> bool {
        match &self.robots {
            None => true,
            Some(RawRobotsConfig::Toggle(enabled)) => *enabled,
            Some(RawRobotsConfig::Config(config)) => config.enabled,
        }
    }

    pub(super) fn feeds_enabled(&self) -> bool {
        self.generate_feeds.unwrap_or(false)
    }
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub(super) struct LanguageConfig {
    pub(super) label: Option<String>,
    pub(super) content_dir: Option<PathBuf>,
    pub(super) url_prefix: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct SidebarConfig {
    pub(super) show_hidden: bool,
    pub(super) fold: bool,
    pub(super) section: Vec<SidebarSectionConfig>,
}

impl Default for SidebarConfig {
    fn default() -> Self {
        Self {
            show_hidden: false,
            fold: true,
            section: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub(super) struct SidebarSectionConfig {
    pub(super) title: Option<String>,
    pub(super) item: Vec<SidebarItemConfig>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub(super) struct SidebarItemConfig {
    #[serde(alias = "path", alias = "url")]
    pub(super) target: Option<String>,
    pub(super) glob: Option<String>,
    pub(super) label: Option<String>,
    pub(super) icon: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub(super) struct PagesConfig {
    pub(super) include: Vec<String>,
    pub(super) exclude: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default, deny_unknown_fields)]
pub(super) struct StaticConfig {
    pub(super) include: Vec<String>,
    pub(super) exclude: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default)]
pub(super) struct NavbarConfig {
    pub(super) show_hidden: bool,
    pub(super) item: Vec<NavbarItemConfig>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default, deny_unknown_fields)]
pub(super) struct NavbarItemConfig {
    pub(super) position: NavbarPosition,
    #[serde(alias = "path", alias = "url")]
    pub(super) target: Option<String>,
    pub(super) glob: Option<String>,
    pub(super) label: Option<String>,
    pub(super) icon: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(super) enum NavbarPosition {
    #[default]
    Left,
    Center,
    Right,
}
