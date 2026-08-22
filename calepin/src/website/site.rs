use std::path::Path;

use anyhow::{anyhow, bail, Result};

use crate::html::{
    SiteContextInput, SiteLanguageEntry, SiteNavEntry, SiteNavSection, SitePagefindEntry,
};
use crate::utils::html::escape as html_escape;

use super::config::{SearchEngine, WebsiteConfig};
use super::language::LanguageInfo;
use super::navigation::{MenusModel, NavItemModel, NavSectionModel};
use super::pagefind::{base_url_path_prefix, PAGEFIND_CSS, PAGEFIND_DIR, PAGEFIND_JS};
use super::paths::{join_normalized_under_root, normalize_path, slash_path};
use super::url::{
    absolute_site_url, absolute_site_url_without_index, is_absolute_or_special_url, LinkStyle,
};
use super::util::clean_optional_string;
use super::{PageInfo, PageInfoMap};

#[derive(Debug, Clone, Default)]
pub(super) struct SiteMetadata {
    pub(super) title: Option<String>,
    pub(super) description: Option<String>,
    pub(super) base_url: Option<String>,
    pub(super) logo: Option<String>,
    pub(super) logo_alt: Option<String>,
    pub(super) favicon: Option<String>,
    pub(super) image: Option<String>,
    pub(super) theme_color: Option<String>,
}

impl SiteMetadata {
    pub(super) fn from_config(
        config: &WebsiteConfig,
        src_dir: &Path,
        default_favicon_path: &str,
    ) -> Result<Self> {
        Ok(Self {
            title: clean_optional_string(config.title.as_deref()),
            description: clean_optional_string(config.description.as_deref()),
            base_url: clean_optional_string(config.base_url.as_deref())
                .map(|url| url.trim_end_matches('/').to_string()),
            logo: source_asset_output_path(src_dir, config.logo.as_deref(), "logo")?,
            logo_alt: clean_optional_string(config.logo_alt.as_deref())
                .or_else(|| clean_optional_string(config.title.as_deref())),
            favicon: source_asset_output_path(src_dir, config.favicon.as_deref(), "favicon")?
                .or_else(|| Some(default_favicon_path.to_string())),
            image: source_asset_output_path(src_dir, config.image.as_deref(), "image")?,
            theme_color: clean_optional_string(config.theme_color.as_deref()),
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
    let root = normalize_path(src_dir);
    let what = format!("website `{key}` path must stay inside the source directory: {value}");
    let candidate = join_normalized_under_root(&root, path, &what)?;
    let rel = candidate.strip_prefix(&root).map_err(|_| {
        anyhow!("website `{key}` path must stay inside the source directory: {value}")
    })?;
    if rel.as_os_str().is_empty() {
        bail!("website `{key}` path must stay inside the source directory: {value}");
    }
    Ok(Some(slash_path(rel)))
}

/// Per-page facts that shape the rendered chrome.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct PageFlags {
    /// Link to the page source alongside the rendered page.
    pub(super) publish_source: bool,
    /// This is the website fallback page, served by the host under whatever
    /// URL was requested rather than under its own.
    pub(super) fallback: bool,
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

    /// URL prefix that a page served from arbitrary request paths must use to
    /// address the site root: the path component of the configured `base-url`,
    /// or empty when the site is hosted at a domain root.
    pub(super) fn site_root_prefix(&self) -> String {
        self.metadata
            .base_url
            .as_deref()
            .and_then(base_url_path_prefix)
            .unwrap_or_default()
    }

    pub(super) fn theme_context(
        &self,
        current_href: &str,
        page_info: Option<&PageInfo>,
        page_info_map: &PageInfoMap,
        languages: Option<&[LanguageInfo]>,
        search: Option<SearchEngine>,
        page: PageFlags,
    ) -> SiteContextInput {
        let PageFlags {
            publish_source,
            fallback,
        } = page;
        // The fallback page is served under whatever URL was requested, so
        // links relative to the page itself resolve against the wrong
        // directory. It addresses the site from the root instead.
        let root_prefix = if fallback {
            self.site_root_prefix()
        } else {
            String::new()
        };
        let links = if fallback {
            LinkStyle::SiteRoot {
                prefix: &root_prefix,
            }
        } else {
            LinkStyle::PageRelative
        };
        let mut sidebar = Vec::new();
        let mut sidebar_sections = Vec::new();
        let mut page_title = None;
        let current_language = page_info.and_then(|info| info.language.as_deref());

        for section in &self.sections {
            if section
                .language
                .as_deref()
                .is_some_and(|section_language| Some(section_language) != current_language)
            {
                continue;
            }
            let mut builder = SidebarBuilder {
                current_href,
                links,
                language_scoped: section.language.is_some(),
                flat: &mut sidebar,
                page_title: &mut page_title,
            };
            sidebar_sections.push(builder.section(section, 0));
        }
        let language_entries = languages
            .map(|languages| {
                language_entries(current_href, page_info, page_info_map, languages, links)
            })
            .unwrap_or_default();
        let translations = page_info
            .and_then(|info| {
                languages.map(|languages| {
                    translation_entries(current_href, info, page_info_map, languages, links)
                })
            })
            .unwrap_or_default();
        let menus = self
            .menus
            .entries_for_current_page(current_href, current_language, links);

        SiteContextInput {
            sidebar,
            sidebar_sections,
            sidebar_fold: self.sidebar_fold,
            menus,
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
                .map(|logo| html_escape(&links.resolve(current_href, logo))),
            logo_alt: self.metadata.logo_alt.as_deref().map(html_escape),
            home_url: Some(html_escape(&links.resolve(current_href, "index.html"))),
            favicon: self
                .metadata
                .favicon
                .as_deref()
                .map(|favicon| html_escape(&links.resolve(current_href, favicon))),
            page_url: self.metadata.base_url.as_deref().map(|base_url| {
                html_escape(&absolute_site_url_without_index(base_url, current_href))
            }),
            page_title,
            page_image: social_image_url(
                self.metadata.base_url.as_deref(),
                page_info
                    .and_then(|info| info.image.as_deref())
                    .or(self.metadata.image.as_deref()),
            )
            .as_deref()
            .map(html_escape),
            page_description: page_info
                .and_then(|info| info.description.as_deref())
                .or(self.metadata.description.as_deref())
                .map(html_escape),
            theme_color: self.metadata.theme_color.as_deref().map(html_escape),
            page_pdf: page_info
                .and_then(|info| info.pdf_href.as_deref())
                .map(|pdf| html_escape(&links.resolve(current_href, pdf))),
            page_source: publish_source,
            pagefind: (search == Some(SearchEngine::Pagefind)).then(|| SitePagefindEntry {
                css: html_escape(&links.resolve(current_href, PAGEFIND_CSS)),
                js: html_escape(&links.resolve(current_href, PAGEFIND_JS)),
                bundle: html_escape(&links.resolve(current_href, &format!("{PAGEFIND_DIR}/"))),
            }),
            store: Default::default(),
            toc_depth: None,
            toc_floating: None,
        }
    }
}

/// Turns a nav section tree into theme-facing sections, resolving hrefs against
/// the current page. It also accumulates the flat `site.sidebar` list in
/// reading order (a section's own items, then each subsection's), which drives
/// previous/next page navigation.
struct SidebarBuilder<'a> {
    current_href: &'a str,
    links: LinkStyle<'a>,
    /// Section belongs to one language, so the current page's own link stays
    /// verbatim rather than being resolved relative to itself.
    language_scoped: bool,
    flat: &'a mut Vec<SiteNavEntry>,
    page_title: &'a mut Option<String>,
}

impl SidebarBuilder<'_> {
    fn section(&mut self, section: &NavSectionModel, depth: usize) -> SiteNavSection {
        let items = section
            .items
            .iter()
            .map(|item| self.entry(item))
            .collect::<Vec<_>>();
        let sections = section
            .sections
            .iter()
            .map(|nested| self.section(nested, depth + 1))
            .collect::<Vec<_>>();
        let active =
            items.iter().any(|item| item.active) || sections.iter().any(|nested| nested.active);
        SiteNavSection {
            title: section.title.as_ref().map(|title| html_escape(title)),
            active,
            items,
            sections,
            depth,
        }
    }

