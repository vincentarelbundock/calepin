use std::collections::HashMap;

use serde::Serialize;
use tera;

use super::config::{NavItem, PageEntry, SiteConfig};
use super::discover::PageInfo;
use super::icons;
use super::render::SiteRenderResult;

/// Site-level context available to all templates.
#[derive(Debug, Serialize)]
pub struct SiteContext {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub url: Option<String>,
    pub favicon: Option<String>,
    pub navbar: NavbarContext,
    pub sidebar: SidebarContext,
    pub pages: Vec<NavNode>,
    pub dark_mode: bool,
}

#[derive(Debug, Serialize)]
pub struct NavbarContext {
    pub logo: Option<String>,
    pub logo_alt: Option<String>,
    pub background: Option<String>,
    pub left: Vec<NavItemContext>,
    pub right: Vec<NavItemContext>,
    pub search: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct NavItemContext {
    pub text: Option<String>,
    pub href: Option<String>,
    pub icon: Option<String>,
    pub icon_svg: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SidebarContext {
    pub collapse_level: usize,
}

/// A node in the navigation tree (for sidebar rendering).
#[derive(Debug, Clone, Serialize)]
pub struct NavNode {
    pub text: String,
    pub href: Option<String>,
    pub active: bool,
    pub children: Vec<NavNode>,
}

/// Per-page context.
#[derive(Debug, Serialize)]
pub struct PageContext {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub date: Option<String>,
    pub r#abstract: Option<String>,
    pub body: String,
    pub url: String,
    pub toc: Option<String>,
    pub listing: Option<Vec<ListingItem>>,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub prev: Option<NavLink>,
    pub next: Option<NavLink>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListingItem {
    pub title: Option<String>,
    pub date: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Breadcrumb {
    pub text: String,
    pub href: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NavLink {
    pub text: String,
    pub href: String,
}

/// Build the site-level context from config.
pub fn build_site_context(config: &SiteConfig, pages: &[PageInfo]) -> SiteContext {
    let nav_items_left = config
        .website
        .navbar
        .left
        .iter()
        .map(nav_item_to_context)
        .collect();
    let nav_items_right = config
        .website
        .navbar
        .right
        .iter()
        .map(nav_item_to_context)
        .collect();

    let nav_tree = build_nav_tree(&config.website.pages, pages, "");

    SiteContext {
        title: config.website.title.clone(),
        subtitle: config.website.subtitle.clone(),
        url: config.website.site_url.clone(),
        favicon: config.website.favicon.clone(),
        navbar: NavbarContext {
            logo: config.website.navbar.logo.clone(),
            logo_alt: config.website.navbar.logo_alt.clone(),
            background: config.website.navbar.background.clone(),
            left: nav_items_left,
            right: nav_items_right,
            search: config.website.navbar.search,
        },
        sidebar: SidebarContext {
            collapse_level: config.website.sidebar.collapse_level,
        },
        pages: nav_tree,
        dark_mode: true,
    }
}

fn nav_item_to_context(item: &NavItem) -> NavItemContext {
    let icon_svg = item.icon.as_deref().map(icons::get_icon_svg);
    NavItemContext {
        text: item.text.clone(),
        href: item.href.clone(),
        icon: item.icon.clone(),
        icon_svg,
    }
}

/// Build the navigation tree from config page entries, resolving titles from page metadata.
fn build_nav_tree(
    entries: &[PageEntry],
    pages: &[PageInfo],
    _current_url: &str,
) -> Vec<NavNode> {
    let page_map: HashMap<String, &PageInfo> = pages
        .iter()
        .map(|p| (p.source.display().to_string(), p))
        .collect();

    entries
        .iter()
        .map(|entry| match entry {
            PageEntry::Simple(path) => {
                let info = page_map.get(path.as_str());
                let text = info
                    .and_then(|p| p.meta.title.clone())
                    .unwrap_or_else(|| path.clone());
                let href = info.map(|p| p.url.clone());
                NavNode {
                    text,
                    href,
                    active: false,
                    children: vec![],
                }
            }
            PageEntry::Page { text, href, .. } => {
                let info = page_map.get(href.as_str());
                let display_text = text
                    .clone()
                    .or_else(|| info.and_then(|p| p.meta.title.clone()))
                    .unwrap_or_else(|| href.clone());
                let resolved_href = if href.ends_with(".qmd") {
                    info.map(|p| p.url.clone())
                } else {
                    Some(href.clone())
                };
                NavNode {
                    text: display_text,
                    href: resolved_href,
                    active: false,
                    children: vec![],
                }
            }
            PageEntry::Section { section, pages: sub } => {
                let children = build_nav_tree(sub, pages, _current_url);
                NavNode {
                    text: section.clone(),
                    href: None,
                    active: false,
                    children,
                }
            }
        })
        .collect()
}

/// Mark the active page in the nav tree.
pub fn mark_active(nodes: &mut [NavNode], current_url: &str) -> bool {
    for node in nodes.iter_mut() {
        node.active = false;
        if let Some(ref href) = node.href {
            if href == current_url {
                node.active = true;
                return true;
            }
        }
        if mark_active(&mut node.children, current_url) {
            node.active = true;
            return true;
        }
    }
    false
}

/// Build the per-page context.
pub fn build_page_context(
    page: &PageInfo,
    result: Option<&SiteRenderResult>,
    pages: &[PageInfo],
    listing_items: Option<Vec<ListingItem>>,
) -> PageContext {
    let body = result
        .map(|r| r.body.clone())
        .unwrap_or_default();

    // Use render result metadata if available, fall back to frontmatter
    let title = result
        .and_then(|r| r.title.clone())
        .or_else(|| page.meta.title.clone());
    let date = result
        .and_then(|r| r.date.clone())
        .or_else(|| page.meta.date.clone());
    let subtitle = result
        .and_then(|r| r.subtitle.clone())
        .or_else(|| page.meta.subtitle.clone());
    let abstract_text = result
        .and_then(|r| r.abstract_text.clone())
        .or_else(|| page.meta.r#abstract.clone());

    // Build prev/next links
    let idx = pages.iter().position(|p| p.source == page.source);
    let prev = idx.and_then(|i| {
        if i > 0 {
            let p = &pages[i - 1];
            Some(NavLink {
                text: p.meta.title.clone().unwrap_or_else(|| p.source.display().to_string()),
                href: p.url.clone(),
            })
        } else {
            None
        }
    });
    let next = idx.and_then(|i| {
        if i + 1 < pages.len() {
            let p = &pages[i + 1];
            Some(NavLink {
                text: p.meta.title.clone().unwrap_or_else(|| p.source.display().to_string()),
                href: p.url.clone(),
            })
        } else {
            None
        }
    });

    // Build breadcrumbs
    let breadcrumbs = build_breadcrumbs(page);

    PageContext {
        title,
        subtitle,
        date,
        r#abstract: abstract_text,
        body,
        url: page.url.clone(),
        toc: None, // TODO: extract from calepin output
        listing: listing_items,
        breadcrumbs,
        prev,
        next,
    }
}

fn build_breadcrumbs(page: &PageInfo) -> Vec<Breadcrumb> {
    let mut crumbs = vec![Breadcrumb {
        text: "Home".to_string(),
        href: Some("/".to_string()),
    }];

    // Add intermediate path components
    let components: Vec<_> = page.source.components().collect();
    if components.len() > 1 {
        for comp in components[..components.len() - 1].iter() {
            crumbs.push(Breadcrumb {
                text: comp.as_os_str().to_string_lossy().to_string(),
                href: None,
            });
        }
    }

    // Current page (no link)
    if let Some(title) = &page.meta.title {
        crumbs.push(Breadcrumb {
            text: title.clone(),
            href: None,
        });
    }

    crumbs
}

/// Build the full Tera context for a page.
pub fn build_tera_context(
    site: &SiteContext,
    page: &PageContext,
) -> tera::Context {
    let mut ctx = tera::Context::new();
    ctx.insert("site", site);
    ctx.insert("page", page);
    ctx
}
