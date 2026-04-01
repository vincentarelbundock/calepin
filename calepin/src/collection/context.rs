//! Collection and document context for template rendering.
//!
//! The collection builder passes structured context to Jinja templates.
//! Top-level config fields provide the collection context. `[var]` is passed
//! through as arbitrary template variables.

use std::collections::HashMap;

use serde::Serialize;
use super::discover::DocumentInfo;
use super::render::CollectionRenderResult;
use super::contents::{DocumentNode, expand_contents_for_lang, expand_includes};
use crate::config::{ContentSection, Metadata, LanguageConfig, NavbarConfig};

/// Build a lookup map from source path string to DocumentInfo.
fn pages_to_map(pages: &[DocumentInfo]) -> HashMap<String, &DocumentInfo> {
    pages.iter().map(|p| (p.source.display().to_string(), p)).collect()
}

/// Collection-level context available to all templates as `{{ collection.* }}`.
#[derive(Debug, Serialize)]
pub struct CollectionContext {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub url: Option<String>,
    pub base_path: String,
    pub favicon: Option<String>,
    pub navbar: crate::config::NavbarConfig,
    pub pages: Vec<PageNode>,
    pub languages: Vec<LanguageConfig>,
    pub dark_mode: bool,
    pub math: String,
}

/// A node in the page tree, used by both website navigation and book orchestration.
#[derive(Debug, Clone, Serialize)]
pub struct PageNode {
    pub title: String,
    pub href: Option<String>,
    pub file: Option<String>,
    pub active: bool,
    pub children: Vec<PageNode>,
}

