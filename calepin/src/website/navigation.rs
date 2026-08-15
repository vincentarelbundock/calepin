use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};

use crate::html::SiteNavEntry;
use crate::typst::paths::is_generated_entry_file;
use crate::utils::html::escape as html_escape;
use crate::utils::static_files::{collect_files_by, path_has_common_skip_dir};
use crate::utils::url::output_href_with_extension;

use super::config::{FooterConfig, MenuItemConfig, PagesConfig, SidebarConfig, StaticConfig};
use super::icons::{accessible_nav_label, nav_label_html, IconCache};
use super::language::LanguageInfo;
use super::metadata::{PageMeta, PageMetaMap};
use super::paths::{
    join_normalized_under_root, normalize_path, rel_posix, relative_or_self, slash_path,
    wildcard_match,
};
use super::url::{is_absolute_or_special_url, is_safe_output_route, page_relative_url};
use super::util::clean_optional_string;
use super::{PageInfo, PageInfoMap, FALLBACK_PAGE, INDEX_PAGE, PAGES_INDEX_FILE};

#[derive(Debug, Clone)]
pub(super) struct NavSectionModel {
    pub(super) language: Option<String>,
    pub(super) title: Option<String>,
    pub(super) items: Vec<NavItemModel>,
}

#[derive(Debug, Clone)]
pub(super) struct NavItemModel {
    pub(super) language: Option<String>,
    pub(super) href: String,
    pub(super) label: String,
    pub(super) label_html: String,
}

#[derive(Debug, Clone, Default)]
pub(super) struct MenusModel {
    pub(super) items: BTreeMap<String, Vec<NavItemModel>>,
}

impl MenusModel {
    pub(super) fn entries_for_current_page(
        &self,
        current_href: &str,
        current_language: Option<&str>,
    ) -> BTreeMap<String, Vec<SiteNavEntry>> {
        self.items
            .iter()
            .map(|(name, items)| {
                let entries = items
                    .iter()
                    .filter(|item| {
                        item.language
                            .as_deref()
                            .is_none_or(|language| Some(language) == current_language)
                    })
                    .map(|item| SiteNavEntry {
                        href: html_escape(&menu_item_href(current_href, &item.href)),
                        label: html_escape(&item.label),
                        label_html: item.label_html.clone(),
                        active: item.href == current_href,
                    })
                    .collect();
                (name.clone(), entries)
            })
            .collect()
    }
}

fn menu_item_href(current_href: &str, href: &str) -> String {
    if href.is_empty() {
        String::new()
    } else {
        page_relative_url(current_href, href)
    }
}

/// Plan for one navigation entry, resolved before metadata is available.
#[derive(Debug, Clone)]
pub(super) struct NavItemPlan {
    pub(super) path: Option<PathBuf>,
    pub(super) url: Option<String>,
    pub(super) configured_label: Option<String>,
    pub(super) configured_aria_label: Option<String>,
    pub(super) weight: Option<i32>,
}

#[derive(Debug, Clone)]
pub(super) struct NavSectionPlan {
    pub(super) language: Option<String>,
    pub(super) title: Option<String>,
    pub(super) items: Vec<NavItemPlan>,
}

struct NavItemInput<'a> {
    target: Option<&'a str>,
    glob: Option<&'a str>,
    label: Option<&'a str>,
    aria_label: Option<&'a str>,
    weight: Option<i32>,
}

#[derive(Debug, Clone, Copy)]
enum NavSurface<'a> {
    Sidebar,
    Menu(&'a str),
}

impl NavSurface<'_> {
    fn context(self) -> String {
        match self {
            NavSurface::Sidebar => "sidebar".to_string(),
            NavSurface::Menu(name) => format!("menu `{name}`"),
        }
    }

    fn allows_urls(self) -> bool {
        matches!(self, NavSurface::Sidebar | NavSurface::Menu(_))
    }

    fn skip_duplicate_items(self) -> bool {
        matches!(self, NavSurface::Sidebar)
    }

    fn allows_linkless_items(self) -> bool {
        matches!(self, NavSurface::Sidebar | NavSurface::Menu("footer"))
    }

    fn uses_configured_labels(self) -> bool {
        matches!(self, NavSurface::Menu(_))
    }
}

struct NavItemResolution<'a> {
    surface: NavSurface<'a>,
    src_dir: &'a Path,
    pages: Option<&'a PagesConfig>,
    all_typ_files: &'a [PathBuf],
    used: &'a mut BTreeSet<PathBuf>,
    build_files: &'a mut Vec<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct MenusPlan {
    pub(super) items: BTreeMap<String, Vec<MenuItemPlan>>,
}

pub(super) type MenuItemPlan = NavItemPlan;

