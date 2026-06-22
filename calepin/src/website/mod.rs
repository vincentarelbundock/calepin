mod config;
mod feeds;
mod icons;
mod language;
mod metadata;
mod navigation;
mod outputs;
mod pagefind;
mod paths;
mod preprocess;
mod render;
mod scaffold;
mod serve;
mod site;
mod svg;
mod templates;
mod url;
mod util;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use notify::RecursiveMode;
use serde::{Deserialize, Serialize};
use xxhash_rust::xxh3::xxh3_64;

use crate::cli::{set_quiet, CompileArgs, CompileFormat, WatchArgs};
use crate::config::CalepinConfig;
use crate::html::HtmlSyntaxTheme;
use crate::utils::path::absolutize_from;
use crate::utils::progress::ProgressManager;
use crate::utils::static_files::path_has_skip_dir;
pub(crate) use crate::utils::static_files::COMMON_SKIP_DIRS as SKIP_DIRS;
use crate::utils::watch::{is_rebuild_event, run_debounced_watch_until};
#[cfg(test)]
use config::{LanguageConfig, SidebarItemConfig, SidebarSectionConfig};
#[cfg(test)]
use config::{MenuItemConfig, PagesConfig, SidebarConfig, StaticConfig};
use config::{SearchEngine, WebsiteConfig};
#[cfg(test)]
use feeds::{feed_items_from_pages, infer_feed_format, rss_feed_date, FeedFormat, FeedTarget};
use feeds::{feed_targets, write_feeds};
use icons::IconCache;
use language::{configured_languages, LanguageInfo};
#[cfg(test)]
use metadata::PageMeta;
#[cfg(test)]
use metadata::{extract_document_title, page_meta_from_value};
use metadata::{load_page_meta, PageMetaMap};
use navigation::{
    build_page_info, build_pages_index, discover_site_build_pages, discover_site_menus,
    discover_site_pages, discover_static_files, fallback_pages, implicit_build_pages,
    menus_from_plan, nav_from_plans, write_pages_index, MenusModel, NavSectionModel,
};
#[cfg(test)]
use navigation::{
    discover_menus, discover_pages, MenuItemPlan, MenusPlan, NavItemPlan, NavSectionPlan,
};
#[cfg(test)]
use outputs::MANIFEST_PATH;
use outputs::{
    clear_previous_outputs, copy_static_files, copy_typ_sources, expected_generated_outputs,
    load_manifest, reconcile_manifest_outputs, remove_unexpected_rendered_outputs,
    static_output_paths, write_default_favicon, write_manifest, GeneratedOutputInputs,
};
use pagefind::{
    base_url_path_prefix, cached_pagefind_outputs, manifest_output_paths, pagefind_pages,
    pagefind_signature, remove_stale_pagefind_outputs, write_pagefind_index, PAGEFIND_DIR,
};
#[cfg(test)]
use paths::wildcard_match;
use paths::{normalize_path, rel_posix, relative_or_self, slash_path};
use preprocess::{preprocess_documents, WebsitePreprocessOptions};
#[cfg(test)]
use render::render_documents;
pub(crate) use scaffold::scaffold_website;
#[cfg(test)]
use site::{language_entries, translation_entries};
use site::{SiteMetadata, SiteModel};
#[cfg(test)]
use svg::sanitize_icon_svg;
use templates::{write_robots, write_sitemap};
#[cfg(test)]
use url::page_relative_url;
use util::clean_optional_string;