    fn entry(&mut self, item: &NavItemModel) -> SiteNavEntry {
        let is_current_page = !item.href.is_empty() && item.href == self.current_href;
        if is_current_page {
            *self.page_title = Some(html_escape(&item.label));
        }
        let item_href = if item.href.is_empty() {
            String::new()
        } else if is_current_page && self.language_scoped {
            item.href.clone()
        } else {
            self.links.resolve(self.current_href, &item.href)
        };
        let entry = SiteNavEntry {
            href: html_escape(&item_href),
            label: html_escape(&item.label),
            label_html: item.label_html.clone(),
            active: is_current_page,
        };
        self.flat.push(entry.clone());
        entry
    }
}

/// Resolves a social-card image to the absolute URL that Open Graph and
/// Twitter scrapers need. Already-absolute URLs pass through; a site-relative
/// path needs `base_url` to become absolute, so without one the image is
/// dropped rather than emitted in a form no scraper will follow.
fn social_image_url(base_url: Option<&str>, image: Option<&str>) -> Option<String> {
    let image = clean_optional_string(image)?;
    if image.starts_with("http://") || image.starts_with("https://") || image.starts_with("//") {
        return Some(image);
    }
    base_url.map(|base_url| absolute_site_url(base_url, &image))
}

pub(super) fn language_entries(
    current_href: &str,
    current: Option<&PageInfo>,
    page_info: &PageInfoMap,
    languages: &[LanguageInfo],
    links: LinkStyle,
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
                href: html_escape(&links.resolve(current_href, &href)),
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
    links: LinkStyle,
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
                    href: html_escape(&links.resolve(current_href, &info.href)),
                    active: info.language == current.language,
                })
        })
        .collect()
}
