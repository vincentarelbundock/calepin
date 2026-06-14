mod serve;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use notify::RecursiveMode;
use notify_debouncer_full::new_debouncer;
use serde::{Deserialize, Serialize};
use xxhash_rust::xxh3::xxh3_64;

use crate::cli::{set_quiet, CompileArgs, CompileFormat, WatchArgs};
use crate::config::CalepinConfig;
use crate::html::{
    write_html_theme_stylesheet, SiteContextInput, SiteLanguageEntry, SiteNavEntry, SiteNavSection,
};
use crate::typst::compile::{compile_with_typst, CompileOptions};
use crate::typst::preprocess::{
    preprocess_cached, read_page_meta_with_root, PreprocessOptions, PreprocessOutput,
};

const DEFAULT_CONFIG: &str = "calepin.toml";
const DEFAULT_SRC_DIR: &str = "docs";
const WEBSITE_STYLESHEET_PATH: &str = ".calepin/calepin-website.css";
const DEFAULT_FAVICON_PATH: &str = ".calepin/favicon.svg";
const DEFAULT_FAVICON_SVG: &str = include_str!("../assets/default-favicon.svg");
const FALLBACK_PAGE: &str = "404.typ";
const INDEX_PAGE: &str = "index.typ";
const SOURCE_DATA_ID: &str = "calepin-website-source-data";
const MANIFEST_PATH: &str = ".calepin/website-manifest.json";
const ICON_CACHE_DIR: &str = ".calepin/icons";
const DEFAULT_ICON_PREFIX: &str = "lucide";
const ICON_DOWNLOAD_TIMEOUT_SECS: u64 = 5;
const PAGES_INDEX_FILE: &str = "website-pages.json";
/// Root-relative reference to the pages index. Each page renders with
/// `--root` at its own directory, so the index is written into every page
/// directory's `.calepin` and this reference resolves for all of them.
const PAGES_INDEX_REF: &str = "/.calepin/website-pages.json";
const SKIP_DIRS: &[&str] = &[".calepin", ".git", "target", "node_modules", ".venv"];

pub(crate) fn scaffold_website(dir: &Path, force: bool) -> Result<()> {
    let docs = absolutize_for_create(dir)?;
    fs::create_dir_all(&docs).with_context(|| format!("failed to create {}", docs.display()))?;

    write_scaffold_file(&docs.join(DEFAULT_CONFIG), WEBSITE_TOML_TEMPLATE, force)?;
    write_scaffold_file(&docs.join("index.typ"), INDEX_TYP_TEMPLATE, force)?;
    write_scaffold_file(&docs.join("404.typ"), NOT_FOUND_TYP_TEMPLATE, force)?;
    Ok(())
}

pub(crate) fn scaffold_academic_website(dir: &Path, force: bool) -> Result<()> {
    let docs = absolutize_for_create(dir)?;
    fs::create_dir_all(&docs).with_context(|| format!("failed to create {}", docs.display()))?;

    for (path, contents) in ACADEMIC_SCAFFOLD_FILES {
        write_scaffold_file(&docs.join(path), contents, force)?;
    }
    Ok(())
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
        theme: args.theme,
        parallelism: None,
        render_pdf,
        quiet: args.common.quiet,
        timeout: args.common.timeout,
        params: args.common.params,
        typst_args: args.typst_args,
        incremental_inputs: None,
        clean: true,
    })?;
    Ok(())
}

pub(crate) fn watch_from_watch_args(args: WatchArgs) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let config_path =
        discover_website_config(&current_dir, &args.input, args.common.config.as_deref())?;
    if args.format.is_some() {
        return Err(anyhow!(
            "website directory watch does not support `--format`; use `calepin compile` for one-shot format control"
        ));
    }

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
        theme: None,
        parallelism: None,
        render_pdf: None,
        quiet: args.common.quiet,
        timeout: args.common.timeout,
        params: args.common.params.clone(),
        typst_args: args.typst_args.clone(),
        incremental_inputs: None,
        clean: true,
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
    theme: Option<String>,
    parallelism: Option<usize>,
    /// `None` defers to the `pdf` key in calepin.toml (default: HTML only).
    render_pdf: Option<bool>,
    quiet: bool,
    timeout: Option<u64>,
    params: Vec<String>,
    typst_args: Vec<String>,
    incremental_inputs: Option<Vec<PathBuf>>,
    clean: bool,
}

/// Per-page metadata exposed through the `<website-metadata>` Typst label,
/// extracted during preprocessing and persisted under `.calepin/`. `title` and
/// `pdf` are the keys calepin interprets; `raw` carries the author's whole
/// dictionary verbatim for the pages index.
#[derive(Debug, Clone, Default, PartialEq)]
struct PageMeta {
    title: Option<String>,
    pdf: Option<bool>,
    translation_key: Option<String>,
    slug: Option<String>,
    url: Option<String>,
    raw: serde_json::Value,
}

type PageMetaMap = BTreeMap<PathBuf, PageMeta>;

#[derive(Debug, Clone)]
struct LanguageInfo {
    code: String,
    label: String,
    content_dir: PathBuf,
    url_prefix: String,
    default: bool,
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
    config_path: PathBuf,
    theme_dir: Option<PathBuf>,
    page_fingerprints: BTreeMap<PathBuf, u64>,
    nav_signature: u64,
    /// Hash of the pages index; when it changes, every page may render
    /// differently (listings), so incremental rebuilds fall back to full.
    pages_signature: u64,
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
    let config_dir = config_path.parent().unwrap_or(&current_dir);
    let cli_theme = args
        .theme
        .as_deref()
        .map(|value| crate::theme::ThemeSelection::parse(value, &current_dir))
        .transpose()?;
    let config_theme = config.theme_selection(config_dir)?;
    let site_theme = cli_theme.clone().unwrap_or_else(|| config_theme.clone());
    let theme_dir = match &site_theme {
        crate::theme::ThemeSelection::Dir(dir) => Some(dir.clone()),
        _ => None,
    };
    let site_entry = crate::theme::resolve_html_entry(&site_theme, crate::theme::HtmlScope::Site)?;
    let mut theme_stylesheet_path = site_entry
        .as_ref()
        .filter(|entry| entry.is_default)
        .map(|_| PathBuf::from(WEBSITE_STYLESHEET_PATH));
    let languages = configured_languages(&src_dir, &config)?;
    let (section_plans, mut typ_files) = discover_site_pages(
        &src_dir,
        config.sidebar.as_ref(),
        config.pages.as_ref(),
        &languages,
    )?;
    let (navbar_plan, mut navbar_files) = discover_site_navbar(
        &src_dir,
        config.navbar.as_ref(),
        config.pages.as_ref(),
        &languages,
    )?;
    typ_files.append(&mut navbar_files);
    let mut included_pages =
        discover_site_build_pages(&src_dir, config.pages.as_ref(), &languages)?;
    typ_files.append(&mut included_pages);
    typ_files.extend(implicit_build_pages(&src_dir, &languages));
    typ_files.sort_by_key(|path| rel_posix(&src_dir, path));
    typ_files.dedup();
    let fallback_files = fallback_pages(&src_dir, &languages);
    let page_fingerprints = fingerprint_files(&typ_files)?;

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
        cli_theme: cli_theme.clone(),
        fallback_theme: config_theme.clone(),
        parallelism: args.parallelism,
    })?;
    if theme_stylesheet_path.is_none()
        && preprocessed.values().any(|output| {
            crate::theme::resolve_html_entry(&output.theme, crate::theme::HtmlScope::Site)
                .ok()
                .flatten()
                .is_some_and(|entry| entry.is_default)
        })
    {
        theme_stylesheet_path = Some(PathBuf::from(WEBSITE_STYLESHEET_PATH));
    }

    let page_meta = load_page_meta(&src_dir, &typ_files);
    let metadata = SiteMetadata::from_config(&config, &src_dir)?;
    let default_favicon_path = if clean_optional_string(config.favicon.as_deref()).is_none() {
        Some(PathBuf::from(DEFAULT_FAVICON_PATH))
    } else {
        None
    };
    let sitemap_path = metadata
        .base_url
        .as_ref()
        .map(|_| out_dir.join("sitemap.xml"));
    let pdf_files = pdf_enabled_files(&typ_files, &page_meta, args.render_pdf, config.pdf);
    let page_info = build_page_info(&src_dir, &typ_files, &page_meta, &pdf_files, &languages)?;
    let mut icon_cache = IconCache::new(src_dir.join(ICON_CACHE_DIR));
    let sidebar_sections = nav_from_plans(&section_plans, &page_meta, &page_info, &mut icon_cache)?;
    let navbar = navbar_from_plan(&navbar_plan, &page_meta, &page_info, &mut icon_cache)?;
    let nav_signature = navigation_signature(&sidebar_sections) ^ navbar_signature(&navbar);
    let pages_index =
        build_pages_index(&src_dir, &typ_files, &section_plans, &page_meta, &page_info);
    let pages_index_json = serde_json::to_string_pretty(&pages_index)?;
    let pages_signature = xxh3_64(pages_index_json.as_bytes());
    write_pages_index(&typ_files, &pages_index_json)?;
    let expected_outputs = expected_generated_outputs(
        &out_dir,
        &typ_files,
        &page_info,
        &sitemap_path,
        theme_stylesheet_path.as_deref(),
        default_favicon_path.as_deref(),
    );
    let previous_manifest = load_manifest(&out_dir)?;

    if args.clean {
        clear_previous_outputs(&src_dir, &out_dir)?;
    }
    reconcile_manifest_outputs(&out_dir, &previous_manifest, &expected_outputs)?;
    if args.clean && out_dir != src_dir {
        remove_unexpected_rendered_outputs(&out_dir, &expected_outputs)?;
    }
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;
    if let Some(path) = theme_stylesheet_path.as_deref() {
        let entry = crate::theme::resolve_html_entry(
            &crate::theme::ThemeSelection::Default,
            crate::theme::HtmlScope::Site,
        )?
        .expect("default theme must provide a site entry");
        write_html_theme_stylesheet(&entry, &out_dir, path)?;
    }
    if let Some(path) = default_favicon_path.as_deref() {
        write_default_favicon(&out_dir, path)?;
    }

    if out_dir != src_dir {
        if args.incremental_inputs.is_none() {
            copy_assets(&src_dir, &out_dir)?;
        }
        let source_files = if args.incremental_inputs.is_some() {
            build_set.clone()
        } else if config.sidebar.is_some() {
            typ_files.clone()
        } else {
            iter_typ_files(&src_dir, false, &[])?
        };
        copy_typ_sources(&src_dir, &out_dir, &source_files)?;
    }

    // Phase 2: render the preprocessed pages with full site context.
    let site = SiteModel::new(
        sidebar_sections,
        navbar,
        metadata.clone(),
        config.sidebar.as_ref().is_none_or(|sidebar| sidebar.fold),
    );
    render_documents(
        &BuildContext {
            out_dir: out_dir.clone(),
            typst: calepin_config.executables.typst,
            pdf_files,
            page_info: page_info.clone(),
            languages: languages.clone(),
            theme_stylesheet: theme_stylesheet_path.map(|path| slash_path(&path)),
            parallelism: args.parallelism,
            typst_args: args.typst_args,
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
    write_sitemap(&out_dir, metadata.base_url.as_deref(), &sitemap_hrefs)?;
    write_manifest(&out_dir, &expected_outputs)?;
    Ok(WebsiteBuildResult {
        src_dir,
        out_dir,
        config_path,
        theme_dir,
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

    let (tx, rx) = mpsc::channel();
    let mut debouncer = new_debouncer(Duration::from_millis(350), None, tx)
        .context("failed to create file watcher")?;
    debouncer
        .watch(&current.src_dir, RecursiveMode::Recursive)
        .with_context(|| format!("failed to watch {}", current.src_dir.display()))?;
    debouncer
        .watch(&current.config_path, RecursiveMode::NonRecursive)
        .with_context(|| format!("failed to watch {}", current.config_path.display()))?;
    if let Some(theme_dir) = current.theme_dir.as_deref() {
        debouncer
            .watch(theme_dir, RecursiveMode::Recursive)
            .with_context(|| format!("failed to watch {}", theme_dir.display()))?;
    }

    if !quiet {
        eprintln!("Watching {}", current.src_dir.display());
        eprintln!("Press Ctrl+C to stop.");
    }

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(events)) => {
                let mut changed = Vec::new();
                for event in events {
                    if !is_rebuild_event(&event.event.kind) {
                        continue;
                    }
                    for path in event.event.paths {
                        let path = path.canonicalize().unwrap_or(path);
                        if should_rebuild_for_path(&current, &path) && !changed.contains(&path) {
                            changed.push(path);
                        }
                    }
                }
                if changed.is_empty() {
                    continue;
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
                        current = next;
                        live.rebuilt();
                    }
                    Ok(None) => {}
                    Err(error) => {
                        cwarn!("website rebuild failed: {}", error);
                        live.set_error(format!("{error:#}"));
                    }
                }
            }
            Ok(Err(errors)) => {
                for error in errors {
                    cwarn!("watch error: {}", error);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}

fn is_rebuild_event(kind: &notify::EventKind) -> bool {
    matches!(
        kind,
        notify::EventKind::Create(_)
            | notify::EventKind::Modify(notify::event::ModifyKind::Data(_))
            | notify::EventKind::Modify(notify::event::ModifyKind::Name(_))
            | notify::EventKind::Modify(notify::event::ModifyKind::Any)
            | notify::EventKind::Remove(_)
    )
}

fn should_rebuild_for_path(initial: &WebsiteBuildResult, path: &Path) -> bool {
    if path == initial.config_path {
        return true;
    }
    if let Some(theme_dir) = initial.theme_dir.as_deref() {
        if path.starts_with(theme_dir) {
            return true;
        }
    }
    // A distinct output directory only ever receives generated copies; reacting
    // to them would re-trigger the build that produced them.
    if initial.out_dir != initial.src_dir && path.starts_with(&initial.out_dir) {
        return false;
    }
    if !path.starts_with(&initial.src_dir) {
        return false;
    }
    let rel = path.strip_prefix(&initial.src_dir).unwrap_or(path);
    if rel.components().next().is_none() {
        return false;
    }
    if rel.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|name| SKIP_DIRS.contains(&name))
    }) {
        return false;
    }
    if path.starts_with(&initial.out_dir) {
        if let Some("html" | "pdf") = path.extension().and_then(|extension| extension.to_str()) {
            return false;
        }
    }
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some(
            "typ"
                | "toml"
                | "css"
                | "js"
                | "svg"
                | "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "webp"
                | "ico"
                | "mp4"
        )
    )
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

