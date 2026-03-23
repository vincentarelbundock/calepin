//! Site and page context for template rendering.
//!
//! The site builder passes structured context to Jinja templates.
//! `[meta]` and `[site]` provide the site context. `[var]` is passed
//! through as arbitrary template variables.

use std::collections::HashMap;

use serde::Serialize;
use super::discover::PageInfo;
use super::render::SiteRenderResult;
use crate::project::{ProjectConfig, PageNode};

/// Site-level context available to all templates as `{{ site.* }}`.
#[derive(Debug, Serialize)]
pub struct SiteContext {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub url: Option<String>,
    pub favicon: Option<String>,
    pub logo: Option<String>,
    pub logo_dark: Option<String>,
    pub pages: Vec<NavNode>,
    pub dark_mode: bool,
    pub math_block: String,
}

/// A node in the navigation tree (for sidebar rendering).
#[derive(Debug, Clone, Serialize)]
pub struct NavNode {
    pub text: String,
    pub href: Option<String>,
    pub active: bool,
    pub children: Vec<NavNode>,
}

/// Per-page context available to templates as `{{ page.* }}`.
#[derive(Debug, Serialize)]
pub struct PageContext {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub date: Option<String>,
    pub r#abstract: Option<String>,
    pub body: String,
    pub url: String,
    pub source_url: String,
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

/// Build the site-level context from project config.
pub fn build_site_context(
    config: &ProjectConfig,
    pages: &[PageInfo],
    base_dir: &std::path::Path,
) -> SiteContext {
    let meta = config.meta.as_ref();
    let site = config.site.as_ref();

    // Build nav tree from [site].pages
    let page_nodes = site.map(|s| s.expand_pages(base_dir)).unwrap_or_default();
    let nav_tree = build_nav_tree(&page_nodes, pages);

    // Math block
    let html_math_method = "katex".to_string();
    let math_block = {
        let mut vars = HashMap::new();
        vars.insert("html_math_method".to_string(), html_math_method);
        vars.insert("base".to_string(), "html".to_string());
        crate::render::template::render_element_block("math", "html", &vars)
    };

    SiteContext {
        title: meta.and_then(|m| m.title.clone()),
        subtitle: meta.and_then(|m| m.subtitle.clone()),
        url: meta.and_then(|m| m.url.clone()),
        favicon: site.and_then(|s| s.favicon.clone()),
        logo: site.and_then(|s| s.logo.clone()),
        logo_dark: site.and_then(|s| s.logo_dark.clone()),
        pages: nav_tree,
        dark_mode: true,
        math_block,
    }
}

/// Build the navigation tree from expanded PageNodes, resolving titles from page metadata.
fn build_nav_tree(nodes: &[PageNode], pages: &[PageInfo]) -> Vec<NavNode> {
    let page_map: HashMap<String, &PageInfo> = pages
        .iter()
        .map(|p| (p.source.display().to_string(), p))
        .collect();

    nodes.iter().map(|node| match node {
        PageNode::Page(path) => {
            let info = page_map.get(path.as_str());
            let text = info
                .and_then(|p| p.meta.title.clone())
                .unwrap_or_else(|| path.clone());
            let href = info.map(|p| p.url.clone());
            NavNode {
                text: crate::render::markdown::render_inline(&text, "html"),
                href,
                active: false,
                children: vec![],
            }
        }
        PageNode::Section { title, pages: children } => {
            let child_nodes: Vec<PageNode> = children.iter()
                .map(|p| PageNode::Page(p.clone()))
                .collect();
            NavNode {
                text: title.clone(),
                href: None,
                active: false,
                children: build_nav_tree(&child_nodes, pages),
            }
        }
    }).collect()
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
    let body = result.map(|r| r.body.clone()).unwrap_or_default();

    let title = result.and_then(|r| r.title.clone()).or_else(|| page.meta.title.clone());
    let date = result.and_then(|r| r.date.clone()).or_else(|| page.meta.date.clone());
    let subtitle = result.and_then(|r| r.subtitle.clone()).or_else(|| page.meta.subtitle.clone());
    let abstract_text = result.and_then(|r| r.abstract_text.clone()).or_else(|| page.meta.r#abstract.clone());

    // Prev/next navigation excludes standalone pages
    let nav_pages: Vec<&PageInfo> = pages.iter().filter(|p| !p.standalone).collect();
    let idx = nav_pages.iter().position(|p| p.source == page.source);
    let prev = idx.and_then(|i| {
        if i > 0 {
            let p = nav_pages[i - 1];
            Some(NavLink {
                text: p.meta.title.clone().unwrap_or_else(|| p.source.display().to_string()),
                href: p.url.clone(),
            })
        } else { None }
    });
    let next = idx.and_then(|i| {
        if i + 1 < nav_pages.len() {
            let p = nav_pages[i + 1];
            Some(NavLink {
                text: p.meta.title.clone().unwrap_or_else(|| p.source.display().to_string()),
                href: p.url.clone(),
            })
        } else { None }
    });

    let breadcrumbs = build_breadcrumbs(page);

    PageContext {
        title, subtitle, date,
        r#abstract: abstract_text,
        body,
        url: page.url.clone(),
        source_url: format!("/_source/{}", page.source.display()),
        toc: result.and_then(|r| r.toc.clone()),
        listing: listing_items,
        breadcrumbs, prev, next,
    }
}

fn build_breadcrumbs(page: &PageInfo) -> Vec<Breadcrumb> {
    let mut crumbs = vec![Breadcrumb {
        text: "Home".to_string(),
        href: Some("/".to_string()),
    }];
    let components: Vec<_> = page.output.components().collect();
    if components.len() > 1 {
        for comp in components[..components.len() - 1].iter() {
            crumbs.push(Breadcrumb {
                text: comp.as_os_str().to_string_lossy().to_string(),
                href: None,
            });
        }
    }
    if let Some(title) = &page.meta.title {
        crumbs.push(Breadcrumb { text: title.clone(), href: None });
    }
    crumbs
}