const DEFAULT_CONFIG: &str = "calepin.toml";
const DEFAULT_SRC_DIR: &str = "docs";
const DEFAULT_WEBSITE_ASSET_DIR: &str = ".calepin";
const DEFAULT_FAVICON_NAME: &str = "favicon.svg";
const FALLBACK_PAGE: &str = "404.typ";
const INDEX_PAGE: &str = "index.typ";
const SOURCE_DATA_ID: &str = "calepin-website-source-data";
#[cfg(test)]
const ICON_CACHE_DIR: &str = ".calepin/icons";
const ICON_CACHE_SUBDIR: &str = "icons";
const PAGES_INDEX_FILE: &str = "website-pages.json";
const ROBOTS_FILE: &str = "robots.txt";
const ROBOTS_TEMPLATE_DIR: &str = "templates";
const ROBOTS_TEMPLATE_FILE: &str = "robots.txt";
const DEFAULT_ROBOTS_TEMPLATE: &str =
    "User-agent: *\nAllow: /\n{% if sitemap_url %}Sitemap: {{ sitemap_url }}\n{% endif %}";
fn resolve_website_asset_dir(config: &WebsiteConfig) -> Result<PathBuf> {
    let raw = match config.asset_dir.as_ref() {
        Some(value) if value.as_os_str().is_empty() => {
            bail!("website `asset-dir` must not be empty")
        }
        Some(value) => value,
        None => Path::new(DEFAULT_WEBSITE_ASSET_DIR),
    };
    let raw = raw.to_path_buf();
    if raw.is_absolute() {
        bail!(
            "website `asset-dir` must be a relative path: {}",
            raw.display()
        );
    }
    if raw.to_string_lossy().contains('\\') {
        bail!(
            "website `asset-dir` path must not contain '\\': {}",
            raw.display()
        );
    }
    if raw
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        bail!(
            "website `asset-dir` path must not escape the source directory: {}",
            raw.display()
        );
    }
    let normalized = normalize_path(&raw);
    if normalized.as_os_str().is_empty() {
        bail!("website `asset-dir` must include at least one path segment");
    }

    Ok(normalized)
}

fn website_asset_default_favicon_path(asset_dir: &Path) -> PathBuf {
    asset_dir.join(DEFAULT_FAVICON_NAME)
}

fn website_icon_cache_dir(asset_dir: &Path) -> PathBuf {
    asset_dir.join(ICON_CACHE_SUBDIR)
}

pub(crate) fn build_from_compile_args(args: CompileArgs) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let config_path =
        discover_website_config(&current_dir, &args.input, args.common.config.as_deref())?;
    let render_pdf = match args.format {
        None => None,
        Some(CompileFormat::Html) => Some(false),
        Some(format) => {
            return Err(anyhow!(
                "website directory builds only support `--format html` or no `--format`, got `{}`",
                format.as_str()
            ));
        }
    };
    set_quiet(args.common.quiet);
    build_site(WebsiteBuildOptions {
        config: config_path,
        src: Some(args.input),
        out: args.output,
        parallelism: None,
        render_pdf,
        quiet: args.common.quiet,
        timeout: args.common.timeout,
        params: args.common.params,
        typst_args: args.typst_args,
        incremental_inputs: None,
        clean: true,
        minify_html: args.minify,
    })?;
    Ok(())
}

pub(crate) fn watch_from_watch_args(args: WatchArgs) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let config_path =
        discover_website_config(&current_dir, &args.input, args.common.config.as_deref())?;
    let render_pdf = match args.format {
        None => None,
        Some(CompileFormat::Html) => Some(false),
        Some(_) => {
            return Err(anyhow!(
                "website directory watch does not support `--format`; only `--format html` is allowed and `--format` other values are for one-shot format control via `calepin compile`"
            ));
        }
    };
    set_quiet(args.common.quiet);
    if args.open && !args.serve {
        return Err(anyhow!(
            "`calepin watch --open` requires `--serve` when watching a website directory"
        ));
    }
    let options = WebsiteBuildOptions {
        config: config_path,
        src: Some(args.input.clone()),
        out: args.output.clone(),
        parallelism: None,
        render_pdf,
        quiet: args.common.quiet,
        timeout: args.common.timeout,
        params: args.common.params.clone(),
        typst_args: args.typst_args.clone(),
        incremental_inputs: None,
        clean: true,
        minify_html: false,
    };
    let initial = build_site(options.clone())?;
    let live = serve::LiveReload::new();
    let server = if args.serve {
        Some(serve::start(
            &initial.out_dir,
            &args.host,
            args.port,
            Arc::clone(&live),
            args.open,
        )?)
    } else {
        None
    };

    let result = watch_site(options, initial, live, args.common.quiet);
    if let Some(server) = server {
        server.stop();
    }
    result
}