fn navbar_signature(navbar: &NavbarModel) -> u64 {
    let mut bytes = Vec::new();
    for items in [&navbar.left, &navbar.center, &navbar.right] {
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

#[derive(Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct WebsiteConfig {
    default_language: Option<String>,
    languages: BTreeMap<String, LanguageConfig>,
    #[serde(rename = "executables")]
    _executables: Option<toml::Value>,
    theme: Option<RawThemeValue>,
    title: Option<String>,
    description: Option<String>,
    base_url: Option<String>,
    logo: Option<String>,
    logo_alt: Option<String>,
    favicon: Option<String>,
    /// Also render a PDF for every page; pages can override with `pdf` in
    /// their `<website-metadata>`.
    pdf: Option<bool>,
    pages: Option<PagesConfig>,
    navbar: Option<NavbarConfig>,
    sidebar: Option<SidebarConfig>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
enum RawThemeValue {
    Enabled(String),
    Toggle(bool),
}

impl WebsiteConfig {
    fn theme_selection(&self, config_dir: &Path) -> Result<crate::theme::ThemeSelection> {
        match &self.theme {
            None => Ok(crate::theme::ThemeSelection::Default),
            Some(RawThemeValue::Toggle(false)) => Ok(crate::theme::ThemeSelection::Disabled),
            Some(RawThemeValue::Toggle(true)) => Ok(crate::theme::ThemeSelection::Default),
            Some(RawThemeValue::Enabled(value)) => {
                crate::theme::ThemeSelection::parse(value, config_dir)
            }
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct LanguageConfig {
    label: Option<String>,
    content_dir: Option<PathBuf>,
    url_prefix: Option<String>,
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

fn configured_languages(
    src_dir: &Path,
    config: &WebsiteConfig,
) -> Result<Option<Vec<LanguageInfo>>> {
    if config.languages.is_empty() {
        return Ok(None);
    }
    let default_language = match config.default_language.as_deref() {
        Some(default_language) => default_language,
        None if config.languages.len() == 1 => {
            config.languages.keys().next().map(String::as_str).unwrap()
        }
        None => {
            return Err(anyhow!(
                "set default_language when more than one language is configured in [languages]"
            ))
        }
    };
    if !config.languages.contains_key(default_language) {
        return Err(anyhow!(
            "default_language `{default_language}` is not present in [languages]"
        ));
    }

    let mut languages = Vec::new();
    for (code, language) in &config.languages {
        let default = code == default_language;
        let content_dir = language.content_dir.clone().unwrap_or_else(|| {
            if default {
                PathBuf::from(".")
            } else {
                PathBuf::from(code)
            }
        });
        let content_dir = if content_dir == Path::new(".") {
            src_dir.to_path_buf()
        } else if content_dir.is_absolute() {
            content_dir
        } else {
            src_dir.join(content_dir)
        };
        let url_prefix = clean_optional_string(language.url_prefix.as_deref())
            .unwrap_or_else(|| if default { String::new() } else { code.clone() });
        let url_prefix = clean_url_prefix(&url_prefix);
        if !is_safe_output_route(&url_prefix) {
            bail!("url_prefix for language `{code}` must stay inside the output directory: `{url_prefix}`");
        }
        languages.push(LanguageInfo {
            code: code.clone(),
            label: clean_optional_string(language.label.as_deref()).unwrap_or_else(|| code.clone()),
            content_dir,
            url_prefix,
            default,
        });
    }
    languages.sort_by_key(|language| (!language.default, language.code.clone()));
    Ok(Some(languages))
}

fn clean_url_prefix(value: &str) -> String {
    value
        .trim()
        .trim_matches('/')
        .trim_start_matches("./")
        .to_string()
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct SidebarConfig {
    show_hidden: bool,
    fold: bool,
    section: Vec<SidebarSectionConfig>,
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

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct SidebarSectionConfig {
    title: Option<String>,
    item: Vec<SidebarItemConfig>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct SidebarItemConfig {
    #[serde(alias = "path", alias = "url")]
    target: Option<String>,
    glob: Option<String>,
    label: Option<String>,
    icon: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct PagesConfig {
    include: Vec<String>,
    exclude: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct NavbarConfig {
    show_hidden: bool,
    item: Vec<NavbarItemConfig>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct NavbarItemConfig {
    position: NavbarPosition,
    #[serde(alias = "path", alias = "url")]
    target: Option<String>,
    glob: Option<String>,
    label: Option<String>,
    icon: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum NavbarPosition {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone)]
struct NavSectionModel {
    language: Option<String>,
    title: Option<String>,
    items: Vec<NavItemModel>,
}

#[derive(Debug, Clone)]
struct NavItemModel {
    language: Option<String>,
    href: String,
    label: String,
    label_html: String,
}

#[derive(Debug, Clone, Default)]
struct NavbarModel {
    left: Vec<NavItemModel>,
    center: Vec<NavItemModel>,
    right: Vec<NavItemModel>,
}

impl NavbarModel {
    fn entries_for_current_page(
        &self,
        current_href: &str,
        items: &[NavItemModel],
        current_language: Option<&str>,
    ) -> Vec<SiteNavEntry> {
        items
            .iter()
            .filter(|item| {
                item.language
                    .as_deref()
                    .is_none_or(|language| Some(language) == current_language)
            })
            .map(|item| SiteNavEntry {
                href: html_escape(&page_relative_url(current_href, &item.href)),
                label: html_escape(&item.label),
                label_html: item.label_html.clone(),
                active: item.href == current_href,
            })
            .collect()
    }
}

#[derive(Debug, Clone, Default)]
struct SiteMetadata {
    title: Option<String>,
    description: Option<String>,
    base_url: Option<String>,
    logo: Option<String>,
    logo_alt: Option<String>,
    favicon: Option<String>,
}

impl SiteMetadata {
    fn from_config(config: &WebsiteConfig, src_dir: &Path) -> Result<Self> {
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
struct SiteModel {
    sections: Vec<NavSectionModel>,
    navbar: NavbarModel,
    metadata: SiteMetadata,
    sidebar_fold: bool,
}

impl SiteModel {
    fn new(
        sections: Vec<NavSectionModel>,
        navbar: NavbarModel,
        metadata: SiteMetadata,
        sidebar_fold: bool,
    ) -> Self {
        Self {
            sections,
            navbar,
            metadata,
            sidebar_fold,
        }
    }

    fn theme_context(
        &self,
        current_href: &str,
        page_info: Option<&PageInfo>,
        page_info_map: &PageInfoMap,
        languages: Option<&[LanguageInfo]>,
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

        SiteContextInput {
            sidebar,
            sidebar_sections,
            sidebar_fold: self.sidebar_fold,
            navbar_left: self.navbar.entries_for_current_page(
                current_href,
                &self.navbar.left,
                current_language,
            ),
            navbar_center: self.navbar.entries_for_current_page(
                current_href,
                &self.navbar.center,
                current_language,
            ),
            navbar_right: self.navbar.entries_for_current_page(
                current_href,
                &self.navbar.right,
                current_language,
            ),
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
        }
    }
}

fn language_entries(
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

fn translation_entries(
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

#[derive(Debug, Deserialize, Serialize, Default)]
struct WebsiteManifest {
    outputs: Vec<String>,
}

#[derive(Clone)]
struct BuildContext {
    out_dir: PathBuf,
    typst: PathBuf,
    pdf_files: BTreeSet<PathBuf>,
    page_info: PageInfoMap,
    languages: Option<Vec<LanguageInfo>>,
    theme_stylesheet: Option<String>,
    parallelism: Option<usize>,
    typst_args: Vec<String>,
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
        return absolutize_from(current_dir, config);
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
    absolutize_from(current_dir, path)
}

fn resolve_cli_path(current_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        current_dir.join(path)
    }
}

/// Plan for one sidebar entry: the page and its explicitly configured label,
/// resolved before metadata is available.
#[derive(Debug, Clone)]
struct NavItemPlan {
    path: Option<PathBuf>,
    url: Option<String>,
    configured_label: Option<String>,
    icon: Option<String>,
}

#[derive(Debug, Clone)]
struct NavSectionPlan {
    language: Option<String>,
    title: Option<String>,
    items: Vec<NavItemPlan>,
}

#[derive(Debug, Clone, Default)]
struct NavbarPlan {
    left: Vec<NavbarItemPlan>,
    center: Vec<NavbarItemPlan>,
    right: Vec<NavbarItemPlan>,
}

#[derive(Debug, Clone)]
struct NavbarItemPlan {
    path: Option<PathBuf>,
    url: Option<String>,
    configured_label: Option<String>,
    icon: Option<String>,
}

fn discover_site_pages(
    src_dir: &Path,
    sidebar: Option<&SidebarConfig>,
    pages: Option<&PagesConfig>,
    languages: &Option<Vec<LanguageInfo>>,
) -> Result<(Vec<NavSectionPlan>, Vec<PathBuf>)> {
    let Some(languages) = languages else {
        return discover_pages(src_dir, sidebar, pages, None);
    };
    let mut sections = Vec::new();
    let mut files = Vec::new();
    for language in languages {
        let (mut language_sections, mut language_files) = discover_pages(
            &language.content_dir,
            sidebar,
            pages,
            Some(language.code.clone()),
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

fn discover_site_navbar(
    src_dir: &Path,
    navbar: Option<&NavbarConfig>,
    pages: Option<&PagesConfig>,
    languages: &Option<Vec<LanguageInfo>>,
) -> Result<(NavbarPlan, Vec<PathBuf>)> {
    let Some(navbar) = navbar else {
        return Ok((default_navbar_plan(), Vec::new()));
    };
    let Some(languages) = languages else {
        return discover_navbar(src_dir, navbar, pages);
    };
    let mut plan = NavbarPlan::default();
    let mut files = Vec::new();
    for language in languages {
        let (mut language_plan, mut language_files) =
            discover_navbar(&language.content_dir, navbar, pages)?;
        language_files.retain(|path| !is_nested_language_page(path, language, languages));
        retain_navbar_language_items(&mut language_plan, language, languages);
        if !language.default {
            retain_language_specific_navbar_items(&mut language_plan);
        }
        plan.left.append(&mut language_plan.left);
        plan.center.append(&mut language_plan.center);
        plan.right.append(&mut language_plan.right);
        files.append(&mut language_files);
    }
    Ok((plan, files))
}

fn default_navbar_plan() -> NavbarPlan {
    NavbarPlan::default()
}

fn retain_navbar_language_items(
    plan: &mut NavbarPlan,
    current: &LanguageInfo,
    languages: &[LanguageInfo],
) {
    for items in [&mut plan.left, &mut plan.center, &mut plan.right] {
        items.retain(|item| {
            item.path
                .as_ref()
                .is_none_or(|path| !is_nested_language_page(path, current, languages))
        });
    }
}

fn retain_language_specific_navbar_items(plan: &mut NavbarPlan) {
    for items in [&mut plan.left, &mut plan.center, &mut plan.right] {
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

fn discover_pages(
    src_dir: &Path,
    sidebar: Option<&SidebarConfig>,
    pages: Option<&PagesConfig>,
    language: Option<String>,
) -> Result<(Vec<NavSectionPlan>, Vec<PathBuf>)> {
    let Some(sidebar) = sidebar else {
        let mut files = iter_typ_files(src_dir, false, &[PathBuf::from(FALLBACK_PAGE)])?;
        files.retain(|path| !page_is_excluded(src_dir, path, pages));
        let items = files
            .iter()
            .map(|path| NavItemPlan {
                path: Some(path.clone()),
                url: None,
                configured_label: None,
                icon: None,
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
    )?;
    let all_typ_files = all_typ_files
        .into_iter()
        .filter(|path| !page_is_excluded(src_dir, path, pages))
        .collect::<Vec<_>>();
    let mut used = BTreeSet::new();
    let mut sections = Vec::new();
    let mut build_files = Vec::new();

    for section_config in &sidebar.section {
        let mut items = Vec::new();
        for item_config in &section_config.item {
            let configured_label = clean_optional_string(item_config.label.as_deref());
            let icon = clean_optional_string(item_config.icon.as_deref());
            if let Some(target) = item_config
                .target
                .as_deref()
                .map(str::trim)
                .filter(|target| !target.is_empty())
            {
                if item_config
                    .glob
                    .as_deref()
                    .is_some_and(|glob| !glob.trim().is_empty())
                {
                    bail!("sidebar target items cannot also set glob");
                }
                match resolve_nav_target("sidebar", src_dir, target) {
                    Some(NavTarget::Url(url)) => {
                        items.push(NavItemPlan {
                            path: None,
                            url: Some(url),
                            configured_label,
                            icon,
                        });
                    }
                    Some(NavTarget::Page(path)) => {
                        if used.insert(path.clone()) {
                            items.push(NavItemPlan {
                                path: Some(path.clone()),
                                url: None,
                                configured_label,
                                icon,
                            });
                            build_files.push(path);
                        }
                    }
                    None => {}
                }
                continue;
            }
            let candidates = resolve_file_list(
                "sidebar",
                src_dir,
                None,
                item_config.glob.as_deref(),
                &all_typ_files,
            )?;
            let candidates = candidates
                .into_iter()
                .filter(|path| !page_is_excluded(src_dir, path, pages))
                .collect::<Vec<_>>();
            for path in candidates {
                if !used.insert(path.clone()) {
                    continue;
                }
                items.push(NavItemPlan {
                    path: Some(path.clone()),
                    url: None,
                    configured_label: configured_label.clone(),
                    icon: icon.clone(),
                });
                build_files.push(path);
            }
        }
        sections.push(NavSectionPlan {
            language: language.clone(),
            title: section_config.title.clone(),
            items,
        });
    }

    Ok((sections, build_files))
}

fn discover_navbar(
    src_dir: &Path,
    navbar: &NavbarConfig,
    pages: Option<&PagesConfig>,
) -> Result<(NavbarPlan, Vec<PathBuf>)> {
    let all_typ_files =
        iter_typ_files(src_dir, navbar.show_hidden, &[PathBuf::from(FALLBACK_PAGE)])?;
    let all_typ_files = all_typ_files
        .into_iter()
        .filter(|path| !page_is_excluded(src_dir, path, pages))
        .collect::<Vec<_>>();
    let mut files = Vec::new();
    let mut used = BTreeSet::new();

    let mut resolve = |items: &[&NavbarItemConfig]| -> Result<Vec<NavbarItemPlan>> {
        let mut out = Vec::new();
        for item in items {
            let configured_label = clean_optional_string(item.label.as_deref());
            let icon = clean_optional_string(item.icon.as_deref());
            if let Some(target) = item
                .target
                .as_deref()
                .map(str::trim)
                .filter(|target| !target.is_empty())
            {
                if item
                    .glob
                    .as_deref()
                    .is_some_and(|glob| !glob.trim().is_empty())
                {
                    bail!("navbar target items cannot also set glob");
                }
                match resolve_nav_target("navbar", src_dir, target) {
                    Some(NavTarget::Url(url)) => {
                        out.push(NavbarItemPlan {
                            path: None,
                            url: Some(url),
                            configured_label: configured_label.clone(),
                            icon: icon.clone(),
                        });
                    }
                    Some(NavTarget::Page(path)) => {
                        if used.insert(path.clone()) {
                            files.push(path.clone());
                        }
                        out.push(NavbarItemPlan {
                            path: Some(path),
                            url: None,
                            configured_label: configured_label.clone(),
                            icon: icon.clone(),
                        });
                    }
                    None => {}
                }
                continue;
            }
            for path in resolve_file_list(
                "navbar",
                src_dir,
                None,
                item.glob.as_deref(),
                &all_typ_files,
            )?
            .into_iter()
            .filter(|path| !page_is_excluded(src_dir, path, pages))
            {
                if used.insert(path.clone()) {
                    files.push(path.clone());
                }
                out.push(NavbarItemPlan {
                    path: Some(path),
                    url: None,
                    configured_label: configured_label.clone(),
                    icon: icon.clone(),
                });
            }
        }
        Ok(out)
    };
    let left = navbar
        .item
        .iter()
        .filter(|item| item.position == NavbarPosition::Left)
        .collect::<Vec<_>>();
    let center = navbar
        .item
        .iter()
        .filter(|item| item.position == NavbarPosition::Center)
        .collect::<Vec<_>>();
    let right = navbar
        .item
        .iter()
        .filter(|item| item.position == NavbarPosition::Right)
        .collect::<Vec<_>>();

    Ok((
        NavbarPlan {
            left: resolve(&left)?,
            center: resolve(&center)?,
            right: resolve(&right)?,
        },
        files,
    ))
}

fn fallback_pages(src_dir: &Path, languages: &Option<Vec<LanguageInfo>>) -> Vec<PathBuf> {
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

fn implicit_build_pages(src_dir: &Path, languages: &Option<Vec<LanguageInfo>>) -> Vec<PathBuf> {
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

fn discover_site_build_pages(
    src_dir: &Path,
    pages: Option<&PagesConfig>,
    languages: &Option<Vec<LanguageInfo>>,
) -> Result<Vec<PathBuf>> {
    let Some(languages) = languages else {
        return discover_build_pages(src_dir, pages);
    };
    let mut files = Vec::new();
    for language in languages {
        let mut language_files = discover_build_pages(&language.content_dir, pages)?;
        language_files.retain(|path| !is_nested_language_page(path, language, languages));
        files.append(&mut language_files);
    }
    Ok(files)
}

fn discover_build_pages(src_dir: &Path, pages: Option<&PagesConfig>) -> Result<Vec<PathBuf>> {
    let Some(pages) = pages else {
        return Ok(Vec::new());
    };
    let all_typ_files = iter_typ_files(src_dir, true, &[PathBuf::from(FALLBACK_PAGE)])?;
    let mut files = BTreeSet::new();
    for pattern in page_patterns(&pages.include) {
        let matches = if has_glob_chars(&pattern) {
            all_typ_files
                .iter()
                .filter(|path| wildcard_match(&pattern, &rel_posix(src_dir, path)))
                .cloned()
                .collect::<Vec<_>>()
        } else {
            let candidate = src_dir.join(Path::new(&pattern));
            if candidate.is_file()
                && candidate.extension().and_then(|ext| ext.to_str()) == Some("typ")
            {
                vec![candidate]
            } else {
                cwarn!(
                    "pages.include path does not exist or is not a .typ file: {}",
                    pattern
                );
                Vec::new()
            }
        };
        for path in matches {
            if !page_is_excluded(src_dir, &path, Some(pages)) {
                files.insert(path);
            }
        }
    }
    Ok(files.into_iter().collect())
}

fn page_is_excluded(src_dir: &Path, path: &Path, pages: Option<&PagesConfig>) -> bool {
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

fn build_page_info(
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
        out.insert(
            path.clone(),
            PageInfo {
                language: language.map(|language| language.code.clone()),
                translation_key,
                href,
                pdf_href,
            },
        );
    }
    Ok(out)
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
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
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

fn output_href_with_extension(url: &str, extension: &str) -> String {
    let url = url.trim().trim_start_matches('/').trim_start_matches("./");
    if url.ends_with('/') {
        return format!("{url}index.{extension}");
    }
    // Replace any author-provided extension so the HTML and PDF outputs of the
    // same page never collide on one path.
    let mut path = PathBuf::from(url);
    path.set_extension(extension);
    slash_path(&path)
}

/// True when `value` is a relative route that cannot escape the output
/// directory once joined onto it.
fn is_safe_output_route(value: &str) -> bool {
    let path = Path::new(value);
    !path.is_absolute()
        && path.components().all(|component| {
            matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
}

struct IconCache {
    dir: PathBuf,
    agent: ureq::Agent,
    /// Icons that already failed this build; avoids repeating slow download
    /// attempts (and duplicate warnings) for every nav item that uses them.
    unavailable: BTreeSet<String>,
}

impl IconCache {
    fn new(dir: PathBuf) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(ICON_DOWNLOAD_TIMEOUT_SECS))
            .build();
        Self {
            dir,
            agent,
            unavailable: BTreeSet::new(),
        }
    }

    /// Returns the icon's inline SVG, or `None` (after a warning) when it
    /// cannot be fetched or is unsafe to inline. Only a malformed icon spec is
    /// an error: a missing network must not fail the whole site build.
    fn resolve(&mut self, spec: Option<&str>) -> Result<Option<String>> {
        let Some(spec) = spec else {
            return Ok(None);
        };
        let icon = parse_icon_spec(spec)?;
        if self.unavailable.contains(spec) {
            return Ok(None);
        }
        let path = self
            .dir
            .join(&icon.prefix)
            .join(format!("{}.svg", icon.name));
        if path.is_file() {
            let svg = fs::read_to_string(&path)
                .with_context(|| format!("failed to read cached icon {}", path.display()))?;
            return Ok(self.sanitized_or_warn(&svg, spec));
        }

        fs::create_dir_all(path.parent().unwrap())
            .with_context(|| format!("failed to create icon cache {}", self.dir.display()))?;
        let url = format!(
            "https://api.iconify.design/{}/{}.svg",
            icon.prefix, icon.name
        );
        let svg = match self.agent.get(&url).call() {
            Ok(response) => match response.into_string() {
                Ok(svg) => svg,
                Err(error) => {
                    cwarn!("failed to read downloaded icon `{spec}`: {error}");
                    self.unavailable.insert(spec.to_string());
                    return Ok(None);
                }
            },
            Err(error) => {
                cwarn!("failed to download icon `{spec}` from {url}: {error}");
                self.unavailable.insert(spec.to_string());
                return Ok(None);
            }
        };
        let Some(svg) = self.sanitized_or_warn(&svg, spec) else {
            return Ok(None);
        };
        fs::write(&path, &svg)
            .with_context(|| format!("failed to cache icon `{spec}` at {}", path.display()))?;
        Ok(Some(svg))
    }

    fn sanitized_or_warn(&mut self, svg: &str, spec: &str) -> Option<String> {
        match sanitize_icon_svg(svg, spec) {
            Ok(svg) => Some(svg),
            Err(error) => {
                cwarn!("{error}");
                self.unavailable.insert(spec.to_string());
                None
            }
        }
    }
}

struct IconSpec {
    prefix: String,
    name: String,
}

fn parse_icon_spec(value: &str) -> Result<IconSpec> {
    let value = value.trim();
    let (prefix, name) = value
        .split_once(':')
        .map(|(prefix, name)| (prefix.trim(), name.trim()))
        .unwrap_or((DEFAULT_ICON_PREFIX, value));
    if !valid_icon_component(prefix) || !valid_icon_component(name) {
        return Err(anyhow!(
            "invalid icon `{value}`; use `name` or `prefix:name` with lowercase letters, digits, and hyphens"
        ));
    }
    Ok(IconSpec {
        prefix: prefix.to_string(),
        name: name.to_string(),
    })
}

fn valid_icon_component(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn sanitize_icon_svg(svg: &str, spec: &str) -> Result<String> {
    let svg = svg.trim();
    let lower = svg.to_ascii_lowercase();
    if !lower.starts_with("<svg")
        || !lower.contains("</svg>")
        || lower.contains("<script")
        || lower.contains("<foreignobject")
        || lower.contains("javascript:")
        || contains_event_handler_attribute(&lower)
    {
        return Err(anyhow!("downloaded icon `{spec}` is not a safe inline SVG"));
    }
    Ok(svg.to_string())
}

/// Detects `on...=` event-handler attributes (`onload=`, `onclick=`, ...) in
/// lowercased SVG source. May reject rare benign content (e.g. text mentioning
/// `on x=`); for icons that trade-off is fine.
fn contains_event_handler_attribute(lower: &str) -> bool {
    lower.match_indices("on").any(|(index, _)| {
        let preceded_by_whitespace = lower[..index]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_ascii_whitespace());
        if !preceded_by_whitespace {
            return false;
        }
        let rest = &lower[index + "on".len()..];
        let name_len = rest.chars().take_while(char::is_ascii_alphanumeric).count();
        name_len > 0 && rest[name_len..].trim_start().starts_with('=')
    })
}

fn nav_label_html(
    label: &str,
    fallback_icon: Option<&str>,
    icon_cache: &mut IconCache,
) -> Result<String> {
    let mut html = String::new();
    if let Some(icon) = fallback_icon {
        if !label_contains_icon_token(label) {
            push_nav_icon(&mut html, icon, icon_cache)?;
        }
    }
    let mut rest = label;
    while let Some(start) = rest.find("{icon:") {
        html.push_str(&html_escape(&rest[..start]));
        let after_start = &rest[start + "{icon:".len()..];
        let Some(end) = after_start.find('}') else {
            html.push_str(&html_escape(&rest[start..]));
            return Ok(html);
        };
        let icon = after_start[..end].trim();
        push_nav_icon(&mut html, icon, icon_cache)?;
        rest = &after_start[end + 1..];
    }
    html.push_str(&html_escape(rest));
    Ok(html)
}

fn label_contains_icon_token(label: &str) -> bool {
    label.contains("{icon:")
}

fn push_nav_icon(html: &mut String, icon: &str, icon_cache: &mut IconCache) -> Result<()> {
    if let Some(svg) = icon_cache.resolve(Some(icon))? {
        html.push_str(r#"<span class="calepin-nav-icon">"#);
        html.push_str(&svg);
        html.push_str("</span>");
    }
    Ok(())
}

fn accessible_nav_label(label: &str, fallback: &str) -> String {
    let stripped = strip_icon_tokens(label).trim().to_string();
    if stripped.is_empty() {
        fallback.to_string()
    } else {
        stripped
    }
}

fn strip_icon_tokens(label: &str) -> String {
    let mut out = String::new();
    let mut rest = label;
    while let Some(start) = rest.find("{icon:") {
        out.push_str(&rest[..start]);
        let after_start = &rest[start + "{icon:".len()..];
        let Some(end) = after_start.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        rest = &after_start[end + 1..];
    }
    out.push_str(rest);
    out
}

fn nav_from_plans(
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
                    if let Some(url) = item.url.as_ref() {
                        let raw_label =
                            item.configured_label.clone().unwrap_or_else(|| url.clone());
                        let label_html =
                            nav_label_html(&raw_label, item.icon.as_deref(), icon_cache)?;
                        return Ok(NavItemModel {
                            language: None,
                            href: url.clone(),
                            label: accessible_nav_label(&raw_label, url),
                            label_html,
                        });
                    }
                    let Some(path) = item.path.as_ref() else {
                        return Err(anyhow!("sidebar item must set path, glob, or url"));
                    };
                    let raw_label = item
                        .configured_label
                        .clone()
                        .or_else(|| page_meta.get(path).and_then(|meta| meta.title.clone()))
                        .unwrap_or_else(|| stem_label(path));
                    let fallback = page_meta
                        .get(path)
                        .and_then(|meta| meta.title.clone())
                        .unwrap_or_else(|| stem_label(path));
                    let label = accessible_nav_label(&raw_label, &fallback);
                    let label_html = nav_label_html(&raw_label, item.icon.as_deref(), icon_cache)?;
                    Ok(NavItemModel {
                        language: section.language.clone(),
                        href: page_info
                            .get(path)
                            .map(|info| info.href.clone())
                            .unwrap_or_default(),
                        label,
                        label_html,
                    })
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

fn navbar_from_plan(
    plan: &NavbarPlan,
    page_meta: &PageMetaMap,
    page_info: &PageInfoMap,
    icon_cache: &mut IconCache,
) -> Result<NavbarModel> {
    Ok(NavbarModel {
        left: navbar_items_from_plan(&plan.left, page_meta, page_info, icon_cache)?,
        center: navbar_items_from_plan(&plan.center, page_meta, page_info, icon_cache)?,
        right: navbar_items_from_plan(&plan.right, page_meta, page_info, icon_cache)?,
    })
}

fn navbar_items_from_plan(
    items: &[NavbarItemPlan],
    page_meta: &PageMetaMap,
    page_info: &PageInfoMap,
    icon_cache: &mut IconCache,
) -> Result<Vec<NavItemModel>> {
    items
        .iter()
        .filter_map(|item| {
            if let Some(path) = &item.path {
                let raw_label = item
                    .configured_label
                    .clone()
                    .or_else(|| page_meta.get(path).and_then(|meta| meta.title.clone()))
                    .unwrap_or_else(|| stem_label(path));
                let fallback = page_meta
                    .get(path)
                    .and_then(|meta| meta.title.clone())
                    .unwrap_or_else(|| stem_label(path));
                return Some(
                    nav_label_html(&raw_label, item.icon.as_deref(), icon_cache).map(
                        |label_html| NavItemModel {
                            language: page_info.get(path).and_then(|info| info.language.clone()),
                            href: page_info
                                .get(path)
                                .map(|info| info.href.clone())
                                .unwrap_or_default(),
                            label: accessible_nav_label(&raw_label, &fallback),
                            label_html,
                        },
                    ),
                );
            }
            item.url.as_ref().map(|url| {
                let raw_label = item.configured_label.clone().unwrap_or_else(|| url.clone());
                nav_label_html(&raw_label, item.icon.as_deref(), icon_cache).map(|label_html| {
                    NavItemModel {
                        language: None,
                        href: url.clone(),
                        label: accessible_nav_label(&raw_label, url),
                        label_html,
                    }
                })
            })
        })
        .collect()
}

/// Builds the site-wide pages index consumed by `calepin.pages()` in the
/// Typst runtime: one entry per built page (the 404 page excluded), with
/// resolved `title`/`pdf` and the raw author metadata under `meta`.
fn build_pages_index(
    src_dir: &Path,
    typ_files: &[PathBuf],
    sections: &[NavSectionPlan],
    page_meta: &PageMetaMap,
    page_info: &PageInfoMap,
) -> serde_json::Value {
    let configured_labels = sections
        .iter()
        .flat_map(|section| section.items.iter())
        .filter_map(|item| item.path.as_ref().zip(item.configured_label.as_deref()))
        .collect::<BTreeMap<_, _>>();
    let entries = typ_files
        .iter()
        .filter(|path| path.file_name().and_then(|name| name.to_str()) != Some(FALLBACK_PAGE))
        .map(|path| {
            let meta = page_meta.get(path);
            let title = configured_labels
                .get(path)
                .map(|label| label.to_string())
                .or_else(|| meta.and_then(|meta| meta.title.clone()))
                .unwrap_or_else(|| stem_label(path));
            let raw = meta
                .map(|meta| meta.raw.clone())
                .filter(serde_json::Value::is_object)
                .unwrap_or_else(|| serde_json::json!({}));
            serde_json::json!({
                "path": rel_posix(src_dir, path),
                "href": page_info.get(path).map(|info| info.href.clone()).unwrap_or_default(),
                "title": title,
                "language": page_info.get(path).and_then(|info| info.language.clone()),
                "translation_key": page_info.get(path).map(|info| info.translation_key.clone()).unwrap_or_default(),
                "translations": page_translations_json(path, page_info),
                "pdf": page_info.get(path).and_then(|info| info.pdf_href.clone()),
                "meta": raw,
            })
        })
        .collect::<Vec<_>>();
    serde_json::Value::Array(entries)
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

/// Writes the pages index into every page directory's `.calepin`, so the
/// constant root-relative `PAGES_INDEX_REF` resolves for each page's typst
/// root (the page's own directory).
fn write_pages_index(typ_files: &[PathBuf], index_json: &str) -> Result<()> {
    let dirs = typ_files
        .iter()
        .filter_map(|path| path.parent())
        .collect::<BTreeSet<_>>();
    for dir in dirs {
        let target_dir = dir.join(".calepin");
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

fn resolve_nav_target(context: &str, src_dir: &Path, target: &str) -> Option<NavTarget> {
    let target = target.trim();
    if is_absolute_or_special_url(target) {
        return Some(NavTarget::Url(target.to_string()));
    }
    let path = Path::new(target);
    if path.extension().and_then(|ext| ext.to_str()) != Some("typ") {
        return Some(NavTarget::Url(target.to_string()));
    }

    let candidate = src_dir.join(path);
    if candidate.is_file() && candidate.extension().and_then(|ext| ext.to_str()) == Some("typ") {
        return Some(NavTarget::Page(candidate));
    }

    cwarn!("{context} target does not exist or is not a .typ file: {target}");
    None
}

fn resolve_file_list(
    context: &str,
    src_dir: &Path,
    item_path: Option<&str>,
    item_glob: Option<&str>,
    all_typ_files: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    if let Some(path) = item_path {
        let path = Path::new(path);
        let candidate = src_dir.join(path);
        if candidate.is_file() && candidate.extension().and_then(|ext| ext.to_str()) == Some("typ")
        {
            return Ok(vec![candidate]);
        }
        cwarn!(
            "{context} item path does not exist or is not a .typ file: {}",
            path.display()
        );
        return Ok(Vec::new());
    }

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

fn iter_typ_files(
    src_dir: &Path,
    include_hidden: bool,
    exclude: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    let exclude = exclude.iter().collect::<BTreeSet<_>>();
    let mut out = Vec::new();
    collect_typ_files(src_dir, src_dir, include_hidden, &exclude, &mut out)?;
    out.sort_by_key(|path| rel_posix(src_dir, path));
    Ok(out)
}

fn collect_typ_files(
    root: &Path,
    dir: &Path,
    include_hidden: bool,
    exclude: &BTreeSet<&PathBuf>,
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        if !include_hidden
            && rel
                .components()
                .any(|part| part.as_os_str().to_string_lossy().starts_with('.'))
        {
            continue;
        }
        if path.is_dir() {
            collect_typ_files(root, &path, include_hidden, exclude, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("typ")
            && !exclude.contains(&rel.to_path_buf())
        {
            out.push(path);
        }
    }
    Ok(())
}

fn stem_label(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .replace(['-', '_'], " ")
}

/// Reads the page metadata persisted by preprocessing. Missing or stale
/// entries degrade to an empty `PageMeta` rather than failing the build.
fn load_page_meta(src_dir: &Path, typ_files: &[PathBuf]) -> PageMetaMap {
    typ_files
        .iter()
        .map(|path| {
            let mut meta = read_page_meta_with_root(path, Some(src_dir))
                .map(|value| page_meta_from_value(&value))
                .unwrap_or_default();
            if meta.title.is_none() {
                meta.title = document_title_from_source(path);
            }
            (path.clone(), meta)
        })
        .collect()
}

fn document_title_from_source(path: &Path) -> Option<String> {
    let source = fs::read_to_string(path).ok()?;
    extract_document_title(&source)
}

fn extract_document_title(source: &str) -> Option<String> {
    let mut offset = 0;
    while let Some(relative) = source[offset..].find("#set") {
        let start = offset + relative;
        let mut rest_start = start + "#set".len();
        rest_start = skip_ws(source, rest_start);
        if !source[rest_start..].starts_with("document") {
            offset = rest_start;
            continue;
        }
        let after_document = rest_start + "document".len();
        if source[after_document..]
            .chars()
            .next()
            .is_some_and(is_identifier_char)
        {
            offset = after_document;
            continue;
        }
        let open = skip_ws(source, after_document);
        if source[open..].chars().next() != Some('(') {
            offset = after_document;
            continue;
        }
        let close = find_matching_delimiter(source, open, '(', ')')?;
        let args = &source[open + 1..close];
        return title_argument(args).and_then(title_value_to_text);
    }
    None
}

fn title_argument(args: &str) -> Option<&str> {
    let mut index = 0;
    while index < args.len() {
        index = skip_ws(args, index);
        if args[index..].starts_with("title") {
            let after_name = index + "title".len();
            if !args[after_name..]
                .chars()
                .next()
                .is_some_and(is_identifier_char)
            {
                let colon = skip_ws(args, after_name);
                if args[colon..].chars().next() == Some(':') {
                    let value_start = skip_ws(args, colon + 1);
                    return Some(args[value_start..].trim());
                }
            }
        }
        index += args[index..].chars().next()?.len_utf8();
    }
    None
}

fn title_value_to_text(value: &str) -> Option<String> {
    if value.starts_with('[') {
        let close = find_matching_delimiter(value, 0, '[', ']')?;
        return clean_optional_string(Some(&typst_content_to_plain_text(&value[1..close])));
    }
    if value.starts_with('"') {
        let close = find_string_end(value, 0)?;
        let raw = &value[..=close];
        let parsed = serde_json::from_str::<String>(raw).ok()?;
        return clean_optional_string(Some(&parsed));
    }
    let value = value.split(',').next().unwrap_or(value);
    clean_optional_string(Some(&typst_content_to_plain_text(value)))
}

fn typst_content_to_plain_text(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '#' => {
                while chars.peek().is_some_and(|next| is_identifier_char(*next)) {
                    chars.next();
                }
            }
            '[' | ']' => {}
            '\n' | '\r' | '\t' => out.push(' '),
            _ => out.push(ch),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn find_matching_delimiter(
    value: &str,
    open_index: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = open_index;
    while index < value.len() {
        let ch = value[index..].chars().next()?;
        if ch == '"' {
            index = find_string_end(value, index)? + 1;
            continue;
        }
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(index);
            }
        }
        index += ch.len_utf8();
    }
    None
}

fn find_string_end(value: &str, quote_index: usize) -> Option<usize> {
    let mut escaped = false;
    let mut index = quote_index + 1;
    while index < value.len() {
        let ch = value[index..].chars().next()?;
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(index);
        }
        index += ch.len_utf8();
    }
    None
}

fn skip_ws(value: &str, mut index: usize) -> usize {
    while index < value.len() {
        let Some(ch) = value[index..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'
}

fn page_meta_from_value(value: &serde_json::Value) -> PageMeta {
    PageMeta {
        title: value
            .get("title")
            .and_then(|title| title.as_str())
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(str::to_string),
        pdf: value.get("pdf").and_then(|pdf| pdf.as_bool()),
        translation_key: value
            .get("translation_key")
            .or_else(|| value.get("translationKey"))
            .and_then(|key| key.as_str())
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(str::to_string),
        slug: value
            .get("slug")
            .and_then(|slug| slug.as_str())
            .map(str::trim)
            .filter(|slug| !slug.is_empty())
            .map(str::to_string),
        url: value
            .get("url")
            .and_then(|url| url.as_str())
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(str::to_string),
        raw: if value.is_object() {
            value.clone()
        } else {
            serde_json::json!({})
        },
    }
}

/// Runs `task` over `items` on a small worker pool, failing on the first
/// error. Results are returned in completion order.
fn run_parallel<T: Send>(
    items: Vec<PathBuf>,
    parallelism: Option<usize>,
    task: impl Fn(&Path) -> Result<T> + Sync,
) -> Result<Vec<(PathBuf, T)>> {
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let worker_count = parallelism
        .unwrap_or_else(|| {
            thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1)
                .min(32)
        })
        .max(1)
        .min(items.len());
    let queue = Mutex::new(VecDeque::from(items));
    let results = Mutex::new(Vec::new());

    thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..worker_count {
            handles.push(scope.spawn(|| -> Result<()> {
                loop {
                    let Some(item) = queue.lock().unwrap().pop_front() else {
                        return Ok(());
                    };
                    let value = task(&item)?;
                    results.lock().unwrap().push((item, value));
                }
            }));
        }

        for handle in handles {
            match handle.join() {
                Ok(result) => result?,
                Err(_) => return Err(anyhow!("website build worker panicked")),
            }
        }
        Ok(())
    })?;
    Ok(results.into_inner().unwrap())
}

struct WebsitePreprocessOptions<'a> {
    typ_files: &'a [PathBuf],
    src_dir: &'a Path,
    config_path: &'a Path,
    quiet: bool,
    timeout: Option<u64>,
    params: &'a [String],
    cli_theme: Option<crate::theme::ThemeSelection>,
    fallback_theme: crate::theme::ThemeSelection,
    parallelism: Option<usize>,
}

fn preprocess_documents(
    options: WebsitePreprocessOptions<'_>,
) -> Result<BTreeMap<PathBuf, PreprocessOutput>> {
    let display_root =
        fs::canonicalize(options.src_dir).unwrap_or_else(|_| options.src_dir.to_path_buf());
    let outputs = run_parallel(options.typ_files.to_vec(), options.parallelism, |input| {
        preprocess_cached(PreprocessOptions {
            input: input.to_path_buf(),
            root: Some(options.src_dir.to_path_buf()),
            config: Some(options.config_path.to_path_buf()),
            display_root: Some(display_root.clone()),
            quiet: options.quiet,
            timeout: options.timeout,
            sync_pages: false,
            theme: options.cli_theme.clone(),
            fallback_theme: options.fallback_theme.clone(),
            param_overrides: options.params.to_vec(),
        })
        .with_context(|| format!("failed to preprocess {}", input.display()))
    })?;
    Ok(outputs.into_iter().collect())
}

fn render_documents(
    context: &BuildContext,
    typ_files: Vec<PathBuf>,
    site: &SiteModel,
    preprocessed: &BTreeMap<PathBuf, PreprocessOutput>,
) -> Result<()> {
    run_parallel(typ_files, context.parallelism, |input_path| {
        render_document(context, site, input_path, preprocessed)
            .with_context(|| format!("failed to render {}", input_path.display()))
    })
    .map(|_| ())
}

fn render_document(
    context: &BuildContext,
    site: &SiteModel,
    input_path: &Path,
    preprocessed: &BTreeMap<PathBuf, PreprocessOutput>,
) -> Result<()> {
    let preprocessed = preprocessed
        .get(input_path)
        .ok_or_else(|| anyhow!("page was not preprocessed: {}", input_path.display()))?;
    let page_info = context
        .page_info
        .get(input_path)
        .ok_or_else(|| anyhow!("page output was not planned: {}", input_path.display()))?;
    let html_output = context.out_dir.join(&page_info.href);
    if let Some(parent) = html_output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let current_href = page_info.href.clone();
    let mut site_context = site.theme_context(
        &current_href,
        Some(page_info),
        &context.page_info,
        context.languages.as_deref(),
    );
    let page_site_entry =
        crate::theme::resolve_html_entry(&preprocessed.theme, crate::theme::HtmlScope::Site)?;
    if page_site_entry
        .as_ref()
        .is_some_and(|entry| entry.is_default)
    {
        if let Some(stylesheet) = context.theme_stylesheet.as_deref() {
            site_context.stylesheet =
                Some(html_escape(&page_relative_url(&current_href, stylesheet)));
        }
    }
    compile_with_typst(
        &context.typst,
        &preprocessed.layout,
        CompileOptions {
            output: Some(html_output.clone()),
            format: Some("html"),
            typst_args: &context.typst_args,
            theme: &preprocessed.theme,
            html_scope: crate::theme::HtmlScope::Site,
            site_context: Some(&site_context),
            pages_input: Some(PAGES_INDEX_REF),
            current_href_input: Some(&current_href),
        },
    )?;
    embed_source_blob(&html_output, input_path)?;

    if context.pdf_files.contains(input_path) {
        let pdf_href = page_info
            .pdf_href
            .as_ref()
            .ok_or_else(|| anyhow!("PDF output was not planned: {}", input_path.display()))?;
        let pdf_output = context.out_dir.join(pdf_href);
        compile_with_typst(
            &context.typst,
            &preprocessed.layout,
            CompileOptions {
                output: Some(pdf_output),
                format: Some("pdf"),
                typst_args: &context.typst_args,
                theme: &preprocessed.theme,
                html_scope: crate::theme::HtmlScope::Site,
                site_context: None,
                pages_input: Some(PAGES_INDEX_REF),
                current_href_input: Some(&current_href),
            },
        )?;
    }

    Ok(())
}

fn embed_source_blob(html_output: &Path, source_path: &Path) -> Result<()> {
    let source = fs::read_to_string(source_path)
        .with_context(|| format!("failed to read {}", source_path.display()))?;
    let payload = serde_json::to_string(&source)?.replace("</", "<\\/");
    let mut html = fs::read_to_string(html_output)
        .with_context(|| format!("failed to read {}", html_output.display()))?;
    let script =
        format!("\n<script id=\"{SOURCE_DATA_ID}\" type=\"application/json\">{payload}</script>\n");
    if html.contains("</head>") {
        html = html.replacen("</head>", &(script + "</head>"), 1);
    } else {
        html.push_str(&script);
    }
    fs::write(html_output, html)
        .with_context(|| format!("failed to write {}", html_output.display()))
}

fn expected_generated_outputs(
    out_dir: &Path,
    typ_files: &[PathBuf],
    page_info: &PageInfoMap,
    sitemap_path: &Option<PathBuf>,
    theme_stylesheet_path: Option<&Path>,
    default_favicon_path: Option<&Path>,
) -> BTreeSet<PathBuf> {
    let mut outputs = BTreeSet::new();
    for input_path in typ_files {
        if let Some(info) = page_info.get(input_path) {
            outputs.insert(out_dir.join(&info.href));
            if let Some(pdf_href) = &info.pdf_href {
                outputs.insert(out_dir.join(pdf_href));
            }
        }
    }
    if let Some(path) = sitemap_path {
        outputs.insert(path.clone());
    }
    if let Some(path) = theme_stylesheet_path {
        outputs.insert(out_dir.join(path));
    }
    if let Some(path) = default_favicon_path {
        outputs.insert(out_dir.join(path));
    }
    outputs
}

fn write_default_favicon(out_dir: &Path, path: &Path) -> Result<()> {
    let path = out_dir.join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&path, DEFAULT_FAVICON_SVG)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn load_manifest(out_dir: &Path) -> Result<WebsiteManifest> {
    let path = out_dir.join(MANIFEST_PATH);
    if !path.is_file() {
        return Ok(WebsiteManifest::default());
    }
    let contents =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&contents).with_context(|| format!("failed to parse {}", path.display()))
}

fn reconcile_manifest_outputs(
    out_dir: &Path,
    manifest: &WebsiteManifest,
    expected_outputs: &BTreeSet<PathBuf>,
) -> Result<()> {
    for rel in &manifest.outputs {
        let path = out_dir.join(Path::new(rel));
        if expected_outputs.contains(&path) || !path.exists() {
            continue;
        }
        if path.is_file() {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove stale output {}", path.display()))?;
        }
    }
    Ok(())
}

fn remove_unexpected_rendered_outputs(
    out_dir: &Path,
    expected_outputs: &BTreeSet<PathBuf>,
) -> Result<()> {
    if !out_dir.is_dir() {
        return Ok(());
    }
    remove_unexpected_rendered_outputs_in(out_dir, out_dir, expected_outputs)
}

fn remove_unexpected_rendered_outputs_in(
    root: &Path,
    dir: &Path,
    expected_outputs: &BTreeSet<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let Some(first) = rel.components().next() else {
            continue;
        };
        if first
            .as_os_str()
            .to_str()
            .is_some_and(|name| name == "assets" || SKIP_DIRS.contains(&name))
        {
            continue;
        }
        if path.is_dir() {
            remove_unexpected_rendered_outputs_in(root, &path, expected_outputs)?;
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("html" | "pdf")
        ) && !expected_outputs.contains(&path)
        {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove stale output {}", path.display()))?;
        }
    }
    Ok(())
}

fn write_manifest(out_dir: &Path, expected_outputs: &BTreeSet<PathBuf>) -> Result<()> {
    let path = out_dir.join(MANIFEST_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let manifest = WebsiteManifest {
        outputs: expected_outputs
            .iter()
            .map(|path| rel_posix(out_dir, path))
            .collect(),
    };
    let contents = serde_json::to_string_pretty(&manifest)?;
    fs::write(&path, contents).with_context(|| format!("failed to write {}", path.display()))
}

/// Writes the sitemap from every built page except the 404 page.
fn write_sitemap(out_dir: &Path, base_url: Option<&str>, hrefs: &BTreeSet<String>) -> Result<()> {
    let path = out_dir.join("sitemap.xml");
    let Some(base_url) = base_url else {
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove stale sitemap {}", path.display()))?;
        }
        return Ok(());
    };

    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for href in hrefs {
        xml.push_str("  <url><loc>");
        xml.push_str(&xml_escape(&absolute_site_url(base_url, &href)));
        xml.push_str("</loc></url>\n");
    }
    xml.push_str("</urlset>\n");

    fs::write(&path, xml).with_context(|| format!("failed to write {}", path.display()))
}

fn clear_previous_outputs(src_dir: &Path, out_dir: &Path) -> Result<()> {
    if out_dir == src_dir {
        return Ok(());
    } else if out_dir.exists() {
        for entry in fs::read_dir(out_dir)
            .with_context(|| format!("failed to read {}", out_dir.display()))?
        {
            let path = entry?.path();
            let name = path.file_name().and_then(|name| name.to_str());
            if path.is_dir() {
                // The output directory may be a git checkout (e.g. a gh-pages
                // worktree) or hold regenerable state; never delete those.
                if name.is_some_and(|name| SKIP_DIRS.contains(&name)) {
                    continue;
                }
                fs::remove_dir_all(&path)
                    .with_context(|| format!("failed to remove {}", path.display()))?;
            } else if name != Some(".gitkeep") {
                fs::remove_file(&path)
                    .with_context(|| format!("failed to remove {}", path.display()))?;
            }
        }
    }
    Ok(())
}

fn copy_assets(src_dir: &Path, out_dir: &Path) -> Result<()> {
    let assets = src_dir.join("assets");
    if !assets.is_dir() {
        return Ok(());
    }
    let target = out_dir.join("assets");
    if target.exists() {
        fs::remove_dir_all(&target)
            .with_context(|| format!("failed to remove {}", target.display()))?;
    }
    copy_dir_all(&assets, &target)
}

fn copy_typ_sources(src_dir: &Path, out_dir: &Path, typ_files: &[PathBuf]) -> Result<()> {
    for input_path in typ_files {
        let rel = input_path.strip_prefix(src_dir).unwrap_or(input_path);
        let target = out_dir.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::copy(input_path, &target).with_context(|| {
            format!(
                "failed to copy {} to {}",
                input_path.display(),
                target.display()
            )
        })?;
    }
    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("failed to create {}", dst.display()))?;
    for entry in fs::read_dir(src).with_context(|| format!("failed to read {}", src.display()))? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_all(&path, &target)?;
        } else {
            fs::copy(&path, &target).with_context(|| {
                format!("failed to copy {} to {}", path.display(), target.display())
            })?;
        }
    }
    Ok(())
}

fn rel_posix(src_dir: &Path, path: &Path) -> String {
    path.strip_prefix(src_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn slash_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            _ => out.push(component.as_os_str()),
        }
    }
    out
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let mut dp = vec![vec![false; value.len() + 1]; pattern.len() + 1];
    dp[0][0] = true;
    for i in 1..=pattern.len() {
        if pattern[i - 1] == b'*' {
            dp[i][0] = dp[i - 1][0];
        }
    }
    for i in 1..=pattern.len() {
        for j in 1..=value.len() {
            dp[i][j] = match pattern[i - 1] {
                b'*' => dp[i - 1][j] || dp[i][j - 1],
                b'?' => dp[i - 1][j - 1],
                byte => dp[i - 1][j - 1] && byte == value[j - 1],
            };
        }
    }
    dp[pattern.len()][value.len()]
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn xml_escape(value: &str) -> String {
    html_escape(value).replace('\'', "&apos;")
}

fn clean_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn absolute_site_url(base_url: &str, href: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let href = href.trim_start_matches('/');
    if href.is_empty() {
        base.to_string()
    } else {
        format!("{base}/{href}")
    }
}

fn page_relative_url(current_href: &str, target: &str) -> String {
    if is_absolute_or_special_url(target) {
        return target.to_string();
    }

    let target = target.trim_start_matches("./");
    let current_dir = current_href
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let current_dir = current_dir
        .get(..current_dir.len().saturating_sub(1))
        .unwrap_or(&[]);
    let target_parts = target
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let common_len = current_dir
        .iter()
        .zip(target_parts.iter())
        .take_while(|(left, right)| left == right)
        .count();
    let up_levels = current_dir.len().saturating_sub(common_len);
    let remaining_target = target_parts.get(common_len..).unwrap_or(&[]).join("/");

    match (up_levels, remaining_target.is_empty()) {
        (0, false) => remaining_target,
        (0, true) => target.to_string(),
        (_, false) => format!("{}{}", "../".repeat(up_levels), remaining_target),
        (_, true) => "../".repeat(up_levels).trim_end_matches('/').to_string(),
    }
}

fn is_absolute_or_special_url(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with('#')
        || value.starts_with("data:")
        || value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("//")
        || value.starts_with("mailto:")
        || value.starts_with("tel:")
}

fn absolutize_from(root: &Path, path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(root.join(path))
    }
}

fn absolutize_for_create(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn write_scaffold_file(path: &Path, contents: &str, force: bool) -> Result<()> {
    if path.exists() && !force {
        return Err(anyhow!(
            "{} already exists; pass --force to overwrite it",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

const WEBSITE_TOML_TEMPLATE: &str = include_str!("../assets/scaffolds/website/calepin.toml");
const INDEX_TYP_TEMPLATE: &str = include_str!("../assets/scaffolds/website/docs/index.typ");
const NOT_FOUND_TYP_TEMPLATE: &str = include_str!("../assets/scaffolds/website/docs/404.typ");

const ACADEMIC_SCAFFOLD_FILES: &[(&str, &str)] = &[
    (
        "calepin.toml",
        include_str!("../assets/scaffolds/academic/docs/calepin.toml"),
    ),
    (
        "index.typ",
        include_str!("../assets/scaffolds/academic/docs/index.typ"),
    ),
    (
        "assets/academic.typ",
        include_str!("../assets/scaffolds/academic/docs/assets/academic.typ"),
    ),
    (
        "assets/profile.svg",
        include_str!("../assets/scaffolds/academic/docs/assets/profile.svg"),
    ),
    (
        "publications/index.typ",
        include_str!("../assets/scaffolds/academic/docs/publications/index.typ"),
    ),
    (
        "publications/assets/academic.typ",
        include_str!("../assets/scaffolds/academic/docs/assets/academic.typ"),
    ),
    (
        "publications/example-paper.typ",
        include_str!("../assets/scaffolds/academic/docs/publications/example-paper.typ"),
    ),
    (
        "talks/index.typ",
        include_str!("../assets/scaffolds/academic/docs/talks/index.typ"),
    ),
    (
        "talks/assets/academic.typ",
        include_str!("../assets/scaffolds/academic/docs/assets/academic.typ"),
    ),
    (
        "talks/example-talk.typ",
        include_str!("../assets/scaffolds/academic/docs/talks/example-talk.typ"),
    ),
    (
        "teaching/index.typ",
        include_str!("../assets/scaffolds/academic/docs/teaching/index.typ"),
    ),
    (
        "teaching/assets/academic.typ",
        include_str!("../assets/scaffolds/academic/docs/assets/academic.typ"),
    ),
    (
        "teaching/pol6021.typ",
        include_str!("../assets/scaffolds/academic/docs/teaching/pol6021.typ"),
    ),
    (
        "projects/index.typ",
        include_str!("../assets/scaffolds/academic/docs/projects/index.typ"),
    ),
    (
        "projects/assets/academic.typ",
        include_str!("../assets/scaffolds/academic/docs/assets/academic.typ"),
    ),
    (
        "projects/example-project.typ",
        include_str!("../assets/scaffolds/academic/docs/projects/example-project.typ"),
    ),
    (
        "posts/index.typ",
        include_str!("../assets/scaffolds/academic/docs/posts/index.typ"),
    ),
    (
        "posts/assets/academic.typ",
        include_str!("../assets/scaffolds/academic/docs/assets/academic.typ"),
    ),
    (
        "posts/welcome.typ",
        include_str!("../assets/scaffolds/academic/docs/posts/welcome.typ"),
    ),
    (
        "cv.typ",
        include_str!("../assets/scaffolds/academic/docs/cv.typ"),
    ),
    (
        "404.typ",
        include_str!("../assets/scaffolds/academic/docs/404.typ"),
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn test_build_result(root: &Path, pages: &[PathBuf]) -> WebsiteBuildResult {
        WebsiteBuildResult {
            src_dir: root.to_path_buf(),
            out_dir: root.to_path_buf(),
            config_path: root.join("calepin.toml"),
            theme_dir: None,
            page_fingerprints: fingerprint_files(pages).unwrap(),
            nav_signature: 0,
            pages_signature: 0,
        }
    }

    fn test_page_info(src: &Path, files: &[PathBuf], pdf_files: &BTreeSet<PathBuf>) -> PageInfoMap {
        build_page_info(src, files, &PageMetaMap::new(), pdf_files, &None).unwrap()
    }

    fn website_config_from_toml(toml: &str) -> WebsiteConfig {
        try_website_config_from_toml(toml).unwrap()
    }

    fn try_website_config_from_toml(toml: &str) -> Result<WebsiteConfig, toml::de::Error> {
        toml::from_str(toml)
    }

    #[test]
    fn theme_key_parses_builtin_name() {
        let config = website_config_from_toml(r#"theme = "academic""#);
        assert_eq!(
            config.theme_selection(Path::new("/tmp")).unwrap(),
            crate::theme::ThemeSelection::Builtin("academic")
        );
    }

    #[test]
    fn theme_key_false_disables() {
        let config = website_config_from_toml("theme = false");
        assert_eq!(
            config.theme_selection(Path::new("/tmp")).unwrap(),
            crate::theme::ThemeSelection::Disabled
        );
    }

    #[test]
    fn missing_theme_key_is_default() {
        let config = website_config_from_toml("");
        assert_eq!(
            config.theme_selection(Path::new("/tmp")).unwrap(),
            crate::theme::ThemeSelection::Default
        );
    }

    #[test]
    fn html_theme_key_is_rejected() {
        assert!(try_website_config_from_toml(r#"html_theme = "academic""#).is_err());
    }

    #[test]
    fn home_config_key_is_rejected() {
        assert!(try_website_config_from_toml(r#"home = "index.html""#).is_err());
    }

    #[test]
    fn favicon_config_parses_and_defaults_to_generated_asset() {
        let src = Path::new("/site/docs");
        let config = website_config_from_toml(r#"favicon = "assets/favicon.ico""#);
        let metadata = SiteMetadata::from_config(&config, src).unwrap();
        assert_eq!(metadata.favicon.as_deref(), Some("assets/favicon.ico"));

        let config = website_config_from_toml("");
        let metadata = SiteMetadata::from_config(&config, src).unwrap();
        assert_eq!(metadata.favicon.as_deref(), Some(DEFAULT_FAVICON_PATH));
    }

    #[test]
    fn logo_and_favicon_paths_resolve_from_source_directory() {
        let src = Path::new("/site/docs");
        let config = website_config_from_toml(
            r#"
logo = "./assets/logo.svg"
favicon = "assets/favicon.ico"
"#,
        );

        let metadata = SiteMetadata::from_config(&config, src).unwrap();

        assert_eq!(metadata.logo.as_deref(), Some("assets/logo.svg"));
        assert_eq!(metadata.favicon.as_deref(), Some("assets/favicon.ico"));

        let config = website_config_from_toml(r#"logo = "../logo.svg""#);
        let err = SiteMetadata::from_config(&config, src).unwrap_err();
        assert!(err.to_string().contains("source directory"));
    }

    #[test]
    fn configured_languages_defaults_to_directory_per_language() {
        let src = Path::new("/site/docs");
        let config = WebsiteConfig {
            default_language: Some("en".to_string()),
            languages: BTreeMap::from([
                (
                    "en".to_string(),
                    LanguageConfig {
                        label: Some("English".to_string()),
                        content_dir: Some(PathBuf::from(".")),
                        ..LanguageConfig::default()
                    },
                ),
                (
                    "fr".to_string(),
                    LanguageConfig {
                        label: Some("Français".to_string()),
                        ..LanguageConfig::default()
                    },
                ),
            ]),
            ..WebsiteConfig::default()
        };

        let languages = configured_languages(src, &config).unwrap().unwrap();

        assert_eq!(languages[0].code, "en");
        assert_eq!(languages[0].content_dir, src);
        assert_eq!(languages[0].url_prefix, "");
        assert!(languages[0].default);
        assert_eq!(languages[1].code, "fr");
        assert_eq!(languages[1].content_dir, src.join("fr"));
        assert_eq!(languages[1].url_prefix, "fr");
        assert_eq!(languages[1].label, "Français");
    }

    #[test]
    fn configured_languages_requires_explicit_default_for_multiple_languages() {
        let src = Path::new("/site/docs");
        let config = WebsiteConfig {
            languages: BTreeMap::from([
                ("en".to_string(), LanguageConfig::default()),
                ("fr".to_string(), LanguageConfig::default()),
            ]),
            ..WebsiteConfig::default()
        };

        let error = configured_languages(src, &config).unwrap_err();
        assert!(error.to_string().contains("default_language"));

        let single = WebsiteConfig {
            languages: BTreeMap::from([("fr".to_string(), LanguageConfig::default())]),
            ..WebsiteConfig::default()
        };
        let languages = configured_languages(src, &single).unwrap().unwrap();
        assert!(languages[0].default);
    }

    #[test]
    fn configured_languages_rejects_url_prefix_escaping_output_directory() {
        let src = Path::new("/site/docs");
        let config = WebsiteConfig {
            default_language: Some("en".to_string()),
            languages: BTreeMap::from([(
                "en".to_string(),
                LanguageConfig {
                    url_prefix: Some("../outside".to_string()),
                    ..LanguageConfig::default()
                },
            )]),
            ..WebsiteConfig::default()
        };

        let error = configured_languages(src, &config).unwrap_err();
        assert!(error.to_string().contains("url_prefix"));
    }

    #[test]
    fn discover_site_pages_does_not_treat_language_dirs_as_default_pages() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path();
        fs::write(src.join("index.typ"), "= Home\n").unwrap();
        fs::write(src.join("about.typ"), "= About\n").unwrap();
        fs::create_dir_all(src.join("fr")).unwrap();
        fs::write(src.join("fr").join("index.typ"), "= Accueil\n").unwrap();
        fs::write(src.join("fr").join("about.typ"), "= À propos\n").unwrap();
        let languages = Some(vec![
            LanguageInfo {
                code: "en".to_string(),
                label: "English".to_string(),
                content_dir: src.to_path_buf(),
                url_prefix: String::new(),
                default: true,
            },
            LanguageInfo {
                code: "fr".to_string(),
                label: "Français".to_string(),
                content_dir: src.join("fr"),
                url_prefix: "fr".to_string(),
                default: false,
            },
        ]);

        let (_sections, files) = discover_site_pages(src, None, None, &languages).unwrap();
        let mut rel = files
            .iter()
            .map(|path| rel_posix(src, path))
            .collect::<Vec<_>>();
        rel.sort();

        assert_eq!(
            rel,
            vec!["about.typ", "fr/about.typ", "fr/index.typ", "index.typ"]
        );
    }

    #[test]
    fn implicit_build_pages_include_root_home_and_fallback_outside_configured_navigation() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path();
        fs::write(src.join("index.typ"), "= Home\n").unwrap();
        fs::write(src.join("404.typ"), "= Not Found\n").unwrap();
        fs::write(src.join("about.typ"), "= About\n").unwrap();
        let sidebar = SidebarConfig {
            section: vec![SidebarSectionConfig {
                item: vec![SidebarItemConfig {
                    target: Some("about.typ".to_string()),
                    ..SidebarItemConfig::default()
                }],
                ..SidebarSectionConfig::default()
            }],
            ..SidebarConfig::default()
        };

        let (_sections, files) = discover_site_pages(src, Some(&sidebar), None, &None).unwrap();
        assert_eq!(
            files
                .iter()
                .map(|path| rel_posix(src, path))
                .collect::<Vec<_>>(),
            vec!["about.typ"]
        );

        let mut build_files = files;
        build_files.extend(implicit_build_pages(src, &None));
        build_files.sort_by_key(|path| rel_posix(src, path));

        assert_eq!(
            build_files
                .iter()
                .map(|path| rel_posix(src, path))
                .collect::<Vec<_>>(),
            vec!["404.typ", "about.typ", "index.typ"]
        );
    }

    #[test]
    fn implicit_build_pages_include_each_language_home_and_fallback() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path();
        fs::write(src.join("index.typ"), "= Home\n").unwrap();
        fs::write(src.join("404.typ"), "= Not Found\n").unwrap();
        fs::create_dir_all(src.join("fr")).unwrap();
        fs::write(src.join("fr").join("index.typ"), "= Accueil\n").unwrap();
        fs::write(src.join("fr").join("404.typ"), "= Introuvable\n").unwrap();
        let languages = Some(vec![
            LanguageInfo {
                code: "en".to_string(),
                label: "English".to_string(),
                content_dir: src.to_path_buf(),
                url_prefix: String::new(),
                default: true,
            },
            LanguageInfo {
                code: "fr".to_string(),
                label: "Français".to_string(),
                content_dir: src.join("fr"),
                url_prefix: "fr".to_string(),
                default: false,
            },
        ]);

        let mut pages = implicit_build_pages(src, &languages);
        pages.sort_by_key(|path| rel_posix(src, path));

        assert_eq!(
            pages
                .iter()
                .map(|path| rel_posix(src, path))
                .collect::<Vec<_>>(),
            vec!["404.typ", "fr/404.typ", "fr/index.typ", "index.typ"]
        );
    }

    #[test]
    fn pages_include_adds_build_only_pages() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path();
        fs::write(src.join("index.typ"), "= Home\n").unwrap();
        fs::write(src.join("about.typ"), "= About\n").unwrap();
        fs::create_dir_all(src.join("landing")).unwrap();
        fs::write(src.join("landing").join("campaign.typ"), "= Campaign\n").unwrap();
        let pages = PagesConfig {
            include: vec!["landing/*.typ".to_string()],
            ..PagesConfig::default()
        };
        let sidebar = SidebarConfig {
            section: vec![SidebarSectionConfig {
                item: vec![SidebarItemConfig {
                    target: Some("about.typ".to_string()),
                    ..SidebarItemConfig::default()
                }],
                ..SidebarSectionConfig::default()
            }],
            ..SidebarConfig::default()
        };

        let (_sections, nav_files) =
            discover_site_pages(src, Some(&sidebar), Some(&pages), &None).unwrap();
        let include_files = discover_site_build_pages(src, Some(&pages), &None).unwrap();

        assert_eq!(
            nav_files
                .iter()
                .map(|path| rel_posix(src, path))
                .collect::<Vec<_>>(),
            vec!["about.typ"]
        );
        assert_eq!(
            include_files
                .iter()
                .map(|path| rel_posix(src, path))
                .collect::<Vec<_>>(),
            vec!["landing/campaign.typ"]
        );
    }

    #[test]
    fn pages_exclude_removes_pages_from_navigation_and_includes() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path();
        fs::write(src.join("index.typ"), "= Home\n").unwrap();
        fs::write(src.join("about.typ"), "= About\n").unwrap();
        fs::create_dir_all(src.join("drafts")).unwrap();
        fs::write(src.join("drafts").join("idea.typ"), "= Draft\n").unwrap();
        let pages = PagesConfig {
            include: vec!["drafts/*.typ".to_string()],
            exclude: vec!["drafts/**".to_string(), "about.typ".to_string()],
        };

        let (_sections, nav_files) = discover_site_pages(src, None, Some(&pages), &None).unwrap();
        let include_files = discover_site_build_pages(src, Some(&pages), &None).unwrap();
        let required = implicit_build_pages(src, &None);

        assert_eq!(
            nav_files
                .iter()
                .map(|path| rel_posix(src, path))
                .collect::<Vec<_>>(),
            vec!["index.typ"]
        );
        assert!(include_files.is_empty());
        assert_eq!(
            required
                .iter()
                .map(|path| rel_posix(src, path))
                .collect::<Vec<_>>(),
            vec!["index.typ"]
        );
    }

    #[test]
    fn build_page_info_uses_language_prefixes_slugs_and_translation_keys() {
        let src = Path::new("/site/docs");
        let en = PathBuf::from("/site/docs/about.typ");
        let fr = PathBuf::from("/site/docs/fr/about.typ");
        let languages = Some(vec![
            LanguageInfo {
                code: "en".to_string(),
                label: "English".to_string(),
                content_dir: src.to_path_buf(),
                url_prefix: String::new(),
                default: true,
            },
            LanguageInfo {
                code: "fr".to_string(),
                label: "Français".to_string(),
                content_dir: src.join("fr"),
                url_prefix: "fr".to_string(),
                default: false,
            },
        ]);
        let meta = PageMetaMap::from([(
            fr.clone(),
            page_meta_from_value(
                &serde_json::json!({"translation_key": "about", "slug": "a-propos"}),
            ),
        )]);
        let pdf_files = BTreeSet::from([fr.clone()]);

        let info = build_page_info(
            src,
            &[en.clone(), fr.clone()],
            &meta,
            &pdf_files,
            &languages,
        )
        .unwrap();

        assert_eq!(info[&en].language.as_deref(), Some("en"));
        assert_eq!(info[&en].translation_key, "about");
        assert_eq!(info[&en].href, "about.html");
        assert_eq!(info[&fr].language.as_deref(), Some("fr"));
        assert_eq!(info[&fr].translation_key, "about");
        assert_eq!(info[&fr].href, "fr/a-propos.html");
        assert_eq!(info[&fr].pdf_href.as_deref(), Some("fr/a-propos.pdf"));
    }

    #[test]
    fn build_page_info_keeps_pdf_distinct_from_custom_url_with_extension() {
        let src = Path::new("/site/docs");
        let page = PathBuf::from("/site/docs/about.typ");
        let meta = PageMetaMap::from([(
            page.clone(),
            page_meta_from_value(&serde_json::json!({"url": "info/about.html"})),
        )]);
        let pdf_files = BTreeSet::from([page.clone()]);

        let info =
            build_page_info(src, std::slice::from_ref(&page), &meta, &pdf_files, &None).unwrap();

        assert_eq!(info[&page].href, "info/about.html");
        assert_eq!(info[&page].pdf_href.as_deref(), Some("info/about.pdf"));
    }

    #[test]
    fn build_page_info_rejects_slug_and_url_escaping_output_directory() {
        let src = Path::new("/site/docs");
        let page = PathBuf::from("/site/docs/about.typ");

        for value in [
            serde_json::json!({"slug": "../escape"}),
            serde_json::json!({"slug": "/absolute"}),
            serde_json::json!({"url": "../escape.html"}),
        ] {
            let meta = PageMetaMap::from([(page.clone(), page_meta_from_value(&value))]);
            let error = build_page_info(
                src,
                std::slice::from_ref(&page),
                &meta,
                &BTreeSet::new(),
                &None,
            )
            .unwrap_err();
            assert!(
                error.to_string().contains("output directory"),
                "expected rejection for {value}: {error}"
            );
        }
    }

    #[test]
    fn sanitize_icon_svg_accepts_plain_icons_and_rejects_scripting_vectors() {
        let plain = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M3 12L12 3l9 9"/></svg>"#;
        assert_eq!(sanitize_icon_svg(plain, "home").unwrap(), plain);

        for bad in [
            r#"<svg><script>alert(1)</script></svg>"#,
            r#"<svg onclick="alert(1)"></svg>"#,
            r#"<svg ONLOAD = "alert(1)"></svg>"#,
            r#"<svg><a href="javascript:alert(1)">x</a></svg>"#,
            r#"<svg><foreignObject></foreignObject></svg>"#,
            "not svg at all",
        ] {
            assert!(sanitize_icon_svg(bad, "home").is_err(), "accepted: {bad}");
        }
    }

    #[test]
    fn translation_entries_are_relative_to_current_page() {
        let en = PathBuf::from("/site/docs/about.typ");
        let fr = PathBuf::from("/site/docs/fr/about.typ");
        let page_info = PageInfoMap::from([
            (
                en,
                PageInfo {
                    language: Some("en".to_string()),
                    translation_key: "about".to_string(),
                    href: "about.html".to_string(),
                    pdf_href: None,
                },
            ),
            (
                fr.clone(),
                PageInfo {
                    language: Some("fr".to_string()),
                    translation_key: "about".to_string(),
                    href: "fr/a-propos.html".to_string(),
                    pdf_href: None,
                },
            ),
        ]);
        let languages = vec![
            LanguageInfo {
                code: "en".to_string(),
                label: "English".to_string(),
                content_dir: PathBuf::from("/site/docs"),
                url_prefix: String::new(),
                default: true,
            },
            LanguageInfo {
                code: "fr".to_string(),
                label: "Français".to_string(),
                content_dir: PathBuf::from("/site/docs/fr"),
                url_prefix: "fr".to_string(),
                default: false,
            },
        ];

        let entries = translation_entries(
            "fr/a-propos.html",
            page_info.get(&fr).unwrap(),
            &page_info,
            &languages,
        );

        assert_eq!(entries[0].href, "../about.html");
        assert_eq!(entries[0].label, "English");
        assert!(!entries[0].active);
        assert_eq!(entries[1].href, "a-propos.html");
        assert!(entries[1].active);
    }

    #[test]
    fn language_entries_include_all_languages_with_home_fallbacks() {
        let en = PathBuf::from("/site/docs/about.typ");
        let page_info = PageInfoMap::from([(
            en.clone(),
            PageInfo {
                language: Some("en".to_string()),
                translation_key: "about".to_string(),
                href: "about.html".to_string(),
                pdf_href: None,
            },
        )]);
        let languages = vec![
            LanguageInfo {
                code: "en".to_string(),
                label: "English".to_string(),
                content_dir: PathBuf::from("/site/docs"),
                url_prefix: String::new(),
                default: true,
            },
            LanguageInfo {
                code: "fr".to_string(),
                label: "Français".to_string(),
                content_dir: PathBuf::from("/site/docs/fr"),
                url_prefix: "fr".to_string(),
                default: false,
            },
        ];

        let entries = language_entries("about.html", page_info.get(&en), &page_info, &languages);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].href, "about.html");
        assert!(entries[0].active);
        assert_eq!(entries[1].href, "fr/index.html");
        assert!(!entries[1].active);
    }

    #[test]
    fn changed_typ_pages_skips_unchanged_hashes() {
        let temp = tempfile::tempdir().unwrap();
        let page = temp.path().join("index.typ");
        fs::write(&page, "= Home\n").unwrap();
        let current = test_build_result(temp.path(), std::slice::from_ref(&page));

        let changed = changed_typ_pages(&current, std::slice::from_ref(&page)).unwrap();

        assert_eq!(changed, Some(Vec::new()));
    }

    #[test]
    fn changed_typ_pages_returns_modified_known_pages() {
        let temp = tempfile::tempdir().unwrap();
        let page = temp.path().join("index.typ");
        fs::write(&page, "= Home\n").unwrap();
        let current = test_build_result(temp.path(), std::slice::from_ref(&page));

        fs::write(&page, "= Updated\n").unwrap();
        let changed = changed_typ_pages(&current, std::slice::from_ref(&page)).unwrap();

        assert_eq!(changed, Some(vec![page]));
    }

    #[test]
    fn changed_typ_pages_falls_back_for_structural_changes() {
        let temp = tempfile::tempdir().unwrap();
        let page = temp.path().join("index.typ");
        let asset = temp.path().join("assets").join("site.css");
        fs::create_dir_all(asset.parent().unwrap()).unwrap();
        fs::write(&page, "= Home\n").unwrap();
        fs::write(&asset, "body {}\n").unwrap();
        let current = test_build_result(temp.path(), std::slice::from_ref(&page));

        let changed = changed_typ_pages(&current, std::slice::from_ref(&asset)).unwrap();

        assert_eq!(changed, None);
    }

    #[test]
    fn changed_typ_pages_falls_back_for_new_or_removed_pages() {
        let temp = tempfile::tempdir().unwrap();
        let page = temp.path().join("index.typ");
        let new_page = temp.path().join("new.typ");
        fs::write(&page, "= Home\n").unwrap();
        fs::write(&new_page, "= New\n").unwrap();
        let current = test_build_result(temp.path(), std::slice::from_ref(&page));

        let new_changed = changed_typ_pages(&current, std::slice::from_ref(&new_page)).unwrap();
        fs::remove_file(&page).unwrap();
        let removed_changed = changed_typ_pages(&current, std::slice::from_ref(&page)).unwrap();

        assert_eq!(new_changed, None);
        assert_eq!(removed_changed, None);
    }

    #[test]
    fn reconcile_manifest_outputs_removes_only_stale_generated_files() {
        let temp = tempfile::tempdir().unwrap();
        let stale = temp.path().join("old.html");
        let current = temp.path().join("index.html");
        fs::write(&stale, "old").unwrap();
        fs::write(&current, "current").unwrap();
        let manifest = WebsiteManifest {
            outputs: vec!["old.html".to_string(), "index.html".to_string()],
        };
        let expected = BTreeSet::from([current.clone()]);

        reconcile_manifest_outputs(temp.path(), &manifest, &expected).unwrap();

        assert!(!stale.exists());
        assert!(current.exists());
    }

    #[test]
    fn write_sitemap_uses_absolute_page_urls() {
        let temp = tempfile::tempdir().unwrap();
        let hrefs = BTreeSet::from(["index.html".to_string(), "guide/usage.html".to_string()]);

        write_sitemap(temp.path(), Some("https://example.com/project/"), &hrefs).unwrap();

        let sitemap = fs::read_to_string(temp.path().join("sitemap.xml")).unwrap();
        assert!(sitemap.contains("<loc>https://example.com/project/index.html</loc>"));
        assert!(sitemap.contains("<loc>https://example.com/project/guide/usage.html</loc>"));
    }

    #[test]
    fn theme_context_rewrites_brand_urls_relative_to_current_page() {
        let site = SiteModel::new(
            vec![NavSectionModel {
                language: None,
                title: Some("Guide".to_string()),
                items: vec![NavItemModel {
                    language: None,
                    href: "guide/usage.html".to_string(),
                    label: "Usage".to_string(),
                    label_html: html_escape("Usage"),
                }],
            }],
            NavbarModel::default(),
            SiteMetadata {
                title: Some("Example".to_string()),
                description: None,
                base_url: None,
                logo: Some("assets/logo.svg".to_string()),
                logo_alt: Some("Example".to_string()),
                favicon: Some("assets/favicon.ico".to_string()),
            },
            true,
        );

        let context = site.theme_context("guide/usage.html", None, &PageInfoMap::new(), None);

        assert_eq!(context.logo.as_deref(), Some("../assets/logo.svg"));
        assert_eq!(context.home_url.as_deref(), Some("../index.html"));
        assert_eq!(context.favicon.as_deref(), Some("../assets/favicon.ico"));
        assert_eq!(context.logo_alt.as_deref(), Some("Example"));
        assert_eq!(context.stylesheet, None);
    }

    #[test]
    fn theme_context_rewrites_nav_urls_relative_to_current_page() {
        let site = SiteModel::new(
            vec![NavSectionModel {
                language: None,
                title: None,
                items: vec![
                    NavItemModel {
                        language: None,
                        href: "index.html".to_string(),
                        label: "Home".to_string(),
                        label_html: html_escape("Home"),
                    },
                    NavItemModel {
                        language: None,
                        href: "publications/index.html".to_string(),
                        label: "Publications".to_string(),
                        label_html: html_escape("Publications"),
                    },
                    NavItemModel {
                        language: None,
                        href: "posts/welcome.html".to_string(),
                        label: "Welcome".to_string(),
                        label_html: html_escape("Welcome"),
                    },
                ],
            }],
            NavbarModel::default(),
            SiteMetadata::default(),
            true,
        );

        let context = site.theme_context("posts/welcome.html", None, &PageInfoMap::new(), None);
        let hrefs = context
            .sidebar
            .iter()
            .map(|item| item.href.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            hrefs,
            vec![
                "../index.html",
                "../publications/index.html",
                "welcome.html"
            ]
        );
        assert!(context.sidebar[2].active);
    }

    #[test]
    fn theme_context_marks_section_containing_current_page_active() {
        let section = |title: &str, href: &str| NavSectionModel {
            language: None,
            title: Some(title.to_string()),
            items: vec![NavItemModel {
                language: None,
                href: href.to_string(),
                label: title.to_string(),
                label_html: html_escape(title),
            }],
        };
        let site = SiteModel::new(
            vec![
                section("Guide", "guide/usage.html"),
                section("Reference", "reference/cli.html"),
            ],
            NavbarModel::default(),
            SiteMetadata::default(),
            true,
        );

        let context = site.theme_context("reference/cli.html", None, &PageInfoMap::new(), None);

        assert!(context.sidebar_fold);
        assert!(!context.sidebar_sections[0].active);
        assert!(context.sidebar_sections[1].active);
    }

    #[test]
    fn page_relative_url_rewrites_generated_stylesheet_for_nested_pages() {
        assert_eq!(
            page_relative_url("guide/usage.html", WEBSITE_STYLESHEET_PATH),
            "../.calepin/calepin-website.css"
        );
        assert_eq!(
            page_relative_url("guide/usage.html", "guide/advanced.html"),
            "advanced.html"
        );
        assert_eq!(
            page_relative_url("posts/welcome.html", "publications/index.html"),
            "../publications/index.html"
        );
        assert_eq!(
            page_relative_url("index.html", WEBSITE_STYLESHEET_PATH),
            ".calepin/calepin-website.css"
        );
    }

    #[test]
    fn theme_key_resolves_local_directory_against_config_dir() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("themes/my-theme")).unwrap();
        std::fs::write(
            temp.path().join("themes/my-theme/site.html"),
            "{{ doc.body }}",
        )
        .unwrap();
        let config = website_config_from_toml(r#"theme = "themes/my-theme""#);

        assert_eq!(
            config.theme_selection(temp.path()).unwrap(),
            crate::theme::ThemeSelection::Dir(temp.path().join("themes/my-theme"))
        );
    }

    #[test]
    fn nav_from_plans_prefers_configured_label_then_metadata_title_then_stem() {
        let src = Path::new("/site/docs");
        let labeled = PathBuf::from("/site/docs/a.typ");
        let titled = PathBuf::from("/site/docs/b-page.typ");
        let bare = PathBuf::from("/site/docs/c_page.typ");
        let sections = vec![NavSectionPlan {
            language: None,
            title: Some("Guide".to_string()),
            items: vec![
                NavItemPlan {
                    path: Some(labeled.clone()),
                    url: None,
                    configured_label: Some("Configured".to_string()),
                    icon: None,
                },
                NavItemPlan {
                    path: Some(titled.clone()),
                    url: None,
                    configured_label: None,
                    icon: None,
                },
                NavItemPlan {
                    path: Some(bare.clone()),
                    url: None,
                    configured_label: None,
                    icon: None,
                },
            ],
        }];
        let meta = PageMetaMap::from([
            (
                labeled.clone(),
                PageMeta {
                    title: Some("Ignored".to_string()),
                    ..PageMeta::default()
                },
            ),
            (
                titled.clone(),
                PageMeta {
                    title: Some("From Metadata".to_string()),
                    ..PageMeta::default()
                },
            ),
        ]);

        let files = vec![labeled.clone(), titled.clone(), bare.clone()];
        let page_info = test_page_info(src, &files, &BTreeSet::new());
        let icon_temp = tempfile::tempdir().unwrap();
        let mut icon_cache = IconCache::new(icon_temp.path().join(".calepin/icons"));
        let nav = nav_from_plans(&sections, &meta, &page_info, &mut icon_cache).unwrap();

        let labels = nav[0]
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>();
        assert_eq!(labels, vec!["Configured", "From Metadata", "c page"]);
    }

    #[test]
    fn nav_from_plans_interpolates_cached_icons_in_labels() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path();
        let home = src.join("index.typ");
        fs::write(&home, "= Home\n").unwrap();
        let icon_path = src.join(".calepin/icons/lucide/home.svg");
        fs::create_dir_all(icon_path.parent().unwrap()).unwrap();
        fs::write(
            &icon_path,
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M3 12L12 3l9 9"/></svg>"#,
        )
        .unwrap();
        let sections = vec![NavSectionPlan {
            language: None,
            title: None,
            items: vec![NavItemPlan {
                path: Some(home.clone()),
                url: None,
                configured_label: Some("{icon:home} Home".to_string()),
                icon: None,
            }],
        }];
        let page_info = test_page_info(src, std::slice::from_ref(&home), &BTreeSet::new());
        let mut icon_cache = IconCache::new(src.join(ICON_CACHE_DIR));

        let nav =
            nav_from_plans(&sections, &PageMetaMap::new(), &page_info, &mut icon_cache).unwrap();

        assert_eq!(nav[0].items[0].label, "Home");
        assert!(nav[0].items[0]
            .label_html
            .contains(r#"<span class="calepin-nav-icon">"#));
        assert!(nav[0].items[0].label_html.contains("viewBox=\"0 0 24 24\""));
        assert!(nav[0].items[0].label_html.ends_with(" Home"));
    }

    #[test]
    fn discover_pages_resolves_sidebar_page_and_external_targets() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path();
        fs::write(src.join("index.typ"), "= Home\n").unwrap();
        let sidebar = SidebarConfig {
            section: vec![SidebarSectionConfig {
                item: vec![
                    SidebarItemConfig {
                        target: Some("index.typ".to_string()),
                        label: Some("Home".to_string()),
                        ..SidebarItemConfig::default()
                    },
                    SidebarItemConfig {
                        target: Some("https://example.com".to_string()),
                        label: Some("External".to_string()),
                        ..SidebarItemConfig::default()
                    },
                ],
                ..SidebarSectionConfig::default()
            }],
            ..SidebarConfig::default()
        };

        let (sections, files) = discover_pages(src, Some(&sidebar), None, None).unwrap();

        assert_eq!(
            files
                .iter()
                .map(|path| rel_posix(src, path))
                .collect::<Vec<_>>(),
            vec!["index.typ"]
        );
        assert_eq!(
            sections[0].items[0].path.as_deref(),
            Some(src.join("index.typ").as_path())
        );
        assert_eq!(
            sections[0].items[1].url.as_deref(),
            Some("https://example.com")
        );
    }

    #[test]
    fn discover_pages_rejects_target_combined_with_glob() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path();
        fs::write(src.join("index.typ"), "= Home\n").unwrap();
        let sidebar = SidebarConfig {
            section: vec![SidebarSectionConfig {
                item: vec![SidebarItemConfig {
                    target: Some("index.typ".to_string()),
                    glob: Some("*.typ".to_string()),
                    ..SidebarItemConfig::default()
                }],
                ..SidebarSectionConfig::default()
            }],
            ..SidebarConfig::default()
        };

        let err = discover_pages(src, Some(&sidebar), None, None).unwrap_err();

        assert!(err
            .to_string()
            .contains("sidebar target items cannot also set glob"));
    }

    #[test]
    fn discover_navbar_resolves_left_center_right_items() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path();
        fs::write(src.join("index.typ"), "= Home\n").unwrap();
        fs::create_dir_all(src.join("guide")).unwrap();
        fs::write(src.join("guide").join("usage.typ"), "= Usage\n").unwrap();
        let navbar = NavbarConfig {
            item: vec![
                NavbarItemConfig {
                    position: NavbarPosition::Left,
                    target: Some("index.typ".to_string()),
                    label: Some("Home".to_string()),
                    ..NavbarItemConfig::default()
                },
                NavbarItemConfig {
                    position: NavbarPosition::Center,
                    glob: Some("guide/*.typ".to_string()),
                    ..NavbarItemConfig::default()
                },
                NavbarItemConfig {
                    position: NavbarPosition::Right,
                    target: Some("https://github.com/example/project".to_string()),
                    label: Some("GitHub".to_string()),
                    ..NavbarItemConfig::default()
                },
            ],
            ..NavbarConfig::default()
        };

        let (plan, files) = discover_navbar(src, &navbar, None).unwrap();

        assert_eq!(plan.left.len(), 1);
        assert_eq!(plan.center.len(), 1);
        assert_eq!(plan.right.len(), 1);
        assert_eq!(
            plan.right[0].url.as_deref(),
            Some("https://github.com/example/project")
        );
        assert_eq!(
            files
                .iter()
                .map(|path| rel_posix(src, path))
                .collect::<Vec<_>>(),
            vec!["index.typ", "guide/usage.typ"]
        );
    }

    #[test]
    fn navbar_config_rejects_unknown_item_fields() {
        let err = toml::from_str::<NavbarConfig>(
            r#"
            [[item]]
            position = "right"
            behavior = "theme"
            "#,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unknown field `behavior`"));
    }

    #[test]
    fn discover_navbar_rejects_target_combined_with_glob() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path();
        fs::write(src.join("index.typ"), "= Home\n").unwrap();
        let navbar = NavbarConfig {
            item: vec![NavbarItemConfig {
                target: Some("https://example.com".to_string()),
                glob: Some("*.typ".to_string()),
                ..NavbarItemConfig::default()
            }],
            ..NavbarConfig::default()
        };

        let err = discover_navbar(src, &navbar, None).unwrap_err();

        assert!(err
            .to_string()
            .contains("navbar target items cannot also set glob"));
    }

    #[test]
    fn navbar_from_plan_uses_page_metadata_and_external_labels() {
        let src = Path::new("/site/docs");
        let home = PathBuf::from("/site/docs/index.typ");
        let usage = PathBuf::from("/site/docs/guide/usage.typ");
        let plan = NavbarPlan {
            left: vec![NavbarItemPlan {
                path: Some(home.clone()),
                url: None,
                configured_label: Some("Home".to_string()),
                icon: None,
            }],
            center: vec![NavbarItemPlan {
                path: Some(usage.clone()),
                url: None,
                configured_label: None,
                icon: None,
            }],
            right: vec![NavbarItemPlan {
                path: None,
                url: Some("https://example.com".to_string()),
                configured_label: Some("External".to_string()),
                icon: None,
            }],
        };
        let meta = PageMetaMap::from([(
            usage.clone(),
            PageMeta {
                title: Some("Usage Guide".to_string()),
                ..PageMeta::default()
            },
        )]);
        let page_info = test_page_info(src, &[home, usage], &BTreeSet::new());

        let icon_temp = tempfile::tempdir().unwrap();
        let mut icon_cache = IconCache::new(icon_temp.path().join(".calepin/icons"));
        let navbar = navbar_from_plan(&plan, &meta, &page_info, &mut icon_cache).unwrap();

        assert_eq!(navbar.left[0].href, "index.html");
        assert_eq!(navbar.left[0].label, "Home");
        assert_eq!(navbar.center[0].href, "guide/usage.html");
        assert_eq!(navbar.center[0].label, "Usage Guide");
        assert_eq!(navbar.right[0].href, "https://example.com");
        assert_eq!(navbar.right[0].label, "External");
    }

    #[test]
    fn theme_context_exposes_relative_navbar_regions() {
        let site = SiteModel::new(
            Vec::new(),
            NavbarModel {
                left: vec![NavItemModel {
                    language: None,
                    href: "index.html".to_string(),
                    label: "Home".to_string(),
                    label_html: html_escape("Home"),
                }],
                center: vec![NavItemModel {
                    language: None,
                    href: "guide/usage.html".to_string(),
                    label: "Usage".to_string(),
                    label_html: html_escape("Usage"),
                }],
                right: vec![NavItemModel {
                    language: None,
                    href: "https://example.com".to_string(),
                    label: "External".to_string(),
                    label_html: html_escape("External"),
                }],
            },
            SiteMetadata::default(),
            true,
        );

        let context = site.theme_context("guide/usage.html", None, &PageInfoMap::new(), None);

        assert_eq!(context.navbar_left[0].href, "../index.html");
        assert_eq!(context.navbar_center[0].href, "usage.html");
        assert!(context.navbar_center[0].active);
        assert_eq!(context.navbar_right[0].href, "https://example.com");
    }

    #[test]
    fn theme_context_filters_navbar_page_links_by_language() {
        let en = PathBuf::from("/site/docs/index.typ");
        let fr = PathBuf::from("/site/docs/fr/index.typ");
        let page_info = PageInfoMap::from([
            (
                en.clone(),
                PageInfo {
                    language: Some("en".to_string()),
                    translation_key: "index".to_string(),
                    href: "index.html".to_string(),
                    pdf_href: None,
                },
            ),
            (
                fr.clone(),
                PageInfo {
                    language: Some("fr".to_string()),
                    translation_key: "index".to_string(),
                    href: "fr/index.html".to_string(),
                    pdf_href: None,
                },
            ),
        ]);
        let site = SiteModel::new(
            Vec::new(),
            NavbarModel {
                left: vec![
                    NavItemModel {
                        language: Some("en".to_string()),
                        href: "index.html".to_string(),
                        label: "Home".to_string(),
                        label_html: html_escape("Home"),
                    },
                    NavItemModel {
                        language: Some("fr".to_string()),
                        href: "fr/index.html".to_string(),
                        label: "Accueil".to_string(),
                        label_html: html_escape("Accueil"),
                    },
                ],
                center: Vec::new(),
                right: vec![NavItemModel {
                    language: None,
                    href: "https://example.com".to_string(),
                    label: "External".to_string(),
                    label_html: html_escape("External"),
                }],
            },
            SiteMetadata::default(),
            true,
        );

        let context = site.theme_context("fr/index.html", page_info.get(&fr), &page_info, None);

        assert_eq!(context.navbar_left.len(), 1);
        assert_eq!(context.navbar_left[0].label, "Accueil");
        assert_eq!(context.navbar_left[0].href, "index.html");
        assert_eq!(context.navbar_right.len(), 1);
        assert_eq!(context.navbar_right[0].href, "https://example.com");
    }

    #[test]
    fn page_meta_from_value_reads_calepin_keys_and_keeps_raw_dict() {
        let value = serde_json::json!({"title": " My Page ", "pdf": false, "date": "2026-06-10"});
        let meta = page_meta_from_value(&value);
        assert_eq!(meta.title.as_deref(), Some("My Page"));
        assert_eq!(meta.pdf, Some(false));
        assert_eq!(meta.raw, value);

        let blank_title = page_meta_from_value(&serde_json::json!({"title": ""}));
        assert_eq!(blank_title.title, None);

        let not_a_dict = page_meta_from_value(&serde_json::json!("not a dict"));
        assert_eq!(not_a_dict.raw, serde_json::json!({}));
    }

    #[test]
    fn extract_document_title_reads_set_document_title() {
        assert_eq!(
            extract_document_title(
                r#"
#set document(title: [Site configuration])

#title()
"#
            )
            .as_deref(),
            Some("Site configuration")
        );
        assert_eq!(
            extract_document_title(
                r#"
#set document(
  title: [#emph[Calepin]: Computational notebooks in Typst],
)
"#
            )
            .as_deref(),
            Some("Calepin: Computational notebooks in Typst")
        );
        assert_eq!(
            extract_document_title(r#"#set document(title: "CLI reference")"#).as_deref(),
            Some("CLI reference")
        );
    }

    #[test]
    fn load_page_meta_falls_back_to_document_title() {
        let temp = tempfile::tempdir().unwrap();
        let page = temp.path().join("page.typ");
        fs::write(&page, "#set document(title: [From document])").unwrap();

        let meta = load_page_meta(temp.path(), std::slice::from_ref(&page));

        assert_eq!(meta[&page].title.as_deref(), Some("From document"));
    }

    #[test]
    fn discover_website_config_prefers_explicit_then_calepin() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("site");
        fs::create_dir_all(&input).unwrap();

        let missing = discover_website_config(temp.path(), &input, None);
        assert!(missing.is_err());
        assert!(missing.unwrap_err().to_string().contains("calepin.toml"));

        fs::write(input.join("calepin.toml"), "").unwrap();
        assert_eq!(
            discover_website_config(temp.path(), &input, None).unwrap(),
            input.join("calepin.toml")
        );

        let explicit = discover_website_config(
            temp.path(),
            &input,
            Some(Path::new("elsewhere/custom.toml")),
        )
        .unwrap();
        assert_eq!(explicit, temp.path().join("elsewhere/custom.toml"));
    }

    #[test]
    fn build_pages_index_resolves_titles_and_excludes_fallback_page() {
        let src = Path::new("/site/docs");
        let post = PathBuf::from("/site/docs/blog/first.typ");
        let home = PathBuf::from("/site/docs/index.typ");
        let fallback = PathBuf::from("/site/docs/404.typ");
        let typ_files = vec![fallback, post.clone(), home.clone()];
        let sections = vec![NavSectionPlan {
            language: None,
            title: None,
            items: vec![NavItemPlan {
                path: Some(home.clone()),
                url: None,
                configured_label: Some("Home".to_string()),
                icon: None,
            }],
        }];
        let raw = serde_json::json!({"title": "First Post", "date": "2026-06-10"});
        let meta = PageMetaMap::from([(post.clone(), page_meta_from_value(&raw))]);
        let pdf_files = BTreeSet::from([post.clone()]);

        let page_info = build_page_info(src, &typ_files, &meta, &pdf_files, &None).unwrap();
        let index = build_pages_index(src, &typ_files, &sections, &meta, &page_info);

        let entries = index.as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["path"], "blog/first.typ");
        assert_eq!(entries[0]["href"], "blog/first.html");
        assert_eq!(entries[0]["title"], "First Post");
        assert_eq!(entries[0]["pdf"], "blog/first.pdf");
        assert_eq!(entries[0]["meta"], raw);
        assert_eq!(entries[1]["title"], "Home");
        assert_eq!(entries[1]["pdf"], serde_json::Value::Null);
        assert_eq!(entries[1]["meta"], serde_json::json!({}));
    }

    #[test]
    fn pdf_enabled_files_honors_per_page_override_over_site_default() {
        let on = PathBuf::from("on.typ");
        let off = PathBuf::from("off.typ");
        let plain = PathBuf::from("plain.typ");
        let files = vec![on.clone(), off.clone(), plain.clone()];
        let meta = PageMetaMap::from([
            (
                on.clone(),
                PageMeta {
                    pdf: Some(true),
                    ..PageMeta::default()
                },
            ),
            (
                off.clone(),
                PageMeta {
                    pdf: Some(false),
                    ..PageMeta::default()
                },
            ),
        ]);

        let with_site_off = pdf_enabled_files(&files, &meta, None, Some(false));
        assert_eq!(with_site_off, BTreeSet::from([on.clone()]));

        let with_default = pdf_enabled_files(&files, &meta, None, None);
        assert_eq!(with_default, BTreeSet::from([on.clone()]));

        let with_site_on = pdf_enabled_files(&files, &meta, None, Some(true));
        assert_eq!(with_site_on, BTreeSet::from([on.clone(), plain]));

        let with_cli_off = pdf_enabled_files(&files, &meta, Some(false), Some(true));
        assert!(with_cli_off.is_empty());
    }

    #[test]
    fn should_rebuild_for_path_ignores_distinct_output_directory() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("docs");
        let out = src.join("_site");
        fs::create_dir_all(&out).unwrap();
        let mut current = test_build_result(&src, &[]);
        current.out_dir = out.clone();

        assert!(!should_rebuild_for_path(&current, &out.join("index.typ")));
        assert!(!should_rebuild_for_path(&current, &out.join("style.css")));
        assert!(should_rebuild_for_path(&current, &src.join("index.typ")));
    }

    #[test]
    fn should_rebuild_for_path_ignores_generated_calepin_directory() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("docs");
        let wrappers = [
            src.join(".calepin/index/calepin-wrapper.typ"),
            src.join("websites/.calepin/website-config/calepin-wrapper.typ"),
        ];
        for wrapper in &wrappers {
            fs::create_dir_all(wrapper.parent().unwrap()).unwrap();
            fs::write(wrapper, "#import \"/.calepin/calepin.typ\"").unwrap();
        }
        let current = test_build_result(&src, &[]);

        for wrapper in wrappers {
            assert!(!should_rebuild_for_path(&current, &wrapper));
        }
    }

    #[test]
    fn clear_previous_outputs_preserves_git_directory_in_output_dir() {
        let temp = tempfile::tempdir().unwrap();
        let src = temp.path().join("docs");
        let out = temp.path().join("site");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(out.join(".git")).unwrap();
        fs::write(out.join(".git").join("HEAD"), "ref: refs/heads/main").unwrap();
        fs::write(out.join("stale.html"), "old").unwrap();
        fs::write(out.join(".gitkeep"), "").unwrap();

        clear_previous_outputs(&src, &out).unwrap();

        assert!(out.join(".git").join("HEAD").exists());
        assert!(out.join(".gitkeep").exists());
        assert!(!out.join("stale.html").exists());
    }

    #[test]
    fn remove_unexpected_rendered_outputs_keeps_assets() {
        let temp = tempfile::tempdir().unwrap();
        let expected_page = temp.path().join("index.html");
        let stale_page = temp.path().join("old.html");
        let asset_pdf = temp.path().join("assets").join("manual.pdf");
        fs::create_dir_all(asset_pdf.parent().unwrap()).unwrap();
        fs::write(&expected_page, "index").unwrap();
        fs::write(&stale_page, "old").unwrap();
        fs::write(&asset_pdf, "asset").unwrap();
        let expected = BTreeSet::from([expected_page.clone()]);

        remove_unexpected_rendered_outputs(temp.path(), &expected).unwrap();

        assert!(expected_page.exists());
        assert!(!stale_page.exists());
        assert!(asset_pdf.exists());
    }

    #[test]
    fn clear_previous_outputs_preserves_in_place_rendered_files() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("index.typ"), "= Home\n").unwrap();
        fs::write(temp.path().join("index.html"), "previous html").unwrap();
        fs::write(temp.path().join("index.pdf"), "previous pdf").unwrap();
        fs::write(temp.path().join("notes.html"), "user html").unwrap();

        clear_previous_outputs(temp.path(), temp.path()).unwrap();

        assert!(temp.path().join("index.html").exists());
        assert!(temp.path().join("index.pdf").exists());
        assert!(temp.path().join("notes.html").exists());
    }
}