/// Per-document context available to templates as `{{ document.* }}`.
#[derive(Debug, Serialize)]
pub struct DocumentContext {
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

/// Resolve `include`/`exclude` on navbar items into `children` for dropdown menus.
///
/// For each navbar item that has `include` entries, expands them using the same
/// glob/directory machinery as `[[contents]]`, then converts the resulting
/// `DocumentNode` tree into nested `ContentSection` children with titles
/// resolved from page metadata.
fn resolve_navbar_includes(
    navbar: &NavbarConfig,
    pages: &[DocumentInfo],
    base_dir: &std::path::Path,
) -> NavbarConfig {
    let document_map = pages_to_map(pages);

    fn resolve_items(
        items: &[ContentSection],
        document_map: &std::collections::HashMap<String, &DocumentInfo>,
        base_dir: &std::path::Path,
    ) -> Vec<ContentSection> {
        items.iter().map(|item| {
            let includes = item.resolved_include();
            if includes.is_empty() {
                let mut resolved = item.clone();
                // Resolve .qmd hrefs to output URLs
                if let Some(ref href) = item.href {
                    if href.ends_with(".qmd") {
                        if let Some(info) = document_map.get(href.as_str()) {
                            resolved.href = Some(info.url.clone());
                        }
                    }
                }
                // Recurse into children
                if !resolved.children.is_empty() {
                    resolved.children = resolve_items(&resolved.children, document_map, base_dir);
                }
                return resolved;
            }
            let nodes = expand_includes(&includes, &item.exclude, base_dir);
            let children = nodes_to_children(&nodes, document_map);
            let mut resolved = item.clone();
            resolved.children = children;
            // Clear include so it doesn't get re-resolved
            resolved.include = Vec::new();
            resolved
        }).collect()
    }

    fn nodes_to_children(
        nodes: &[DocumentNode],
        document_map: &std::collections::HashMap<String, &DocumentInfo>,
    ) -> Vec<ContentSection> {
        nodes.iter().filter_map(|node| match node {
            DocumentNode::Document { path, title } => {
                let info = document_map.get(path.as_str());
                let text = title.clone()
                    .or_else(|| info.and_then(|p| p.meta.title.clone()))
                    .unwrap_or_else(|| path.clone());
                let href = info.map(|p| p.url.clone());
                Some(ContentSection {
                    text: Some(crate::render::convert::render_inline(&text, "html")),
                    href,
                    ..Default::default()
                })
            }
            DocumentNode::Section { title, index, documents } => {
                let href = index.as_ref().and_then(|idx| {
                    document_map.get(idx.as_str()).map(|p| p.url.clone())
                });
                Some(ContentSection {
                    text: Some(title.clone()),
                    href,
                    children: nodes_to_children(documents, document_map),
                    ..Default::default()
                })
            }
        }).collect()
    }

    NavbarConfig {
        left: resolve_items(&navbar.left, &document_map, base_dir),
        middle: resolve_items(&navbar.middle, &document_map, base_dir),
        right: resolve_items(&navbar.right, &document_map, base_dir),
    }
}

/// Build the collection-level context from project config.
/// The `pages` field contains the nav tree for the default language.
/// Use `build_nav_tree_for_lang` to get a language-specific nav tree.
pub fn build_collection_context(
    meta: &Metadata,
    pages: &[DocumentInfo],
    base_dir: &std::path::Path,
) -> CollectionContext {
    // Build nav tree for the default language (or all content if no languages configured)
    let default_lang = meta.default_language();
    let document_nodes = expand_contents_for_lang(&meta.contents, base_dir, default_lang);
    let nav_tree = build_page_tree(&document_nodes, pages);

    // Math block
    let math = {
        let vars = crate::render::template::TemplateVars::new();
        crate::render::template::render_element("math", "html", &vars)
    };

    let navbar = resolve_navbar_includes(
        &meta.navbar.clone().unwrap_or_default(),
        pages,
        base_dir,
    );

    let base_path = "/".to_string();

    CollectionContext {
        title: meta.title.clone(),
        subtitle: meta.subtitle.clone(),
        url: meta.url.clone(),
        base_path,
        favicon: meta.favicon.clone(),
        navbar,
        pages: nav_tree,
        languages: meta.languages.clone(),
        dark_mode: true,
        math,
    }
}

/// Build the nav tree for a specific language.
pub fn build_page_tree_for_lang(
    meta: &Metadata,
    pages: &[DocumentInfo],
    base_dir: &std::path::Path,
    lang: &str,
) -> Vec<PageNode> {
    let document_nodes = expand_contents_for_lang(&meta.contents, base_dir, Some(lang));
    build_page_tree(&document_nodes, pages)
}

/// Build the page tree from expanded DocumentNodes, resolving titles and paths from page metadata.
/// Used by both website navigation (via `href`) and book orchestration (via `file`).
pub fn build_page_tree(nodes: &[DocumentNode], pages: &[DocumentInfo]) -> Vec<PageNode> {
    let document_map = pages_to_map(pages);

    nodes.iter().map(|node| match node {
        DocumentNode::Document { path, title } => {
            let info = document_map.get(path.as_str());
            let resolved_title = title.clone()
                .or_else(|| info.and_then(|p| p.meta.title.clone()))
                .unwrap_or_else(|| path.clone());
            let href = info.map(|p| p.url.clone());
            let file = info.map(|p| p.output.display().to_string());
            PageNode {
                title: resolved_title,
                href,
                file,
                active: false,
                children: vec![],
            }
        }
        DocumentNode::Section { title, index, documents: children } => {
            let href = index.as_ref().and_then(|idx| {
                document_map.get(idx.as_str()).map(|p| p.url.clone())
            });
            PageNode {
                title: title.clone(),
                href,
                file: None,
                active: false,
                children: build_page_tree(children, pages),
            }
        }
    }).collect()
}

/// Mark the active page in the nav tree.
pub fn mark_active(nodes: &mut [PageNode], current_url: &str) -> bool {
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

/// Build the per-document context.
pub fn build_document_context(
    page: &DocumentInfo,
    result: Option<&CollectionRenderResult>,
    pages: &[DocumentInfo],
    listing_items: Option<Vec<ListingItem>>,
    languages: &[LanguageConfig],
    nav_paths: &[String],
) -> DocumentContext {
    let body = result.map(|r| r.body.clone()).unwrap_or_default();

    let title = result.and_then(|r| r.title.clone()).or_else(|| page.meta.title.clone());
    let date = result.and_then(|r| r.date.clone()).or_else(|| page.meta.date.clone())
        .map(|d| crate::utils::date::format_date_display(&d, None));
    let subtitle = result.and_then(|r| r.subtitle.clone()).or_else(|| page.meta.subtitle.clone());
    let abstract_text = result.and_then(|r| r.abstract_text.clone()).or_else(|| page.meta.r#abstract.clone());
    let pages_by_source = pages_to_map(pages);
    let nav_documents: Vec<&DocumentInfo> = nav_paths.iter()
        .filter_map(|path| pages_by_source.get(path.as_str()).copied())
        .filter(|p| p.lang == page.lang)
        .collect();
    let idx = nav_documents.iter().position(|p| p.source == page.source);
    let prev = idx.and_then(|i| {
        if i > 0 {
            let p = nav_documents[i - 1];
            Some(NavLink {
                text: p.meta.title.clone().unwrap_or_else(|| p.source.display().to_string()),
                href: p.url.clone(),
            })
        } else { None }
    });
    let next = idx.and_then(|i| {
        if i + 1 < nav_documents.len() {
            let p = nav_documents[i + 1];
            Some(NavLink {
                text: p.meta.title.clone().unwrap_or_else(|| p.source.display().to_string()),
                href: p.url.clone(),
            })
        } else { None }
    });

    // Resolve translations: look up each path in the page list
    let translations = resolve_translations(page, pages, languages);

    let breadcrumbs = build_breadcrumbs(page, pages);

    DocumentContext {
        title, subtitle, date,
        r#abstract: abstract_text,
        body,
        url: page.url.clone(),
        source_url: format!("/_calepin_source/{}", page.source.display()),
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

/// Resolve translations for a page.
///
/// Priority:
/// 1. Explicit `translations` in front matter (language code -> path)
/// 2. Auto-discovery via `[[languages]]` path prefixes: if page is at `fr/guide.qmd`
///    and another language has `path = "en"`, looks for `en/guide.qmd` in pages.
fn resolve_translations(
    page: &DocumentInfo,
    pages: &[DocumentInfo],
    languages: &[LanguageConfig],
) -> Vec<Translation> {
    let document_map = pages_to_map(pages);

    // Auto-discover from [[languages]] path prefixes
    let page_lang = page.lang.as_deref().unwrap_or("");
    let page_source = page.source.display().to_string();

    // Find this page's language config and strip its path prefix to get the relative filename
    let page_lang_config = languages.iter().find(|l| l.abbreviation == page_lang);
    let relative = if let Some(cfg) = page_lang_config {
        if let Some(ref lang_path) = cfg.path {
            if lang_path == "." {
                page_source.as_str()
            } else if page_source.starts_with(lang_path.as_str()) {
                page_source[lang_path.len()..].trim_start_matches('/')
            } else {
                return Vec::new();
            }
        } else {
            return Vec::new();
        }
    } else {
        return Vec::new();
    };

    // Look for the same relative path under each other language's path prefix
    let mut result = Vec::new();
    for lang in languages {
        if lang.abbreviation == page_lang { continue; }
        if let Some(ref lang_path) = lang.path {
            let candidate = if lang_path == "." {
                relative.to_string()
            } else {
                format!("{}/{}", lang_path, relative)
            };
            if let Some(info) = document_map.get(candidate.as_str()) {
                result.push(Translation {
                    flag: lang_to_flag(&lang.abbreviation),
                    lang: lang.abbreviation.clone(),
                    name: lang.language.clone(),
                    url: info.url.clone(),
                });
            }
        }
    }
    result
}

/// Format a YYYY-MM-DD date string for display using the default format.

fn build_breadcrumbs(page: &DocumentInfo, pages: &[DocumentInfo]) -> Vec<Breadcrumb> {
    let document_urls: std::collections::HashSet<String> = pages.iter().map(|p| p.url.clone()).collect();

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
            let href = if document_urls.contains(&index_url) {
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
        crumbs.push(Breadcrumb {
            text: crate::render::convert::render_inline(title, "html"),
            href: None,
        });
    }
    crumbs
}