pub(crate) use serve::serve;

#[derive(Clone)]
struct WebsiteBuildOptions {
    config: PathBuf,
    src: Option<PathBuf>,
    out: Option<PathBuf>,
    parallelism: Option<usize>,
    /// `None` defers to the `pdf` key in calepin.toml (default: HTML only).
    render_pdf: Option<bool>,
    quiet: bool,
    timeout: Option<u64>,
    params: Vec<String>,
    typst_args: Vec<String>,
    incremental_inputs: Option<Vec<PathBuf>>,
    clean: bool,
    minify_html: bool,
}

#[derive(Debug, Clone)]
struct PageInfo {
    language: Option<String>,
    translation_key: String,
    href: String,
    pdf_href: Option<String>,
}

type PageInfoMap = BTreeMap<PathBuf, PageInfo>;

#[derive(Debug, Clone)]
struct WebsiteBuildResult {
    src_dir: PathBuf,
    out_dir: PathBuf,
    asset_dir: PathBuf,
    config_path: PathBuf,
    theme_dirs: Vec<PathBuf>,
    page_fingerprints: BTreeMap<PathBuf, u64>,
    nav_signature: u64,
    /// Hash of the pages index; when it changes, every page may render
    /// differently (listings), so incremental rebuilds fall back to full.
    pages_signature: u64,
}

fn theme_chain_dirs(theme: &crate::theme::ThemeSelection) -> Result<Vec<PathBuf>> {
    let chain = crate::theme::resolve_theme_chain(theme)?;
    chain
        .layers
        .into_iter()
        .filter_map(|layer| match layer {
            crate::theme::ThemeLayer::Dir(path) => Some(path),
            crate::theme::ThemeLayer::Builtin(_) => None,
        })
        .map(|path| {
            path.canonicalize()
                .with_context(|| format!("failed to resolve theme directory {}", path.display()))
        })
        .collect()
}