pub(super) fn discover_site_pages(
    src_dir: &Path,
    sidebar: Option<&SidebarConfig>,
    pages: Option<&PagesConfig>,
    languages: &Option<Vec<LanguageInfo>>,
    asset_dir: &Path,
) -> Result<(Vec<NavSectionPlan>, Vec<PathBuf>)> {
    let Some(languages) = languages else {
        return discover_pages(src_dir, sidebar, pages, None, asset_dir);
    };
    let mut sections = Vec::new();
    let mut files = Vec::new();
    for language in languages {
        let (mut language_sections, mut language_files) = discover_pages(
            &language.content_dir,
            sidebar,
            pages,
            Some(language.code.clone()),
            asset_dir,
        )?;
        language_files.retain(|path| !is_nested_language_page(path, language, languages));
        for section in &mut language_sections {
            section.items.retain(|item| {
                item.path
                    .as_ref()
                    .is_none_or(|path| !is_nested_language_page(path, language, languages))
            });
        }
        sections.append(&mut language_sections);
        files.append(&mut language_files);
    }
    Ok((sections, files))
}

pub(super) fn discover_site_menus(
    src_dir: &Path,
    menus: &BTreeMap<String, Vec<MenuItemConfig>>,
    footer: Option<&FooterConfig>,
    pages: Option<&PagesConfig>,
    languages: &Option<Vec<LanguageInfo>>,
    asset_dir: &Path,
) -> Result<(MenusPlan, Vec<PathBuf>)> {
    let menus = effective_site_menus(menus, footer)?;
    if menus.is_empty() {
        return Ok((MenusPlan::default(), Vec::new()));
    }
    let Some(languages) = languages else {
        return discover_menus(src_dir, &menus, pages, asset_dir);
    };
    let mut plan = MenusPlan::default();
    let mut files = Vec::new();
    for language in languages {
        let (mut language_plan, mut language_files) =
            discover_menus(&language.content_dir, &menus, pages, asset_dir)?;
        language_files.retain(|path| !is_nested_language_page(path, language, languages));
        retain_menu_language_items(&mut language_plan, language, languages);
        if !language.is_default {
            retain_language_specific_menu_items(&mut language_plan);
        }
        for (name, mut items) in language_plan.items {
            plan.items.entry(name).or_default().append(&mut items);
        }
        files.append(&mut language_files);
    }
    Ok((plan, files))
}

fn effective_site_menus(
    menus: &BTreeMap<String, Vec<MenuItemConfig>>,
    footer: Option<&FooterConfig>,
) -> Result<BTreeMap<String, Vec<MenuItemConfig>>> {
    if menus.contains_key("footer") {
        bail!("use `[[footer.item]]` instead of `[[menus.footer]]`");
    }

    let mut effective = menus.clone();
    if let Some(footer) = footer {
        if !footer.item.is_empty() {
            effective.insert("footer".to_string(), footer.item.clone());
        }
    }
    Ok(effective)
}

fn retain_menu_language_items(
    plan: &mut MenusPlan,
    current: &LanguageInfo,
    languages: &[LanguageInfo],
) {
    for items in plan.items.values_mut() {
        items.retain(|item| {
            item.path
                .as_ref()
                .is_none_or(|path| !is_nested_language_page(path, current, languages))
        });
    }
}

fn retain_language_specific_menu_items(plan: &mut MenusPlan) {
    for items in plan.items.values_mut() {
        items.retain(|item| item.path.is_some());
    }
}

fn is_nested_language_page(
    path: &Path,
    current: &LanguageInfo,
    languages: &[LanguageInfo],
) -> bool {
    languages.iter().any(|language| {
        language.code != current.code
            && language.content_dir.starts_with(&current.content_dir)
            && path.starts_with(&language.content_dir)
    })
}

pub(super) fn discover_pages(
    src_dir: &Path,
    sidebar: Option<&SidebarConfig>,
    pages: Option<&PagesConfig>,
    language: Option<String>,
    asset_dir: &Path,
) -> Result<(Vec<NavSectionPlan>, Vec<PathBuf>)> {
    let Some(sidebar) = sidebar else {
        let mut files = iter_typ_files(src_dir, false, &[PathBuf::from(FALLBACK_PAGE)], asset_dir)?;
        files.retain(|path| !is_page_excluded(src_dir, path, pages));
        let items = files
            .iter()
            .map(|path| NavItemPlan {
                path: Some(path.clone()),
                url: None,
                configured_label: None,
                configured_aria_label: None,
                weight: None,
            })
            .collect();
        return Ok((
            vec![NavSectionPlan {
                language,
                title: None,
                items,
            }],
            files,
        ));
    };

    let all_typ_files = iter_typ_files(
        src_dir,
        sidebar.show_hidden,
        &[PathBuf::from(FALLBACK_PAGE)],
        asset_dir,
    )?;
    let all_typ_files = all_typ_files
        .into_iter()
        .filter(|path| !is_page_excluded(src_dir, path, pages))
        .collect::<Vec<_>>();
    let mut used = BTreeSet::new();
    let mut sections = Vec::new();
    let mut build_files = Vec::new();

    for section_config in &sidebar.section {
        let inputs = section_config
            .item
            .iter()
            .map(|item| NavItemInput {
                target: item.target.as_deref(),
                glob: item.glob.as_deref(),
                label: item.label.as_deref(),
                aria_label: None,
                weight: None,
            })
            .collect::<Vec<_>>();
        let mut resolution = NavItemResolution {
            surface: NavSurface::Sidebar,
            src_dir,
            pages,
            all_typ_files: &all_typ_files,
            used: &mut used,
            build_files: &mut build_files,
        };
        let items = resolve_nav_item_plans(&mut resolution, &inputs)?;
        sections.push(NavSectionPlan {
            language: language.clone(),
            title: section_config.title.clone(),
            items,
        });
    }

    Ok((sections, build_files))
}

