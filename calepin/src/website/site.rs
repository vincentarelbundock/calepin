use std::path::Path;

use anyhow::{Context, Result};

use crate::html::{
    SiteContextInput, SiteLanguageEntry, SiteNavEntry, SiteNavSection, SitePagefindEntry,
};
use crate::utils::html::escape as html_escape;

use super::config::{SearchEngine, WebsiteConfig};
use super::language::LanguageInfo;
use super::navigation::{MenusModel, NavSectionModel};
use super::pagefind::{PAGEFIND_CSS, PAGEFIND_DIR, PAGEFIND_JS};
use super::paths::{normalize_path, slash_path};
use super::url::{absolute_site_url, is_absolute_or_special_url, page_relative_url};
use super::util::clean_optional_string;
use super::{PageInfo, PageInfoMap, DEFAULT_FAVICON_PATH};

#[derive(Debug, Clone, Default)]
pub(super) struct SiteMetadata {
    pub(super) title: Option<String>,
    pub(super) description: Option<String>,
    pub(super) base_url: Option<String>,
    pub(super) logo: Option<String>,
    pub(super) logo_alt: Option<String>,
    pub(super) favicon: Option<String>,
}

impl SiteMetadata {
    pub(super) fn from_config(config: &WebsiteConfig, src_dir: &Path) -> Result<Self> {
        Ok(Self {
            title: clean_optional_string(config.title.as_deref()),
            description: clean_optional_string(config.description.as_deref()),
            base_url: clean_optional_string(config.base_url.as_deref())
                .map(|url| url.trim_end_matches('/').to_string()),
            logo: source_asset_output_path(src_dir, config.logo.as_deref(), "logo")?,
            logo_alt: clean_optional_string(config.logo_alt.as_deref())
                .or_else(|| clean_optional_string(config.title.as_deref())),
            favicon: source_asset_output_path(src_dir, config.favicon.as_deref(), "favicon")?
                .or_else(|| Some(DEFAULT_FAVICON_PATH.to_string())),
        })
    }
}

fn source_asset_output_path(
    src_dir: &Path,
    value: Option<&str>,
    key: &str,
) -> Result<Option<String>> {
    let Some(value) = clean_optional_string(value) else {
        return Ok(None);
    };
    if is_absolute_or_special_url(&value) {
        return Ok(Some(value));
    }
    let path = Path::new(&value);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        src_dir.join(path)
    };
    let normalized = normalize_path(&absolute);
    let src_dir = normalize_path(src_dir);
    let rel = normalized.strip_prefix(&src_dir).with_context(|| {
        format!("website `{key}` path must stay inside the source directory: {value}")
    })?;
    Ok(Some(slash_path(rel)))
}

#[derive(Debug)]
pub(super) struct SiteModel {
    sections: Vec<NavSectionModel>,
    menus: MenusModel,
    metadata: SiteMetadata,
    sidebar_fold: bool,
}

impl SiteModel {
    pub(super) fn new(
        sections: Vec<NavSectionModel>,
        menus: MenusModel,
        metadata: SiteMetadata,
        sidebar_fold: bool,
    ) -> Self {
        Self {
            sections,
            menus,
            metadata,
            sidebar_fold,
        }
    }