fn build_site(args: WebsiteBuildOptions) -> Result<WebsiteBuildResult> {
    let current_dir = std::env::current_dir()?;
    let config_path = resolve_config_path(&current_dir, Some(args.config.as_path()))?;
    let config = load_website_config(&config_path, true)?;

    let src_dir = resolve_cli_path(
        &current_dir,
        args.src
            .as_deref()
            .unwrap_or_else(|| Path::new(DEFAULT_SRC_DIR)),
    );
    let out_dir = match args.out.as_deref() {
        Some(out) => resolve_cli_path(&current_dir, out),
        None => src_dir.clone(),
    };
    if !src_dir.is_dir() {
        return Err(anyhow!("source directory not found: {}", src_dir.display()));
    }
    let src_dir = fs::canonicalize(&src_dir)
        .with_context(|| format!("failed to resolve {}", src_dir.display()))?;
    let out_dir = if out_dir.exists() {
        fs::canonicalize(&out_dir)
            .with_context(|| format!("failed to resolve {}", out_dir.display()))?
    } else {
        out_dir
    };

    let calepin_config = CalepinConfig::load(&src_dir, Some(&config_path))?;
    let config_theme = calepin_config.theme_selection()?.unwrap_or_default();
    let site_theme = config_theme.clone();
    let asset_dir = resolve_website_asset_dir(&config)?;
    let theme_dirs = theme_chain_dirs(&site_theme)?;
    let languages = configured_languages(&src_dir, &config)?;
    let (section_plans, mut typ_files) = discover_site_pages(
        &src_dir,
        config.sidebar.as_ref(),
        config.pages.as_ref(),
        &languages,
    )?;
    let (menus_plan, mut menu_files) = discover_site_menus(
        &src_dir,
        &config.menus,
        config.footer.as_ref(),
        config.pages.as_ref(),
        &languages,
    )?;
    typ_files.append(&mut menu_files);
    let mut included_pages =
        discover_site_build_pages(&src_dir, config.pages.as_ref(), &languages)?;
    typ_files.append(&mut included_pages);
    typ_files.extend(implicit_build_pages(&src_dir, &languages));
    typ_files.sort_by_key(|path| rel_posix(&src_dir, path));
    typ_files.dedup();
    let fallback_files = fallback_pages(&src_dir, &languages);
    let page_fingerprints = fingerprint_files(&typ_files)?;
    let static_files = discover_static_files(&src_dir, config.static_files.as_ref())?;
    let config_dir = config_path.parent().unwrap_or(&src_dir);
    let html_syntax_theme = HtmlSyntaxTheme::from_paths(
        config_dir,
        config.highlight_light.as_deref(),
        config.highlight_dark.as_deref(),
    )?;
    let build_set = match &args.incremental_inputs {
        Some(inputs) => {
            let wanted = inputs.iter().cloned().collect::<BTreeSet<_>>();
            typ_files
                .iter()
                .filter(|path| wanted.contains(*path))
                .cloned()
                .collect::<Vec<_>>()
        }
        None => typ_files.clone(),
    };
    let progress = ProgressManager::new(args.quiet);

    // Phase 1: preprocess the build set. This executes code chunks and also
    // extracts each page's `<website-metadata>` from the staged source, where
    // imports resolve.
    let preprocessed = preprocess_documents(WebsitePreprocessOptions {
        typ_files: &build_set,
        src_dir: &src_dir,
        config_path: &config_path,
        quiet: args.quiet,
        timeout: args.timeout,
        params: &args.params,
        fallback_theme: config_theme.clone(),
        html_syntax_theme: html_syntax_theme.clone(),
        asset_dir: &asset_dir,
        parallelism: args.parallelism,
        progress: progress.clone(),
    })?;
    let page_meta = load_page_meta(&src_dir, &typ_files);
    let metadata = SiteMetadata::from_config(
        &config,
        &src_dir,
        &slash_path(&website_asset_default_favicon_path(&asset_dir)),
    )?;
    let default_favicon_path = if clean_optional_string(config.favicon.as_deref()).is_none() {
        Some(website_asset_default_favicon_path(&asset_dir))
    } else {
        None
    };
    let sitemap_path = metadata
        .base_url
        .as_ref()
        .map(|_| out_dir.join("sitemap.xml"));
    let robots_path = config.robots_enabled().then(|| out_dir.join(ROBOTS_FILE));
    let feed_targets = feed_targets(&config)?;
    let feed_paths: BTreeSet<PathBuf> = if config.feeds_enabled() {
        feed_targets
            .iter()
            .map(|feed| out_dir.join(&feed.filename))
            .collect()
    } else {
        BTreeSet::new()
    };
    let minify_html = args.minify_html || config.minify.unwrap_or(false);
    let pdf_files = pdf_enabled_files(&typ_files, &page_meta, args.render_pdf, config.pdf);
    let page_info = build_page_info(&src_dir, &typ_files, &page_meta, &pdf_files, &languages)?;
    let pagefind_pages = pagefind_pages(&out_dir, &typ_files, &page_info, &fallback_files);
    let icon_cache_dir = website_icon_cache_dir(&asset_dir);
    let mut icon_cache = IconCache::new(&src_dir, &icon_cache_dir);
    let pages_index_ref = format!("/{}/{}", slash_path(&asset_dir), PAGES_INDEX_FILE);
    let sidebar_sections = nav_from_plans(&section_plans, &page_meta, &page_info, &mut icon_cache)?;
    let menus = menus_from_plan(&menus_plan, &page_meta, &page_info, &mut icon_cache)?;
    let nav_signature = navigation_signature(&sidebar_sections) ^ menus_signature(&menus);
    let pages_index =
        build_pages_index(&src_dir, &typ_files, &section_plans, &page_meta, &page_info);
    let pages_index_json = serde_json::to_string_pretty(&pages_index)?;
    let pages_signature = xxh3_64(pages_index_json.as_bytes());
    write_pages_index(&typ_files, &pages_index_json, &asset_dir)?;
    let expected_outputs = expected_generated_outputs(GeneratedOutputInputs {
        out_dir: &out_dir,
        typ_files: &typ_files,
        page_info: &page_info,
        sitemap_path: &sitemap_path,
        robots_path: &robots_path,
        feed_paths: &feed_paths,
        default_favicon_path: default_favicon_path.as_deref(),
    });
    let mut expected_outputs = if out_dir == src_dir {
        expected_outputs
    } else {
        expected_outputs
            .into_iter()
            .chain(static_output_paths(&src_dir, &out_dir, &static_files))
            .collect()
    };
    let previous_manifest = load_manifest(&out_dir)?;
    let protected_pagefind_outputs = if config.search == Some(SearchEngine::Pagefind) {
        previous_manifest
            .pagefind
            .as_ref()
            .map(|pagefind| manifest_output_paths(&out_dir, &pagefind.outputs))
            .transpose()?
            .unwrap_or_default()
    } else {
        BTreeSet::new()
    };
    expected_outputs.extend(protected_pagefind_outputs.iter().cloned());

    let output_progress = progress.spinner("[site] prepare output");
    if args.clean {
        clear_previous_outputs(
            &src_dir,
            &out_dir,
            config.search == Some(SearchEngine::Pagefind) && previous_manifest.pagefind.is_some(),
        )?;
    }
    reconcile_manifest_outputs(&out_dir, &previous_manifest, &expected_outputs)?;
    if args.clean && out_dir != src_dir {
        remove_unexpected_rendered_outputs(&out_dir, &expected_outputs)?;
    }
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;
    output_progress.finish("[done] prepare output");

    let asset_progress = progress.spinner("[site] write assets");
    if let Some(path) = default_favicon_path.as_deref() {
        write_default_favicon(&out_dir, path)?;
    }

    if out_dir != src_dir {
        if args.incremental_inputs.is_none() {
            copy_static_files(&src_dir, &out_dir, &static_files)?;
        }
        let source_files = if args.incremental_inputs.is_some() {
            build_set.clone()
        } else {
            typ_files.clone()
        };
        copy_typ_sources(&src_dir, &out_dir, &source_files)?;
    }
    asset_progress.finish("[done] write assets");

    // Phase 2: render the preprocessed pages with full site context.
    let site = SiteModel::new(
        sidebar_sections,
        menus,
        metadata.clone(),
        config.sidebar.as_ref().is_none_or(|sidebar| sidebar.fold),
    );
    render_documents(
        &BuildContext {
            src_dir: src_dir.clone(),
            out_dir: out_dir.clone(),
            typst: calepin_config.executables.typst,
            pdf_files,
            page_meta: page_meta.clone(),
            page_info: page_info.clone(),
            languages: languages.clone(),

            syntax_theme: html_syntax_theme,
            parallelism: args.parallelism,
            typst_args: args.typst_args,
            minify_html,
            search: config.search,
            pages_index_ref: pages_index_ref.clone(),
            progress: progress.clone(),
        },
        build_set,
        &site,
        &preprocessed,
    )?;
    let sitemap_hrefs = typ_files
        .iter()
        .filter(|path| !fallback_files.contains(path))
        .filter_map(|path| page_info.get(path).map(|info| info.href.clone()))
        .collect::<BTreeSet<_>>();
    let site_files_progress = progress.spinner("[site] write sitemap, feeds, robots");
    write_sitemap(&out_dir, metadata.base_url.as_deref(), &sitemap_hrefs)?;
    write_robots(&out_dir, &src_dir, &config, metadata.base_url.as_deref())?;
    write_feeds(
        &out_dir,
        &src_dir,
        &config,
        metadata.base_url.as_deref(),
        &metadata,
        &pages_index,
        &feed_targets,
    )?;
    site_files_progress.finish("[done] write sitemap, feeds, robots");
    let pagefind_manifest = if config.search == Some(SearchEngine::Pagefind) {
        let pagefind_progress = progress.spinner("[pagefind] index");
        let signature = pagefind_signature(&out_dir, &pagefind_pages)?;
        let cached_outputs = cached_pagefind_outputs(&out_dir, &previous_manifest, signature)?;
        let outputs = if let Some(outputs) = cached_outputs {
            pagefind_progress.finish(format!("[cache] {PAGEFIND_DIR}/"));
            outputs
        } else {
            expected_outputs.retain(|path| !protected_pagefind_outputs.contains(path));
            let outputs = write_pagefind_index(&out_dir, &pagefind_pages)?;
            remove_stale_pagefind_outputs(&out_dir, &previous_manifest, &outputs)?;
            expected_outputs.extend(outputs.iter().cloned());
            pagefind_progress.finish(format!("[done] {PAGEFIND_DIR}/"));
            outputs
        };
        expected_outputs.extend(outputs.iter().cloned());
        Some(PagefindManifest {
            signature,
            outputs: outputs
                .iter()
                .map(|path| rel_posix(&out_dir, path))
                .collect(),
        })
    } else {
        None
    };
    let manifest_progress = progress.spinner("[site] write manifest");
    write_manifest(&out_dir, &expected_outputs, pagefind_manifest)?;
    manifest_progress.finish("[done] write manifest");
    let out_dir = out_dir
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", out_dir.display()))?;
    Ok(WebsiteBuildResult {
        src_dir,
        out_dir,
        asset_dir,
        config_path,
        theme_dirs,
        page_fingerprints,
        nav_signature,
        pages_signature,
    })
}