pub(super) fn discover_menus(
    src_dir: &Path,
    menus: &BTreeMap<String, Vec<MenuItemConfig>>,
    pages: Option<&PagesConfig>,
    asset_dir: &Path,
) -> Result<(MenusPlan, Vec<PathBuf>)> {
    for name in menus.keys() {
        validate_menu_name(name)?;
    }
    let all_typ_files = iter_typ_files(src_dir, false, &[PathBuf::from(FALLBACK_PAGE)], asset_dir)?;
    let all_typ_files = all_typ_files
        .into_iter()
        .filter(|path| !is_page_excluded(src_dir, path, pages))
        .collect::<Vec<_>>();
    let mut files = Vec::new();
    let mut used = BTreeSet::new();
    let mut plan = MenusPlan::default();

    for (name, items) in menus {
        let inputs = items
            .iter()
            .map(|item| NavItemInput {
                target: item.target.as_deref(),
                glob: item.glob.as_deref(),
                label: item.label.as_deref(),
                aria_label: item.aria_label.as_deref(),
                weight: item.weight,
            })
            .collect::<Vec<_>>();
        let mut resolution = NavItemResolution {
            surface: NavSurface::Menu(name),
            src_dir,
            pages,
            all_typ_files: &all_typ_files,
            used: &mut used,
            build_files: &mut files,
        };
        let mut resolved = resolve_nav_item_plans(&mut resolution, &inputs)?;
        sort_menu_items(&mut resolved);
        plan.items.insert(name.clone(), resolved);
    }

    files = menu_build_files(&plan);
    Ok((plan, files))
}

fn validate_menu_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
        })
    {
        bail!(
            "invalid menu name `{name}`; use lowercase letters, digits, hyphens, and underscores"
        );
    }
    Ok(())
}

fn sort_menu_items(items: &mut [MenuItemPlan]) {
    if items.iter().all(|item| item.weight.is_none()) {
        return;
    }
    items.sort_by_key(|item| (item.weight.is_none(), item.weight.unwrap_or_default()));
}

fn menu_build_files(plan: &MenusPlan) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut files = Vec::new();
    for items in plan.items.values() {
        for item in items {
            if let Some(path) = &item.path {
                if seen.insert(path.clone()) {
                    files.push(path.clone());
                }
            }
        }
    }
    files
}

