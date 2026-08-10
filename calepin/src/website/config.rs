use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::feeds::FeedsConfig;

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default, deny_unknown_fields)]
pub(super) struct WebsiteConfig {
    #[serde(rename(deserialize = "default-language"), alias = "default_language")]
    pub(super) default_language: Option<String>,
    pub(super) languages: BTreeMap<String, LanguageConfig>,
    #[serde(rename = "executables")]
    pub(super) _executables: Option<toml::Value>,
    pub(super) theme: Option<toml::Value>,
    pub(super) store: BTreeMap<String, toml::Value>,
    // Resolved from `CalepinConfig` (config.rs), not here; this field only
    // exists so `deny_unknown_fields` accepts `[toc]` in calepin.toml.
    #[serde(rename = "toc")]
    pub(super) _toc: Option<toml::Value>,
    pub(super) title: Option<String>,
    pub(super) description: Option<String>,
    #[serde(rename(deserialize = "base-url"), alias = "base_url")]
    pub(super) base_url: Option<String>,
    pub(super) logo: Option<String>,
    #[serde(rename(deserialize = "logo-alt"), alias = "logo_alt")]
    pub(super) logo_alt: Option<String>,
    pub(super) favicon: Option<String>,
    #[serde(rename = "asset-dir")]
    pub(super) asset_dir: Option<PathBuf>,
    /// Default output directory for website builds when no output path is
    /// given on the command line. Relative paths resolve from the directory
    /// holding `calepin.toml`.
    #[serde(rename = "output-dir", alias = "output")]
    pub(super) output_dir: Option<PathBuf>,
    #[serde(rename = "highlight-light")]
    pub(super) highlight_light: Option<PathBuf>,
    #[serde(rename = "highlight-dark")]
    pub(super) highlight_dark: Option<PathBuf>,
    /// Also render a PDF for every page; pages can override with `pdf` in
    /// their `<website-metadata>`.
    pub(super) pdf: Option<bool>,
    /// Publish each page's Typst source (default `true`): copies `.typ` files
    /// into the output directory and embeds the source in the rendered HTML for
    /// the view-source mode. `false` suppresses both.
    pub(super) typ: Option<bool>,
    /// Minify generated HTML after theming and website metadata injection.
    pub(super) minify: Option<bool>,
    pub(super) search: Option<SearchEngine>,
    /// Deprecated toggle kept for backward compatibility; prefer `feeds =
    /// true` or a `[feeds]` table. When set, it takes precedence.
    #[serde(rename(deserialize = "generate-feeds"), alias = "generate_feeds")]
    pub(super) generate_feeds: Option<bool>,
    pub(super) feeds: Option<RawFeedsConfig>,
    pub(super) robots: Option<RawRobotsConfig>,
    pub(super) pages: Option<PagesConfig>,
    #[serde(rename = "static")]
    pub(super) static_files: Option<StaticConfig>,
    pub(super) menus: BTreeMap<String, Vec<MenuItemConfig>>,
    pub(super) footer: Option<FooterConfig>,
    pub(super) sidebar: Option<SidebarConfig>,
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

/// `feeds` accepts a bare toggle (`feeds = true`) or a `[feeds]` table, which
/// implies the toggle.
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub(super) enum RawFeedsConfig {
    Toggle(bool),
    Config(FeedsConfig),
}

impl WebsiteConfig {
    pub(super) fn robots_enabled(&self) -> bool {
        match &self.robots {
            None => true,
            Some(RawRobotsConfig::Toggle(enabled)) => *enabled,
            Some(RawRobotsConfig::Config(config)) => config.enabled,
        }
    }

    pub(super) fn feeds_enabled(&self) -> bool {
        match self.generate_feeds {
            Some(enabled) => enabled,
            None => match &self.feeds {
                None => false,
                Some(RawFeedsConfig::Toggle(enabled)) => *enabled,
                Some(RawFeedsConfig::Config(_)) => true,
            },
        }
    }

    pub(super) fn feeds_config(&self) -> Option<&FeedsConfig> {
        match &self.feeds {
            Some(RawFeedsConfig::Config(config)) => Some(config),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(default, deny_unknown_fields)]
pub(super) struct LanguageConfig {
    pub(super) label: Option<String>,
    #[serde(rename(deserialize = "content-dir"), alias = "content_dir")]
    pub(super) content_dir: Option<PathBuf>,
    #[serde(rename(deserialize = "url-prefix"), alias = "url_prefix")]
    pub(super) url_prefix: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct SidebarConfig {
    #[serde(rename(deserialize = "show-hidden"), alias = "show_hidden")]
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
#[serde(default, deny_unknown_fields)]
pub(super) struct SidebarItemConfig {
    #[serde(alias = "path", alias = "url")]
    pub(super) target: Option<String>,
    pub(super) glob: Option<String>,
    pub(super) label: Option<String>,
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
#[serde(default, deny_unknown_fields)]
pub(super) struct FooterConfig {
    pub(super) item: Vec<MenuItemConfig>,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
#[serde(default, deny_unknown_fields)]
pub(super) struct MenuItemConfig {
    #[serde(alias = "path", alias = "url")]
    pub(super) target: Option<String>,
    pub(super) glob: Option<String>,
    pub(super) label: Option<String>,
    #[serde(rename = "aria-label")]
    pub(super) aria_label: Option<String>,
    pub(super) weight: Option<i32>,
}