fn watch_site(
    options: WebsiteBuildOptions,
    initial: WebsiteBuildResult,
    live: Arc<serve::LiveReload>,
    quiet: bool,
) -> Result<()> {
    let mut current = initial;
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_handler = Arc::clone(&stop);
    ctrlc::set_handler(move || {
        stop_for_handler.store(true, Ordering::Relaxed);
    })
    .context("failed to set Ctrl+C handler")?;

    if !quiet {
        eprintln!("Watching {}", current.src_dir.display());
        eprintln!("Press Ctrl+C to stop.");
    }

    loop {
        let watches = watch_roots(&current);
        let mut restart = false;
        run_debounced_watch_until(
            &watches,
            Duration::from_millis(350),
            Duration::from_millis(200),
            Arc::clone(&stop),
            is_rebuild_event,
            Some,
            |raw_changed| {
                let changed = raw_changed
                    .iter()
                    .filter(|path| should_rebuild_for_path(&current, path))
                    .cloned()
                    .collect::<Vec<_>>();
                if changed.is_empty() {
                    return true;
                }
                if !quiet {
                    let names = changed
                        .iter()
                        .filter_map(|path| path.file_name())
                        .map(|name| name.to_string_lossy().to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    eprintln!("rebuilding {names}...");
                }
                match rebuild_changed_pages(&options, &current, &changed) {
                    Ok(Some(next)) => {
                        restart = watch_roots_changed(&current, &next);
                        current = next;
                        live.rebuilt();
                        !restart
                    }
                    Ok(None) => true,
                    Err(error) => {
                        cwarn!("website rebuild failed: {}", error);
                        live.set_error(format!("{error:#}"));
                        true
                    }
                }
            },
        )?;
        if stop.load(Ordering::Relaxed) || !restart {
            return Ok(());
        }
    }
}

fn watch_roots(current: &WebsiteBuildResult) -> Vec<(PathBuf, RecursiveMode)> {
    let mut watches = vec![
        (current.src_dir.clone(), RecursiveMode::Recursive),
        (current.config_path.clone(), RecursiveMode::NonRecursive),
    ];
    for theme_dir in &current.theme_dirs {
        watches.push((theme_dir.to_path_buf(), RecursiveMode::Recursive));
    }
    watches
}

fn watch_roots_changed(previous: &WebsiteBuildResult, next: &WebsiteBuildResult) -> bool {
    watch_roots(previous) != watch_roots(next)
}

fn should_rebuild_for_path(initial: &WebsiteBuildResult, path: &Path) -> bool {
    if path == initial.config_path {
        return true;
    }
    if initial
        .theme_dirs
        .iter()
        .any(|theme_dir| path.starts_with(theme_dir))
    {
        return true;
    }
    // A distinct output directory only ever receives generated copies; reacting
    // to them would re-trigger the build that produced them.
    if initial.out_dir != initial.src_dir && path.starts_with(&initial.out_dir) {
        return false;
    }
    if !path.starts_with(&initial.src_dir) {
        return false;
    }
    let rel = relative_or_self(&initial.src_dir, path);
    if rel.components().next().is_none() {
        return false;
    }
    if path_has_skip_dir(rel, &[initial.asset_dir.as_path()]) {
        return false;
    }
    if path.starts_with(&initial.out_dir) {
        if let Some("html" | "pdf") = path.extension().and_then(|extension| extension.to_str()) {
            return false;
        }
    }
    true
}

fn rebuild_changed_pages(
    options: &WebsiteBuildOptions,
    current: &WebsiteBuildResult,
    changed: &[PathBuf],
) -> Result<Option<WebsiteBuildResult>> {
    let Some(pages) = changed_typ_pages(current, changed)? else {
        return Ok(Some(build_site(options.clone())?));
    };
    if pages.is_empty() {
        return Ok(None);
    }

    let mut incremental_options = options.clone();
    incremental_options.incremental_inputs = Some(pages);
    incremental_options.clean = false;
    let next = build_site(incremental_options)?;
    if next.nav_signature != current.nav_signature
        || next.pages_signature != current.pages_signature
        || next
            .page_fingerprints
            .keys()
            .ne(current.page_fingerprints.keys())
    {
        return Ok(Some(build_site(options.clone())?));
    }
    Ok(Some(next))
}

fn fingerprint_files(paths: &[PathBuf]) -> Result<BTreeMap<PathBuf, u64>> {
    paths
        .iter()
        .map(|path| {
            let bytes =
                fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
            Ok((path.clone(), xxh3_64(&bytes)))
        })
        .collect()
}

fn navigation_signature(sections: &[NavSectionModel]) -> u64 {
    let mut bytes = Vec::new();
    for section in sections {
        if let Some(language) = &section.language {
            bytes.extend_from_slice(language.as_bytes());
        }
        bytes.push(0);
        if let Some(title) = &section.title {
            bytes.extend_from_slice(title.as_bytes());
        }
        bytes.push(0);
        for item in &section.items {
            bytes.extend_from_slice(item.href.as_bytes());
            bytes.push(0);
            bytes.extend_from_slice(item.label.as_bytes());
            bytes.push(0);
            bytes.extend_from_slice(item.label_html.as_bytes());
            bytes.push(0);
        }
        bytes.push(0xff);
    }
    xxh3_64(&bytes)
}

fn menus_signature(menus: &MenusModel) -> u64 {
    let mut bytes = Vec::new();
    for (name, items) in &menus.items {
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(0);
        for item in items {
            if let Some(language) = &item.language {
                bytes.extend_from_slice(language.as_bytes());
            }
            bytes.push(0);
            bytes.extend_from_slice(item.href.as_bytes());
            bytes.push(0);
            bytes.extend_from_slice(item.label.as_bytes());
            bytes.push(0);
            bytes.extend_from_slice(item.label_html.as_bytes());
            bytes.push(0);
        }
        bytes.push(0xff);
    }
    xxh3_64(&bytes)
}

fn changed_typ_pages(
    current: &WebsiteBuildResult,
    changed: &[PathBuf],
) -> Result<Option<Vec<PathBuf>>> {
    let mut pages = Vec::new();
    for path in changed {
        if path.extension().and_then(|extension| extension.to_str()) != Some("typ") {
            return Ok(None);
        }
        if !path.starts_with(&current.src_dir) || !path.is_file() {
            return Ok(None);
        }
        let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
        let fingerprint = xxh3_64(&bytes);
        match current.page_fingerprints.get(path) {
            Some(previous) if *previous == fingerprint => {}
            Some(_) => pages.push(path.clone()),
            None => return Ok(None),
        }
    }
    pages.sort();
    pages.dedup();
    Ok(Some(pages))
}

fn pdf_enabled_files(
    typ_files: &[PathBuf],
    page_meta: &PageMetaMap,
    cli_render_pdf: Option<bool>,
    config_pdf: Option<bool>,
) -> BTreeSet<PathBuf> {
    // `--format html` is one-shot format control; page metadata cannot
    // override it.
    if cli_render_pdf == Some(false) {
        return BTreeSet::new();
    }
    let default = cli_render_pdf.unwrap_or_else(|| config_pdf.unwrap_or(false));
    typ_files
        .iter()
        .filter(|path| {
            page_meta
                .get(*path)
                .and_then(|meta| meta.pdf)
                .unwrap_or(default)
        })
        .cloned()
        .collect()
}

#[derive(Debug, Deserialize, Serialize, Default)]
struct WebsiteManifest {
    outputs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pagefind: Option<PagefindManifest>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PagefindManifest {
    signature: u64,
    outputs: Vec<String>,
}

#[derive(Clone)]
struct BuildContext {
    src_dir: PathBuf,
    out_dir: PathBuf,
    typst: PathBuf,
    pdf_files: BTreeSet<PathBuf>,
    page_meta: PageMetaMap,
    page_info: PageInfoMap,
    languages: Option<Vec<LanguageInfo>>,

    syntax_theme: HtmlSyntaxTheme,
    parallelism: Option<usize>,
    typst_args: Vec<String>,
    minify_html: bool,
    search: Option<SearchEngine>,
    pages_index_ref: String,
    progress: ProgressManager,
}

fn load_website_config(path: &Path, required: bool) -> Result<WebsiteConfig> {
    if !path.is_file() {
        if required {
            return Err(anyhow!("config file not found: {}", path.display()));
        }
        return Ok(WebsiteConfig::default());
    }
    let contents =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))
}

/// Resolves the website config: an explicit `--config` path wins, otherwise
/// the input directory is searched for `calepin.toml`.
fn discover_website_config(
    current_dir: &Path,
    input: &Path,
    explicit: Option<&Path>,
) -> Result<PathBuf> {
    if let Some(config) = explicit {
        return Ok(absolutize_from(current_dir, config));
    }
    let input_dir = resolve_cli_path(current_dir, input);
    let preferred = input_dir.join(DEFAULT_CONFIG);
    if preferred.is_file() {
        return Ok(preferred);
    }
    Err(anyhow!(
        "no {DEFAULT_CONFIG} found in {}; create one with `calepin new website` or pass `--config <path>`",
        input_dir.display()
    ))
}

fn resolve_config_path(current_dir: &Path, value: Option<&Path>) -> Result<PathBuf> {
    let path = value.unwrap_or_else(|| Path::new(DEFAULT_CONFIG));
    Ok(absolutize_from(current_dir, path))
}

fn resolve_cli_path(current_dir: &Path, path: &Path) -> PathBuf {
    absolutize_from(current_dir, path)
}

#[cfg(test)]
mod tests;