fn resolve_nav_item_plans(
    resolution: &mut NavItemResolution<'_>,
    inputs: &[NavItemInput<'_>],
) -> Result<Vec<NavItemPlan>> {
    let mut items = Vec::new();
    for input in inputs {
        let configured_label = clean_optional_string(input.label);
        if let Some(target) = input
            .target
            .map(str::trim)
            .filter(|target| !target.is_empty())
        {
            if input.glob.is_some_and(|glob| !glob.trim().is_empty()) {
                bail!(
                    "{} target items cannot also set glob",
                    resolution.surface.context()
                );
            }
            match resolve_nav_target(resolution.surface, resolution.src_dir, target)? {
                Some(NavTarget::Url(url)) => {
                    if !resolution.surface.allows_urls() {
                        bail!(
                            "sidebar target must point to a .typ source page, got literal URL: {url}"
                        );
                    }
                    if configured_label.is_none()
                        && matches!(resolution.surface, NavSurface::Sidebar)
                    {
                        bail!("sidebar URL target items must set label");
                    }
                    items.push(NavItemPlan {
                        path: None,
                        url: Some(url),
                        configured_label,
                        configured_aria_label: input.aria_label.map(str::to_string),
                        weight: input.weight,
                    });
                }
                Some(NavTarget::Page(path)) => {
                    if configured_label.is_some()
                        && matches!(resolution.surface, NavSurface::Sidebar)
                    {
                        bail!("sidebar target items cannot also set label");
                    }
                    if is_page_excluded(resolution.src_dir, &path, resolution.pages) {
                        continue;
                    }
                    let first_use = resolution.used.insert(path.clone());
                    if first_use {
                        resolution.build_files.push(path.clone());
                    }
                    if first_use || !resolution.surface.skip_duplicate_items() {
                        items.push(NavItemPlan {
                            path: Some(path),
                            url: None,
                            configured_label,
                            configured_aria_label: input.aria_label.map(str::to_string),
                            weight: input.weight,
                        });
                    }
                }
                None => {}
            }
            continue;
        }

        if configured_label.is_some()
            && matches!(resolution.surface, NavSurface::Sidebar)
            && input
                .glob
                .map(str::trim)
                .is_some_and(|glob| !glob.is_empty())
        {
            bail!("sidebar glob items cannot also set label");
        }

        if resolution.surface.allows_linkless_items() {
            if let Some(label) = configured_label.clone() {
                items.push(NavItemPlan {
                    path: None,
                    url: None,
                    configured_label: Some(label),
                    configured_aria_label: input.aria_label.map(str::to_string),
                    weight: input.weight,
                });
                continue;
            }
        }

        if input.glob.map(str::trim).is_none_or(|glob| glob.is_empty()) {
            bail!(
                "{} item must set path, glob, or url",
                resolution.surface.context()
            );
        }

        for path in resolve_file_list(resolution.src_dir, input.glob, resolution.all_typ_files)?
            .into_iter()
            .filter(|path| !is_page_excluded(resolution.src_dir, path, resolution.pages))
        {
            let first_use = resolution.used.insert(path.clone());
            if first_use {
                resolution.build_files.push(path.clone());
            }
            if first_use || !resolution.surface.skip_duplicate_items() {
                items.push(NavItemPlan {
                    path: Some(path),
                    url: None,
                    configured_label: configured_label.clone(),
                    configured_aria_label: input.aria_label.map(str::to_string),
                    weight: input.weight,
                });
            }
        }
    }
    Ok(items)
}

pub(super) fn fallback_pages(
    src_dir: &Path,
    languages: &Option<Vec<LanguageInfo>>,
) -> Vec<PathBuf> {
    languages
        .as_ref()
        .map(|languages| {
            languages
                .iter()
                .map(|language| language.content_dir.join(FALLBACK_PAGE))
                .collect()
        })
        .unwrap_or_else(|| vec![src_dir.join(FALLBACK_PAGE)])
}

pub(super) fn implicit_build_pages(
    src_dir: &Path,
    languages: &Option<Vec<LanguageInfo>>,
) -> Vec<PathBuf> {
    let roots = languages
        .as_ref()
        .map(|languages| {
            languages
                .iter()
                .map(|language| language.content_dir.as_path())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![src_dir]);
    roots
        .into_iter()
        .flat_map(|root| [root.join(INDEX_PAGE), root.join(FALLBACK_PAGE)])
        .filter(|path| path.is_file())
        .collect()
}

pub(super) fn discover_site_build_pages(
    src_dir: &Path,
    pages: Option<&PagesConfig>,
    languages: &Option<Vec<LanguageInfo>>,
    asset_dir: &Path,
) -> Result<Vec<PathBuf>> {
    let Some(languages) = languages else {
        return discover_build_pages(src_dir, pages, asset_dir);
    };
    let mut files = Vec::new();
    for language in languages {
        let mut language_files = discover_build_pages(&language.content_dir, pages, asset_dir)?;
        language_files.retain(|path| !is_nested_language_page(path, language, languages));
        files.append(&mut language_files);
    }
    Ok(files)
}

pub(super) fn discover_build_pages(
    src_dir: &Path,
    pages: Option<&PagesConfig>,
    asset_dir: &Path,
) -> Result<Vec<PathBuf>> {
    let Some(pages) = pages else {
        return Ok(Vec::new());
    };
    let all_typ_files = iter_typ_files(src_dir, true, &[PathBuf::from(FALLBACK_PAGE)], asset_dir)?;
    let mut files = BTreeSet::new();
    for pattern in page_patterns(&pages.include) {
        let matches = if has_glob_chars(&pattern) {
            all_typ_files
                .iter()
                .filter(|path| wildcard_match(&pattern, &rel_posix(src_dir, path)))
                .cloned()
                .collect::<Vec<_>>()
        } else {
            resolve_existing_source_page("website `pages.include`", src_dir, &pattern, "path")?
                .into_iter()
                .collect()
        };
        for path in matches {
            if !is_page_excluded(src_dir, &path, Some(pages)) {
                files.insert(path);
            }
        }
    }
    Ok(files.into_iter().collect())
}

pub(super) fn discover_static_files(
    src_dir: &Path,
    static_files: Option<&StaticConfig>,
    asset_dir: &Path,
) -> Result<Vec<PathBuf>> {
    let Some(static_files) = static_files else {
        return Ok(Vec::new());
    };
    let include = static_patterns(&static_files.include, "static.include")?;
    let exclude = static_patterns(&static_files.exclude, "static.exclude")?;
    let all_files = iter_static_files(src_dir, asset_dir)?;
    let mut files = BTreeSet::new();
    for pattern in include {
        let matches = if has_glob_chars(&pattern) {
            all_files
                .iter()
                .filter(|path| wildcard_match(&pattern, &rel_posix(src_dir, path)))
                .cloned()
                .collect::<Vec<_>>()
        } else {
            let candidate = normalize_path(&src_dir.join(Path::new(&pattern)));
            if candidate.is_file() {
                vec![candidate]
            } else if candidate.is_dir() {
                all_files
                    .iter()
                    .filter(|path| path.starts_with(&candidate))
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                cwarn!("static.include path does not exist: {}", pattern);
                Vec::new()
            }
        };
        for path in matches {
            let rel = rel_posix(src_dir, &path);
            if !exclude.iter().any(|pattern| wildcard_match(pattern, &rel)) {
                files.insert(path);
            }
        }
    }
    Ok(files.into_iter().collect())
}

fn iter_static_files(src_dir: &Path, asset_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect_static_files(src_dir, src_dir, asset_dir, &mut out)?;
    out.sort_by_key(|path| rel_posix(src_dir, path));
    Ok(out)
}

fn collect_static_files(
    root: &Path,
    dir: &Path,
    asset_dir: &Path,
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    collect_files_by(
        root,
        dir,
        out,
        |rel, _| !path_has_common_skip_dir(rel) && !rel.starts_with(asset_dir),
        |rel, _| {
            !path_has_common_skip_dir(rel)
                && !rel.starts_with(asset_dir)
                && !is_generated_entry_file(rel)
        },
    )
}

fn static_patterns(patterns: &[String], key: &str) -> Result<Vec<String>> {
    patterns
        .iter()
        .filter_map(|pattern| clean_optional_string(Some(pattern.as_str())))
        .map(|pattern| {
            let pattern = slash_path(Path::new(&pattern));
            if !is_safe_output_route(&pattern) {
                bail!("website `{key}` path must stay inside the source directory: {pattern}");
            }
            Ok(pattern)
        })
        .collect()
}

fn is_page_excluded(src_dir: &Path, path: &Path, pages: Option<&PagesConfig>) -> bool {
    let Some(pages) = pages else {
        return false;
    };
    let rel = rel_posix(src_dir, path);
    page_patterns(&pages.exclude)
        .into_iter()
        .any(|pattern| wildcard_match(&pattern, &rel))
}

fn page_patterns(patterns: &[String]) -> Vec<String> {
    patterns
        .iter()
        .filter_map(|pattern| clean_optional_string(Some(pattern.as_str())))
        .map(|pattern| slash_path(Path::new(&pattern)))
        .collect()
}

fn has_glob_chars(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?')
}

pub(super) fn build_page_info(
    src_dir: &Path,
    typ_files: &[PathBuf],
    page_meta: &PageMetaMap,
    pdf_files: &BTreeSet<PathBuf>,
    languages: &Option<Vec<LanguageInfo>>,
) -> Result<PageInfoMap> {
    let mut out = PageInfoMap::new();
    for path in typ_files {
        let language = page_language(path, languages)?;
        let meta = page_meta.get(path);
        for (key, value, route) in [
            ("slug", meta.and_then(|meta| meta.slug.as_deref()), false),
            ("url", meta.and_then(|meta| meta.url.as_deref()), true),
        ] {
            if let Some(value) = value {
                // `url` values are interpreted relative to the output root, so a
                // leading `/` is harmless there; a slug is joined onto the page's
                // directory and must be relative.
                let checked = if route {
                    value.trim_start_matches('/')
                } else {
                    value
                };
                if !is_safe_output_route(checked) {
                    bail!(
                        "page {key} must stay inside the output directory: `{value}` ({})",
                        path.display()
                    );
                }
            }
        }
        let rel = page_relative_source_path(src_dir, path, language);
        let translation_key = meta
            .and_then(|meta| meta.translation_key.clone())
            .unwrap_or_else(|| slash_path(&rel.with_extension("")));
        let href = page_output_href(&rel, language, meta, "html");
        let pdf_href = pdf_files
            .contains(path)
            .then(|| page_output_href(&rel, language, meta, "pdf"));
        let redirects = meta
            .map(|meta| page_redirect_routes(path, &meta.redirect_from))
            .transpose()?
            .unwrap_or_default();
        out.insert(
            path.clone(),
            PageInfo {
                language: language.map(|language| language.code.clone()),
                translation_key,
                href,
                pdf_href,
                image: meta
                    .and_then(|meta| meta.image.as_deref())
                    .map(social_image_target),
                description: meta.and_then(page_excerpt),
                redirects,
            },
        );
    }
    reject_conflicting_redirects(&out)?;
    Ok(out)
}

/// Site-relative form of a page's social image: absolute URLs pass through,
/// everything else is read as a path from the output root.
fn social_image_target(value: &str) -> String {
    if is_absolute_or_special_url(value) && !value.starts_with('/') {
        return value.to_string();
    }
    value
        .trim_start_matches('/')
        .trim_start_matches("./")
        .to_string()
}

/// Normalizes each authored `redirect-from` route to the output path of the
/// stub that will be written for it. Directory-style routes (`old/`, `old`)
/// become `old/index.html` and `old.html` respectively.
fn page_redirect_routes(path: &Path, routes: &[String]) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for route in routes {
        let trimmed = route
            .trim()
            .trim_start_matches('/')
            .trim_start_matches("./");
        if trimmed.is_empty() {
            bail!(
                "page redirect-from route must not be empty ({})",
                path.display()
            );
        }
        if !is_safe_output_route(trimmed) {
            bail!(
                "page redirect-from route must stay inside the output directory: `{route}` ({})",
                path.display()
            );
        }
        let normalized = if trimmed.ends_with('/') {
            format!("{}index.html", trimmed)
        } else if trimmed.ends_with(".html") {
            trimmed.to_string()
        } else {
            format!("{trimmed}.html")
        };
        if !out.contains(&normalized) {
            out.push(normalized);
        }
    }
    Ok(out)
}

/// A redirect stub may not overwrite a real page, and two pages may not claim
/// the same old route: either way one of the two outputs would silently win.
fn reject_conflicting_redirects(page_info: &PageInfoMap) -> Result<()> {
    let hrefs = page_info
        .values()
        .map(|info| info.href.as_str())
        .collect::<BTreeSet<_>>();
    let mut claimed: BTreeMap<&str, &Path> = BTreeMap::new();
    for (path, info) in page_info {
        for redirect in &info.redirects {
            if hrefs.contains(redirect.as_str()) {
                bail!(
                    "page redirect-from route `{redirect}` collides with a rendered page ({})",
                    path.display()
                );
            }
            if let Some(other) = claimed.insert(redirect.as_str(), path) {
                bail!(
                    "pages {} and {} both redirect from `{redirect}`",
                    other.display(),
                    path.display()
                );
            }
        }
    }
    Ok(())
}

fn page_language<'a>(
    path: &Path,
    languages: &'a Option<Vec<LanguageInfo>>,
) -> Result<Option<&'a LanguageInfo>> {
    let Some(languages) = languages else {
        return Ok(None);
    };
    languages
        .iter()
        .filter(|language| path.starts_with(&language.content_dir))
        .max_by_key(|language| language.content_dir.components().count())
        .map(Some)
        .ok_or_else(|| {
            anyhow!(
                "page is outside configured language content directories: {}",
                path.display()
            )
        })
}

