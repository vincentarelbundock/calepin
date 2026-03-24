//! Site and page context for template rendering.
//!
//! The site builder passes structured context to Jinja templates.
//! Top-level config fields provide the site context. `[var]` is passed
//! through as arbitrary template variables.

use std::collections::HashMap;

use serde::Serialize;
use super::discover::PageInfo;
use super::render::SiteRenderResult;
use crate::project::{ProjectConfig, PageNode, expand_contents_for_lang, Language};

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
    pub languages: Vec<Language>,
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
    pub listing_type: Option<String>,
    pub pagination: Option<Pagination>,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub prev: Option<NavLink>,
    pub next: Option<NavLink>,
    pub lang: Option<String>,
    pub flag: String,
    pub translations: Vec<Translation>,
}

/// A link to a translated version of the current page.
#[derive(Debug, Clone, Serialize)]
pub struct Translation {
    /// Language code (e.g., "fr").
    pub lang: String,
    /// Display name (e.g., "Fran\u{00e7}ais"). Empty if no [[languages]] config.
    pub name: String,
    /// Unicode flag emoji (e.g., "\u{1f1eb}\u{1f1f7}" for fr).
    pub flag: String,
    /// URL of the translated page.
    pub url: String,
}

/// Convert a language code to a Unicode regional indicator flag emoji.
/// Maps language codes to country codes where they differ (e.g., "en" -> "gb"),
/// then converts each letter to the corresponding regional indicator symbol.
fn lang_to_flag(lang: &str) -> String {
    let country = match lang {
        "en" => "gb",
        "ja" => "jp",
        "ko" => "kr",
        "zh" => "cn",
        "ar" => "sa",
        "hi" => "in",
        "uk" => "ua",
        "cs" => "cz",
        "da" => "dk",
        "el" => "gr",
        "he" => "il",
        "sv" => "se",
        "nb" | "nn" => "no",
        "ca" => "es",
        "eu" => "es",
        "gl" => "es",
        "ms" => "my",
        "fa" => "ir",
        "vi" => "vn",
        "sq" => "al",
        "hy" => "am",
        "ka" => "ge",
        "et" => "ee",
        "sl" => "si",
        "sr" => "rs",
        "bs" => "ba",
        "mk" => "mk",
        other => other,
    };
    // Regional indicator symbols: U+1F1E6 ('A') through U+1F1FF ('Z')
    let bytes: Vec<char> = country.to_uppercase().chars().map(|c| {
        char::from_u32(0x1F1E6 + (c as u32 - 'A' as u32)).unwrap_or(c)
    }).collect();
    bytes.into_iter().collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct Pagination {
    pub current: usize,
    pub total: usize,
    pub prev_url: Option<String>,
    pub next_url: Option<String>,
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
/// The `pages` field contains the nav tree for the default language.
/// Use `build_nav_tree_for_lang` to get a language-specific nav tree.
pub fn build_site_context(
    config: &ProjectConfig,
    pages: &[PageInfo],
    base_dir: &std::path::Path,
) -> SiteContext {
    // Build nav tree for the default language (or all content if no languages configured)
    let default_lang = config.default_language();
    let page_nodes = expand_contents_for_lang(&config.contents, base_dir, default_lang);
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
        title: config.title.clone(),
        subtitle: config.subtitle.clone(),
        url: config.url.clone(),
        favicon: config.favicon.clone(),
        logo: config.logo.clone(),
        logo_dark: config.logo_dark.clone(),
        pages: nav_tree,
        languages: config.languages.clone(),
        dark_mode: true,
        math_block,
    }
}

/// Build the nav tree for a specific language.
pub fn build_nav_tree_for_lang(
    config: &ProjectConfig,
    pages: &[PageInfo],
    base_dir: &std::path::Path,
    lang: &str,
) -> Vec<NavNode> {
    let page_nodes = expand_contents_for_lang(&config.contents, base_dir, Some(lang));
    build_nav_tree(&page_nodes, pages)
}

/// Build the navigation tree from expanded PageNodes, resolving titles from page metadata.
fn build_nav_tree(nodes: &[PageNode], pages: &[PageInfo]) -> Vec<NavNode> {
    let page_map: HashMap<String, &PageInfo> = pages
        .iter()
        .map(|p| (p.source.display().to_string(), p))
        .collect();

    nodes.iter().map(|node| match node {
        PageNode::Page { path, title } => {
            let info = page_map.get(path.as_str());
            let text = title.clone()
                .or_else(|| info.and_then(|p| p.meta.title.clone()))
                .unwrap_or_else(|| path.clone());
            let href = info.map(|p| p.url.clone());
            NavNode {
                text: crate::render::markdown::render_inline(&text, "html"),
                href,
                active: false,
                children: vec![],
            }
        }
        PageNode::Section { title, index, pages: children } => {
            // Section header can be a link if it has an index page
            let href = index.as_ref().and_then(|idx| {
                page_map.get(idx.as_str()).map(|p| p.url.clone())
            });
            NavNode {
                text: title.clone(),
                href,
                active: false,
                children: build_nav_tree(children, pages),
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
    languages: &[Language],
) -> PageContext {
    let body = result.map(|r| r.body.clone()).unwrap_or_default();

    let title = result.and_then(|r| r.title.clone()).or_else(|| page.meta.title.clone());
    let date = result.and_then(|r| r.date.clone()).or_else(|| page.meta.date.clone())
        .map(|d| format_date(&d));
    let subtitle = result.and_then(|r| r.subtitle.clone()).or_else(|| page.meta.subtitle.clone());
    let abstract_text = result.and_then(|r| r.abstract_text.clone()).or_else(|| page.meta.r#abstract.clone());

    // Prev/next navigation excludes standalone pages and pages in other languages
    let nav_pages: Vec<&PageInfo> = pages.iter().filter(|p| {
        !p.standalone && p.lang == page.lang
    }).collect();
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

    // Resolve translations: look up each path in the page list
    let translations = resolve_translations(page, pages, languages);

    let breadcrumbs = build_breadcrumbs(page, pages);

    PageContext {
        title, subtitle, date,
        r#abstract: abstract_text,
        body,
        url: page.url.clone(),
        source_url: format!("/_source/{}", page.source.display()),
        toc: result.and_then(|r| r.toc.clone()),
        listing: listing_items,
        listing_type: page.meta.listing.as_ref().map(|l| l.r#type.clone()),
        pagination: None,
        breadcrumbs, prev, next,
        flag: page.lang.as_deref().map(lang_to_flag).unwrap_or_default(),
        lang: page.lang.clone(),
        translations,
    }
}

/// Resolve translation paths from frontmatter to URLs.
fn resolve_translations(
    page: &PageInfo,
    pages: &[PageInfo],
    languages: &[Language],
) -> Vec<Translation> {
    let translations = match &page.meta.translations {
        Some(t) if !t.is_empty() => t,
        _ => return Vec::new(),
    };

    let page_map: HashMap<String, &PageInfo> = pages.iter()
        .map(|p| (p.source.display().to_string(), p))
        .collect();

    let lang_names: HashMap<&str, &str> = languages.iter()
        .map(|l| (l.code.as_str(), l.name.as_str()))
        .collect();

    let mut result = Vec::new();
    for (lang_code, path) in translations {
        if let Some(info) = page_map.get(path.as_str()) {
            result.push(Translation {
                flag: lang_to_flag(lang_code),
                lang: lang_code.clone(),
                name: lang_names.get(lang_code.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| lang_code.clone()),
                url: info.url.clone(),
            });
        } else {
            eprintln!(
                "Warning: translation '{}' -> '{}' not found in site pages (referenced from {})",
                lang_code, path, page.source.display()
            );
        }
    }
    result
}

/// Default date display format (strftime-style).
const DEFAULT_DATE_FORMAT: &str = "%B %e, %Y";

/// Format a YYYY-MM-DD date string for display.
pub fn format_date(date: &str) -> String {
    crate::types::format_date_str(date, DEFAULT_DATE_FORMAT)
}

fn build_breadcrumbs(page: &PageInfo, pages: &[PageInfo]) -> Vec<Breadcrumb> {
    // Collect all page URLs for checking if a path leads to a real page
    let page_urls: Vec<String> = pages.iter().map(|p| p.url.clone()).collect();

    let mut crumbs = vec![Breadcrumb {
        text: "Home".to_string(),
        href: Some("/".to_string()),
    }];
    let components: Vec<_> = page.output.components().collect();
    // Build intermediate path segments (skip the filename)
    if components.len() > 1 {
        let mut href_path = String::from("/");
        for comp in components[..components.len() - 1].iter() {
            let name = comp.as_os_str().to_string_lossy();
            href_path.push_str(&name);
            href_path.push('/');
            // Prettify: replace - and _ with spaces, title case
            let pretty = name
                .replace('-', " ")
                .replace('_', " ");
            let pretty: String = pretty.split_whitespace()
                .map(|w| {
                    let mut c = w.chars();
                    match c.next() {
                        None => String::new(),
                        Some(f) => f.to_uppercase().to_string() + c.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            // Only link if there's a page at this path (index.html)
            let index_url = format!("{}index.html", href_path);
            let href = if page_urls.iter().any(|u| *u == index_url) {
                Some(href_path.clone())
            } else {
                None
            };
            crumbs.push(Breadcrumb {
                text: pretty,
                href,
            });
        }
    }
    // Final crumb: page title (not clickable, it's the current page)
    if let Some(title) = &page.meta.title {
        crumbs.push(Breadcrumb { text: title.clone(), href: None });
    }
    crumbs
}