    pub(super) fn theme_context(
        &self,
        current_href: &str,
        page_info: Option<&PageInfo>,
        page_info_map: &PageInfoMap,
        languages: Option<&[LanguageInfo]>,
        search: Option<SearchEngine>,
    ) -> SiteContextInput {
        let mut sidebar = Vec::new();
        let mut sidebar_sections = Vec::new();
        let mut page_title = None;
        let current_language = page_info.and_then(|info| info.language.as_deref());

        for section in &self.sections {
            if section.language.as_deref() != current_language {
                continue;
            }
            let mut items = Vec::new();
            for item in &section.items {
                if item.href == current_href {
                    page_title = Some(html_escape(&item.label));
                }
                let entry = SiteNavEntry {
                    href: html_escape(&page_relative_url(current_href, &item.href)),
                    label: html_escape(&item.label),
                    label_html: item.label_html.clone(),
                    active: item.href == current_href,
                };
                sidebar.push(entry.clone());
                items.push(entry);
            }
            sidebar_sections.push(SiteNavSection {
                title: section.title.as_ref().map(|title| html_escape(title)),
                active: items.iter().any(|item| item.active),
                items,
            });
        }
        let language_entries = languages
            .map(|languages| language_entries(current_href, page_info, page_info_map, languages))
            .unwrap_or_default();
        let translations = page_info
            .and_then(|info| {
                languages.map(|languages| {
                    translation_entries(current_href, info, page_info_map, languages)
                })
            })
            .unwrap_or_default();
        let menus = self
            .menus
            .entries_for_current_page(current_href, current_language);
        let menu_list = menus
            .iter()
            .map(|(name, items)| crate::html::SiteMenu {
                name: name.clone(),
                items: items.clone(),
            })
            .collect();

        SiteContextInput {
            sidebar,
            sidebar_sections,
            sidebar_fold: self.sidebar_fold,
            menus,
            menu_list,
            languages: language_entries,
            translations,
            language: current_language.map(str::to_string),
            title: self.metadata.title.as_deref().map(html_escape),
            description: self.metadata.description.as_deref().map(html_escape),
            base_url: self.metadata.base_url.as_deref().map(html_escape),
            logo: self
                .metadata
                .logo
                .as_deref()
                .map(|logo| html_escape(&page_relative_url(current_href, logo))),
            logo_alt: self.metadata.logo_alt.as_deref().map(html_escape),
            home_url: Some(html_escape(&page_relative_url(current_href, "index.html"))),
            favicon: self
                .metadata
                .favicon
                .as_deref()
                .map(|favicon| html_escape(&page_relative_url(current_href, favicon))),
            current_url: self
                .metadata
                .base_url
                .as_deref()
                .map(|base_url| html_escape(&absolute_site_url(base_url, current_href))),
            page_title,
            stylesheet: None,
            scripts: Vec::new(),
            pagefind: (search == Some(SearchEngine::Pagefind)).then(|| SitePagefindEntry {
                css: html_escape(&page_relative_url(current_href, PAGEFIND_CSS)),
                js: html_escape(&page_relative_url(current_href, PAGEFIND_JS)),
                bundle: html_escape(&page_relative_url(
                    current_href,
                    &format!("{PAGEFIND_DIR}/"),
                )),
            }),
            revealjs: String::new(),
        }
    }
}

pub(super) fn language_entries(
    current_href: &str,
    current: Option<&PageInfo>,
    page_info: &PageInfoMap,
    languages: &[LanguageInfo],
) -> Vec<SiteLanguageEntry> {
    languages
        .iter()
        .map(|language| {
            let href = current
                .and_then(|current| {
                    page_info.values().find(|info| {
                        info.translation_key == current.translation_key
                            && info.language.as_deref() == Some(language.code.as_str())
                    })
                })
                .map(|info| info.href.clone())
                .unwrap_or_else(|| language_home_href(language));
            SiteLanguageEntry {
                code: language.code.clone(),
                label: html_escape(&language.label),
                href: html_escape(&page_relative_url(current_href, &href)),
                active: current
                    .and_then(|info| info.language.as_deref())
                    .is_some_and(|code| code == language.code),
            }
        })
        .collect()
}

fn language_home_href(language: &LanguageInfo) -> String {
    if language.url_prefix.is_empty() {
        "index.html".to_string()
    } else {
        format!("{}/index.html", language.url_prefix)
    }
}

pub(super) fn translation_entries(
    current_href: &str,
    current: &PageInfo,
    page_info: &PageInfoMap,
    languages: &[LanguageInfo],
) -> Vec<SiteLanguageEntry> {
    languages
        .iter()
        .filter_map(|language| {
            page_info
                .values()
                .find(|info| {
                    info.translation_key == current.translation_key
                        && info.language.as_deref() == Some(language.code.as_str())
                })
                .map(|info| SiteLanguageEntry {
                    code: language.code.clone(),
                    label: html_escape(&language.label),
                    href: html_escape(&page_relative_url(current_href, &info.href)),
                    active: info.language == current.language,
                })
        })
        .collect()
}