fn page_relative_source_path(
    src_dir: &Path,
    path: &Path,
    language: Option<&LanguageInfo>,
) -> PathBuf {
    let root = language
        .map(|language| language.content_dir.as_path())
        .unwrap_or(src_dir);
    relative_or_self(root, path).to_path_buf()
}

fn page_output_href(
    rel_source: &Path,
    language: Option<&LanguageInfo>,
    meta: Option<&PageMeta>,
    extension: &str,
) -> String {
    if let Some(url) = meta.and_then(|meta| meta.url.as_deref()) {
        return output_href_with_extension(url, extension);
    }
    let rel = if let Some(slug) = meta.and_then(|meta| meta.slug.as_deref()) {
        let mut rel = rel_source
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(slug);
        rel.set_extension(extension);
        rel
    } else {
        rel_source.with_extension(extension)
    };
    let rel = slash_path(&rel);
    match language
        .map(|language| language.url_prefix.as_str())
        .filter(|prefix| !prefix.is_empty())
    {
        Some(prefix) if rel.is_empty() => prefix.to_string(),
        Some(prefix) => format!("{prefix}/{rel}"),
        None => rel,
    }
}

pub(super) fn nav_from_plans(
    sections: &[NavSectionPlan],
    page_meta: &PageMetaMap,
    page_info: &PageInfoMap,
    icon_cache: &mut IconCache,
) -> Result<Vec<NavSectionModel>> {
    sections
        .iter()
        .map(|section| {
            let items = section
                .items
                .iter()
                .map(|item| {
                    nav_item_model(
                        item,
                        section.language.clone(),
                        page_meta,
                        page_info,
                        icon_cache,
                        NavSurface::Sidebar,
                    )
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(NavSectionModel {
                language: section.language.clone(),
                title: section.title.clone(),
                items,
            })
        })
        .collect()
}

pub(super) fn menus_from_plan(
    plan: &MenusPlan,
    page_meta: &PageMetaMap,
    page_info: &PageInfoMap,
    icon_cache: &mut IconCache,
) -> Result<MenusModel> {
    let mut menus = BTreeMap::new();
    for (name, items) in &plan.items {
        let surface = NavSurface::Menu(name);
        let mut entries = items
            .iter()
            .map(|item| nav_item_model(item, None, page_meta, page_info, icon_cache, surface))
            .collect::<Result<Vec<_>>>()?;
        if name == "footer" {
            entries.retain(|entry| !entry.label_html.trim().is_empty() || !entry.href.is_empty());
        }
        menus.insert(name.clone(), entries);
    }
    Ok(MenusModel { items: menus })
}

fn nav_item_model(
    item: &NavItemPlan,
    language_override: Option<String>,
    page_meta: &PageMetaMap,
    page_info: &PageInfoMap,
    icon_cache: &mut IconCache,
    surface: NavSurface<'_>,
) -> Result<NavItemModel> {
    if let Some(url) = item.url.as_ref() {
        if !surface.allows_urls() {
            bail!("sidebar items must point to .typ source pages");
        }
        let raw_label = item.configured_label.clone().unwrap_or_else(|| url.clone());
        let label_html = nav_label_html(&raw_label, icon_cache)?;
        return Ok(NavItemModel {
            language: None,
            href: url.clone(),
            label: item
                .configured_aria_label
                .clone()
                .unwrap_or_else(|| accessible_nav_label(&raw_label, url)),
            label_html,
        });
    }

    let Some(path) = item.path.as_ref() else {
        if surface.allows_linkless_items() {
            let raw_label = item.configured_label.as_deref().unwrap_or_default();
            let label_html = nav_label_html(raw_label, icon_cache)?;
            return Ok(NavItemModel {
                language: None,
                href: String::new(),
                label: item
                    .configured_aria_label
                    .clone()
                    .unwrap_or_else(|| accessible_nav_label(raw_label, "")),
                label_html,
            });
        }

        return Err(anyhow!(
            "{} item must set path, glob, or url",
            surface.context()
        ));
    };
    let fallback = page_display_label(path, page_meta);
    let raw_label = if surface.uses_configured_labels() {
        item.configured_label
            .clone()
            .unwrap_or_else(|| fallback.clone())
    } else {
        fallback.clone()
    };
    let info = page_info.get(path).ok_or_else(|| {
        anyhow!(
            "{} page output was not planned: {}",
            surface.context(),
            path.display()
        )
    })?;
    let label_html = nav_label_html(&raw_label, icon_cache)?;
    Ok(NavItemModel {
        language: language_override.or_else(|| info.language.clone()),
        href: info.href.clone(),
        label: item
            .configured_aria_label
            .clone()
            .unwrap_or_else(|| accessible_nav_label(&raw_label, &fallback)),
        label_html,
    })
}

fn page_display_label(path: &Path, page_meta: &PageMetaMap) -> String {
    page_meta
        .get(path)
        .and_then(|meta| meta.title.clone())
        .unwrap_or_else(|| stem_label(path))
}

/// Builds the site-wide pages index consumed by `calepin.pages()` in the
/// Typst runtime: one entry per built page (the 404 page excluded), with
/// resolved `title`/`pdf` and the raw author metadata under `meta`.
pub(super) fn build_pages_index(
    src_dir: &Path,
    typ_files: &[PathBuf],
    _sections: &[NavSectionPlan],
    page_meta: &PageMetaMap,
    page_info: &PageInfoMap,
) -> serde_json::Value {
    let entries = typ_files
        .iter()
        .filter(|path| path.file_name().and_then(|name| name.to_str()) != Some(FALLBACK_PAGE))
        .map(|path| {
            let meta = page_meta.get(path);
            let title = meta
                .and_then(|meta| meta.title.clone())
                .unwrap_or_else(|| stem_label(path));
            let raw = meta
                .map(|meta| meta.raw.clone())
                .filter(serde_json::Value::is_object)
                .unwrap_or_else(|| serde_json::json!({}));
            let excerpt = meta.and_then(page_excerpt);
            serde_json::json!({
                "path": rel_posix(src_dir, path),
                "href": page_info.get(path).map(|info| info.href.clone()).unwrap_or_default(),
                "title": title,
                "language": page_info.get(path).and_then(|info| info.language.clone()),
                "translation_key": page_info.get(path).map(|info| info.translation_key.clone()).unwrap_or_default(),
                "translations": page_translations_json(path, page_info),
                "pdf": page_info.get(path).and_then(|info| info.pdf_href.clone()),
                "excerpt": excerpt,
                "meta": raw,
            })
        })
        .collect::<Vec<_>>();
    serde_json::Value::Array(entries)
}

/// An authored `summary` (or `description`) always wins over the excerpt
/// derived from the page body, so `excerpt` is a single field a template can
/// use without checking both.
fn page_excerpt(meta: &PageMeta) -> Option<String> {
    meta.raw
        .get("summary")
        .or_else(|| meta.raw.get("description"))
        .and_then(|value| value.as_str())
        .and_then(|value| clean_optional_string(Some(value)))
        .or_else(|| meta.excerpt.clone())
}

fn page_translations_json(path: &Path, page_info: &PageInfoMap) -> serde_json::Value {
    let Some(current) = page_info.get(path) else {
        return serde_json::json!({});
    };
    let translations = page_info
        .values()
        .filter(|info| info.translation_key == current.translation_key)
        .filter_map(|info| {
            info.language
                .as_ref()
                .map(|language| (language, &info.href))
        })
        .map(|(language, href)| (language.clone(), serde_json::Value::String(href.clone())))
        .collect::<serde_json::Map<_, _>>();
    serde_json::Value::Object(translations)
}

pub(super) fn write_pages_index(
    typ_files: &[PathBuf],
    index_json: &str,
    asset_dir: &Path,
) -> Result<()> {
    let dirs = typ_files
        .iter()
        .filter_map(|path| path.parent())
        .collect::<BTreeSet<_>>();
    for dir in dirs {
        let target_dir = dir.join(asset_dir);
        fs::create_dir_all(&target_dir)
            .with_context(|| format!("failed to create {}", target_dir.display()))?;
        let target = target_dir.join(PAGES_INDEX_FILE);
        fs::write(&target, index_json)
            .with_context(|| format!("failed to write {}", target.display()))?;
    }
    Ok(())
}

enum NavTarget {
    Page(PathBuf),
    Url(String),
}

fn resolve_nav_target(
    surface: NavSurface<'_>,
    src_dir: &Path,
    target: &str,
) -> Result<Option<NavTarget>> {
    let target = target.trim();
    if is_allowed_nav_absolute_or_special_url(target) {
        return Ok(Some(NavTarget::Url(resolve_literal_nav_url(
            surface, target,
        )?)));
    }
    let path = Path::new(target);
    if path.extension().and_then(|ext| ext.to_str()) != Some("typ") {
        return Ok(Some(NavTarget::Url(resolve_literal_nav_url(
            surface, target,
        )?)));
    }

    Ok(
        resolve_existing_source_page(&surface.context(), src_dir, target, "target")?
            .map(NavTarget::Page),
    )
}

fn resolve_file_list(
    src_dir: &Path,
    item_glob: Option<&str>,
    all_typ_files: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    if let Some(pattern) = item_glob {
        let pattern = slash_path(Path::new(pattern));
        return Ok(all_typ_files
            .iter()
            .filter(|path| wildcard_match(&pattern, &rel_posix(src_dir, path)))
            .cloned()
            .collect());
    }

    Ok(Vec::new())
}

fn resolve_existing_source_page(
    context: &str,
    src_dir: &Path,
    value: &str,
    field: &str,
) -> Result<Option<PathBuf>> {
    let candidate = resolve_source_relative_path(context, src_dir, value, field)?;
    if candidate.is_file() && candidate.extension().and_then(|ext| ext.to_str()) == Some("typ") {
        ensure_existing_path_under_source(context, src_dir, &candidate, value, field)?;
        return Ok(Some(candidate));
    }
    cwarn!("{context} {field} does not exist or is not a .typ file: {value}");
    Ok(None)
}

fn resolve_source_relative_path(
    context: &str,
    src_dir: &Path,
    value: &str,
    field: &str,
) -> Result<PathBuf> {
    let path = Path::new(value);
    let what = format!("{context} {field} must stay inside the source directory: {value}");
    let candidate = join_normalized_under_root(src_dir, path, &what)?;
    Ok(candidate)
}

fn ensure_existing_path_under_source(
    context: &str,
    src_dir: &Path,
    path: &Path,
    value: &str,
    field: &str,
) -> Result<()> {
    let root = src_dir
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", src_dir.display()))?;
    let path = path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", path.display()))?;
    if !path.starts_with(&root) {
        bail!("{context} {field} must stay inside the source directory: {value}");
    }
    Ok(())
}

fn resolve_literal_nav_url(surface: NavSurface<'_>, target: &str) -> Result<String> {
    if !surface.allows_urls() {
        bail!("sidebar target must point to a .typ source page, got literal URL: {target}");
    }
    if is_allowed_nav_absolute_or_special_url(target) {
        return Ok(target.to_string());
    }
    let route = target.trim_start_matches("./");
    if target.contains(':') || !is_safe_output_route(route) {
        bail!(
            "{} target is an unsafe URL or output route: {target}",
            surface.context()
        );
    }
    Ok(target.to_string())
}

fn is_allowed_nav_absolute_or_special_url(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with('#')
        || value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("//")
        || value.starts_with("mailto:")
        || value.starts_with("tel:")
}

pub(super) fn iter_typ_files(
    src_dir: &Path,
    include_hidden: bool,
    exclude: &[PathBuf],
    asset_dir: &Path,
) -> Result<Vec<PathBuf>> {
    let exclude = exclude.iter().collect::<BTreeSet<_>>();
    let mut out = Vec::new();
    collect_typ_files(
        src_dir,
        src_dir,
        include_hidden,
        &exclude,
        asset_dir,
        &mut out,
    )?;
    out.sort_by_key(|path| rel_posix(src_dir, path));
    Ok(out)
}

fn collect_typ_files(
    root: &Path,
    dir: &Path,
    include_hidden: bool,
    exclude: &BTreeSet<&PathBuf>,
    asset_dir: &Path,
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    collect_files_by(
        root,
        dir,
        out,
        |rel, _| (include_hidden || !has_hidden_component(rel)) && !rel.starts_with(asset_dir),
        // Generated entry files are Calepin's own staged copies of pages. They
        // are never pages themselves, including when `include_hidden` lets an
        // explicit `[pages] include` glob reach hidden paths.
        |rel, path| {
            (include_hidden || !has_hidden_component(rel))
                && !rel.starts_with(asset_dir)
                && !is_generated_entry_file(rel)
                && path.extension().and_then(|ext| ext.to_str()) == Some("typ")
                && !exclude.contains(&rel.to_path_buf())
        },
    )
}

fn has_hidden_component(path: &Path) -> bool {
    path.components()
        .any(|part| part.as_os_str().to_string_lossy().starts_with('.'))
}

fn stem_label(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .replace(['-', '_'], " ")
}
