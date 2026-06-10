mod serve;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use notify::RecursiveMode;
use notify_debouncer_full::new_debouncer;
use serde::{Deserialize, Serialize};
use xxhash_rust::xxh3::xxh3_64;

use crate::cli::{set_quiet, CompileArgs, CompileFormat, WatchArgs};
use crate::config::CalepinConfig;
use crate::html::{
    is_builtin_html_theme, is_theme_path_like, write_html_theme_stylesheet, SiteContextInput,
    SiteNavEntry, SiteNavSection,
};
use crate::typst::compile::{compile_with_typst, CompileOptions};
use crate::typst::preprocess::{
    preprocess_cached, read_page_meta, PreprocessOptions, PreprocessOutput,
};

const DEFAULT_CONFIG: &str = "website.toml";
const DEFAULT_SRC_DIR: &str = "docs";
const DEFAULT_WEBSITE_THEME: &str = "calepin-website";
const WEBSITE_STYLESHEET_PATH: &str = ".calepin/calepin-website.css";
const FALLBACK_PAGE: &str = "404.typ";
const SOURCE_DATA_ID: &str = "calepin-website-source-data";
const MANIFEST_PATH: &str = ".calepin/website-manifest.json";
const PAGES_INDEX_FILE: &str = "website-pages.json";
/// Root-relative reference to the pages index. Each page renders with
/// `--root` at its own directory, so the index is written into every page
/// directory's `.calepin` and this reference resolves for all of them.
const PAGES_INDEX_REF: &str = "/.calepin/website-pages.json";
const SKIP_DIRS: &[&str] = &[".calepin", ".git", "target", "node_modules", ".venv"];

pub(crate) fn scaffold_website(root: &Path, force: bool) -> Result<()> {
    let root = absolutize_for_create(root)?;
    let docs = root.join(DEFAULT_SRC_DIR);
    fs::create_dir_all(&docs).with_context(|| format!("failed to create {}", docs.display()))?;

    write_scaffold_file(&root.join(DEFAULT_CONFIG), WEBSITE_TOML_TEMPLATE, force)?;
    write_scaffold_file(&docs.join("index.typ"), INDEX_TYP_TEMPLATE, force)?;
    write_scaffold_file(&docs.join("404.typ"), NOT_FOUND_TYP_TEMPLATE, force)?;
    Ok(())
}

pub(crate) fn build_from_compile_args(args: CompileArgs) -> Result<()> {
    let Some(config_path) = args.common.config.clone() else {
        return Err(anyhow!(
            "compiling a website directory requires `--config website.toml`"
        ));
    };
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
    let Some(config_path) = args.common.config.clone() else {
        return Err(anyhow!(
            "watching a website directory requires `--config website.toml`"
        ));
    };
    if args.format.is_some() {
        return Err(anyhow!(
            "website directory watch does not support `--format`; use `calepin compile` for one-shot format control"
        ));
    }

    set_quiet(args.common.quiet);
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
    /// `None` defers to the `pdf` key in website.toml (default: render PDFs).
    render_pdf: Option<bool>,
    quiet: bool,
    timeout: Option<u64>,
    params: Vec<String>,
    typst_args: Vec<String>,
    incremental_inputs: Option<Vec<PathBuf>>,
    clean: bool,
}

/// Per-page metadata exposed through the `<website-metadata>` Typst label,
/// extracted during preprocessing and persisted under `.calepin/`. `title`,
/// `pdf`, and `hidden` are the keys calepin interprets; `raw` carries the
/// author's whole dictionary verbatim for the pages index.
#[derive(Debug, Clone, Default, PartialEq)]
struct PageMeta {
    title: Option<String>,
    pdf: Option<bool>,
    hidden: bool,
    raw: serde_json::Value,
}

type PageMetaMap = BTreeMap<PathBuf, PageMeta>;

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
    let raw_theme = configured_html_theme(args.theme.as_deref(), &config).to_string();

    if !src_dir.is_dir() {
        return Err(anyhow!("source directory not found: {}", src_dir.display()));
    }

    let calepin_config = CalepinConfig::load(&src_dir, Some(&config_path))?;
    let config_dir = config_path.parent().unwrap_or(&current_dir);
    let theme = resolve_html_theme_ref(config_dir, &raw_theme);
    let theme_dir = html_theme_dir(&theme);
    let theme_stylesheet_path =
        (theme == DEFAULT_WEBSITE_THEME).then(|| PathBuf::from(WEBSITE_STYLESHEET_PATH));
    let (section_plans, mut typ_files) = discover_pages(&src_dir, config.sidebar.as_ref())?;
    let fallback = src_dir.join(FALLBACK_PAGE);
    if fallback.is_file() {
        typ_files.push(fallback.clone());
    }
    typ_files.sort_by_key(|path| rel_posix(&src_dir, path));
    typ_files.dedup();
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
    let preprocessed = preprocess_documents(
        &build_set,
        &config_path,
        args.quiet,
        args.timeout,
        &args.params,
        args.parallelism,
    )?;

    let page_meta = load_page_meta(&typ_files);
    let nav_sections = nav_from_plans(&src_dir, &section_plans, &page_meta);
    let nav_signature = navigation_signature(&nav_sections);
    let metadata = SiteMetadata::from_config(&config);
    let sitemap_path = metadata
        .base_url
        .as_ref()
        .map(|_| out_dir.join("sitemap.xml"));
    let pdf_files = pdf_enabled_files(&typ_files, &page_meta, args.render_pdf, config.pdf);
    let pages_index = build_pages_index(&src_dir, &typ_files, &section_plans, &page_meta, &pdf_files);
    let pages_index_json = serde_json::to_string_pretty(&pages_index)?;
    let pages_signature = xxh3_64(pages_index_json.as_bytes());
    write_pages_index(&typ_files, &pages_index_json)?;
    let expected_outputs = expected_generated_outputs(
        &src_dir,
        &out_dir,
        &typ_files,
        &pdf_files,
        &sitemap_path,
        theme_stylesheet_path.as_deref(),
    );
    let previous_manifest = load_manifest(&out_dir)?;

    if args.clean {
        clear_previous_outputs(&src_dir, &out_dir)?;
    }
    reconcile_manifest_outputs(&out_dir, &previous_manifest, &expected_outputs)?;
    if args.clean {
        remove_unexpected_rendered_outputs(&out_dir, &expected_outputs)?;
    }
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;
    if let Some(path) = theme_stylesheet_path.as_deref() {
        write_html_theme_stylesheet(&theme, &out_dir, path)?;
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
    let site = SiteModel::new(nav_sections, metadata.clone());
    render_documents(
        &BuildContext {
            src_dir: src_dir.clone(),
            out_dir: out_dir.clone(),
            typst: calepin_config.executables.typst,
            theme,
            pdf_files,
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
        .filter(|path| **path != fallback)
        .map(|path| rel_html_path(&src_dir, path))
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
    let Some(first) = rel.components().next() else {
        return false;
    };
    if first
        .as_os_str()
        .to_str()
        .is_some_and(|name| SKIP_DIRS.contains(&name))
    {
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
        || next.page_fingerprints.keys().ne(current.page_fingerprints.keys())
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
        if let Some(title) = &section.title {
            bytes.extend_from_slice(title.as_bytes());
        }
        bytes.push(0);
        for item in &section.items {
            bytes.extend_from_slice(item.href.as_bytes());
            bytes.push(0);
            bytes.extend_from_slice(item.label.as_bytes());
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
#[serde(default)]
struct WebsiteConfig {
    html_theme: Option<String>,
    // Backward-compatible aliases for older website configs.
    theme: Option<String>,
    template: Option<String>,
    title: Option<String>,
    description: Option<String>,
    base_url: Option<String>,
    logo: Option<String>,
    logo_alt: Option<String>,
    home: Option<String>,
    github_url: Option<String>,
    /// Also render a PDF for every page; pages can override with `pdf` in
    /// their `<website-metadata>`.
    pdf: Option<bool>,
    sidebar: Option<SidebarConfig>,
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
    let default = cli_render_pdf.unwrap_or_else(|| config_pdf.unwrap_or(true));
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

fn configured_html_theme<'a>(cli_theme: Option<&'a str>, config: &'a WebsiteConfig) -> &'a str {
    cli_theme
        .or(config.html_theme.as_deref())
        .or(config.theme.as_deref())
        .or(config.template.as_deref())
        .unwrap_or(DEFAULT_WEBSITE_THEME)
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct SidebarConfig {
    show_hidden: bool,
    section: Vec<SidebarSectionConfig>,
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
    path: Option<PathBuf>,
    glob: Option<String>,
    label: Option<String>,
}

#[derive(Debug, Clone)]
struct NavSectionModel {
    title: Option<String>,
    items: Vec<NavItemModel>,
}

#[derive(Debug, Clone)]
struct NavItemModel {
    href: String,
    label: String,
}

#[derive(Debug, Clone, Default)]
struct SiteMetadata {
    title: Option<String>,
    description: Option<String>,
    base_url: Option<String>,
    logo: Option<String>,
    logo_alt: Option<String>,
    home: Option<String>,
    github_url: Option<String>,
}

impl SiteMetadata {
    fn from_config(config: &WebsiteConfig) -> Self {
        Self {
            title: clean_optional_string(config.title.as_deref()),
            description: clean_optional_string(config.description.as_deref()),
            base_url: clean_optional_string(config.base_url.as_deref())
                .map(|url| url.trim_end_matches('/').to_string()),
            logo: clean_optional_string(config.logo.as_deref()),
            logo_alt: clean_optional_string(config.logo_alt.as_deref())
                .or_else(|| clean_optional_string(config.title.as_deref())),
            home: clean_optional_string(config.home.as_deref())
                .or_else(|| Some("index.html".to_string())),
            github_url: clean_optional_string(config.github_url.as_deref()),
        }
    }
}

#[derive(Debug)]
struct SiteModel {
    sections: Vec<NavSectionModel>,
    metadata: SiteMetadata,
}

impl SiteModel {
    fn new(sections: Vec<NavSectionModel>, metadata: SiteMetadata) -> Self {
        Self { sections, metadata }
    }

    fn theme_context(&self, current_href: &str) -> SiteContextInput {
        let mut nav = Vec::new();
        let mut nav_sections = Vec::new();
        let mut page_title = None;

        for section in &self.sections {
            let mut items = Vec::new();
            for item in &section.items {
                if item.href == current_href {
                    page_title = Some(html_escape(&item.label));
                }
                let entry = SiteNavEntry {
                    href: html_escape(&item.href),
                    label: html_escape(&item.label),
                    active: item.href == current_href,
                };
                nav.push(entry.clone());
                items.push(entry);
            }
            nav_sections.push(SiteNavSection {
                title: section.title.as_ref().map(|title| html_escape(title)),
                items,
            });
        }

        SiteContextInput {
            nav,
            nav_sections,
            title: self.metadata.title.as_deref().map(html_escape),
            description: self.metadata.description.as_deref().map(html_escape),
            base_url: self.metadata.base_url.as_deref().map(html_escape),
            logo: self
                .metadata
                .logo
                .as_deref()
                .map(|logo| html_escape(&page_relative_url(current_href, logo))),
            logo_alt: self.metadata.logo_alt.as_deref().map(html_escape),
            home_url: self
                .metadata
                .home
                .as_deref()
                .map(|home| html_escape(&page_relative_url(current_href, home))),
            github_url: self.metadata.github_url.as_deref().map(html_escape),
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

#[derive(Debug, Deserialize, Serialize, Default)]
struct WebsiteManifest {
    outputs: Vec<String>,
}

#[derive(Clone)]
struct BuildContext {
    src_dir: PathBuf,
    out_dir: PathBuf,
    typst: PathBuf,
    theme: String,
    pdf_files: BTreeSet<PathBuf>,
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

fn resolve_html_theme_ref(config_dir: &Path, value: &str) -> String {
    if is_builtin_html_theme(value) {
        return value.to_string();
    }
    if is_theme_path_like(value) {
        let path = Path::new(value);
        return if path.is_absolute() {
            path.to_path_buf()
        } else {
            config_dir.join(path)
        }
        .to_string_lossy()
        .to_string();
    }
    value.to_string()
}

fn html_theme_dir(value: &str) -> Option<PathBuf> {
    let path = Path::new(value);
    path.is_dir().then(|| path.to_path_buf())
}

/// Plan for one sidebar entry: the page and its explicitly configured label,
/// resolved before metadata is available.
#[derive(Debug, Clone)]
struct NavItemPlan {
    path: PathBuf,
    configured_label: Option<String>,
}

#[derive(Debug, Clone)]
struct NavSectionPlan {
    title: Option<String>,
    items: Vec<NavItemPlan>,
}

fn discover_pages(
    src_dir: &Path,
    sidebar: Option<&SidebarConfig>,
) -> Result<(Vec<NavSectionPlan>, Vec<PathBuf>)> {
    let Some(sidebar) = sidebar else {
        let files = iter_typ_files(src_dir, false, &[PathBuf::from(FALLBACK_PAGE)])?;
        let items = files
            .iter()
            .map(|path| NavItemPlan {
                path: path.clone(),
                configured_label: None,
            })
            .collect();
        return Ok((vec![NavSectionPlan { title: None, items }], files));
    };

    let all_typ_files = iter_typ_files(
        src_dir,
        sidebar.show_hidden,
        &[PathBuf::from(FALLBACK_PAGE)],
    )?;
    let mut used = BTreeSet::new();
    let mut sections = Vec::new();
    let mut build_files = Vec::new();

    for section_config in &sidebar.section {
        let mut items = Vec::new();
        for item_config in &section_config.item {
            let candidates = resolve_file_list(src_dir, item_config, &all_typ_files)?;
            let configured_label = item_config
                .label
                .as_deref()
                .map(str::trim)
                .filter(|label| !label.is_empty());
            for path in candidates {
                if !used.insert(path.clone()) {
                    continue;
                }
                items.push(NavItemPlan {
                    path: path.clone(),
                    configured_label: configured_label.map(str::to_string),
                });
                build_files.push(path);
            }
        }
        sections.push(NavSectionPlan {
            title: section_config.title.clone(),
            items,
        });
    }

    Ok((sections, build_files))
}

fn nav_from_plans(
    src_dir: &Path,
    sections: &[NavSectionPlan],
    page_meta: &PageMetaMap,
) -> Vec<NavSectionModel> {
    let is_hidden = |path: &PathBuf| page_meta.get(path).is_some_and(|meta| meta.hidden);
    sections
        .iter()
        .map(|section| NavSectionModel {
            title: section.title.clone(),
            items: section
                .items
                .iter()
                .filter(|item| !is_hidden(&item.path))
                .map(|item| NavItemModel {
                    href: rel_html_path(src_dir, &item.path),
                    label: item
                        .configured_label
                        .clone()
                        .or_else(|| page_meta.get(&item.path).and_then(|meta| meta.title.clone()))
                        .unwrap_or_else(|| stem_label(&item.path)),
                })
                .collect(),
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
    pdf_files: &BTreeSet<PathBuf>,
) -> serde_json::Value {
    let configured_labels = sections
        .iter()
        .flat_map(|section| section.items.iter())
        .filter_map(|item| {
            item.configured_label
                .as_deref()
                .map(|label| (&item.path, label))
        })
        .collect::<BTreeMap<_, _>>();
    let fallback = src_dir.join(FALLBACK_PAGE);
    let entries = typ_files
        .iter()
        .filter(|path| **path != fallback)
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
                "href": rel_html_path(src_dir, path),
                "title": title,
                "pdf": pdf_files
                    .contains(path)
                    .then(|| rel_output_path(src_dir, path, "pdf")),
                "meta": raw,
            })
        })
        .collect::<Vec<_>>();
    serde_json::Value::Array(entries)
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

fn resolve_file_list(
    src_dir: &Path,
    item: &SidebarItemConfig,
    all_typ_files: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    if let Some(path) = &item.path {
        let candidate = src_dir.join(path);
        if candidate.is_file() && candidate.extension().and_then(|ext| ext.to_str()) == Some("typ")
        {
            return Ok(vec![candidate]);
        }
        cwarn!(
            "sidebar item path does not exist or is not a .typ file: {}",
            path.display()
        );
        return Ok(Vec::new());
    }

    if let Some(pattern) = &item.glob {
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
fn load_page_meta(typ_files: &[PathBuf]) -> PageMetaMap {
    typ_files
        .iter()
        .map(|path| {
            let meta = read_page_meta(path)
                .map(|value| page_meta_from_value(&value))
                .unwrap_or_default();
            (path.clone(), meta)
        })
        .collect()
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
        hidden: value
            .get("hidden")
            .and_then(|hidden| hidden.as_bool())
            .unwrap_or(false),
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

fn preprocess_documents(
    typ_files: &[PathBuf],
    config_path: &Path,
    quiet: bool,
    timeout: Option<u64>,
    params: &[String],
    parallelism: Option<usize>,
) -> Result<BTreeMap<PathBuf, PreprocessOutput>> {
    let outputs = run_parallel(typ_files.to_vec(), parallelism, |input| {
        preprocess_cached(PreprocessOptions {
            input: input.to_path_buf(),
            config: Some(config_path.to_path_buf()),
            quiet,
            timeout,
            sync_pages: false,
            param_overrides: params.to_vec(),
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
    let rel_output = input_path
        .strip_prefix(&context.src_dir)
        .unwrap_or(input_path)
        .with_extension("");
    let html_rel = rel_output.with_extension("html");
    let html_output = context.out_dir.join(&html_rel);
    if let Some(parent) = html_output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let current_href = html_rel.to_string_lossy().replace('\\', "/");
    let mut site_context = site.theme_context(&current_href);
    if let Some(stylesheet) = context.theme_stylesheet.as_deref() {
        site_context.stylesheet = Some(html_escape(&page_relative_url(&current_href, stylesheet)));
    }
    compile_with_typst(
        &context.typst,
        &preprocessed.layout,
        CompileOptions {
            output: Some(html_output.clone()),
            format: Some("html"),
            typst_args: &context.typst_args,
            html_theme: Some(&context.theme),
            site_context: Some(&site_context),
            pages_input: Some(PAGES_INDEX_REF),
            current_href_input: Some(&current_href),
        },
    )?;
    embed_source_blob(&html_output, input_path)?;

    if context.pdf_files.contains(input_path) {
        let pdf_output = context.out_dir.join(rel_output.with_extension("pdf"));
        compile_with_typst(
            &context.typst,
            &preprocessed.layout,
            CompileOptions {
                output: Some(pdf_output),
                format: Some("pdf"),
                typst_args: &context.typst_args,
                html_theme: None,
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
    src_dir: &Path,
    out_dir: &Path,
    typ_files: &[PathBuf],
    pdf_files: &BTreeSet<PathBuf>,
    sitemap_path: &Option<PathBuf>,
    theme_stylesheet_path: Option<&Path>,
) -> BTreeSet<PathBuf> {
    let mut outputs = BTreeSet::new();
    for input_path in typ_files {
        let rel = input_path.strip_prefix(src_dir).unwrap_or(input_path);
        outputs.insert(out_dir.join(rel).with_extension("html"));
        if pdf_files.contains(input_path) {
            outputs.insert(out_dir.join(rel).with_extension("pdf"));
        }
    }
    if let Some(path) = sitemap_path {
        outputs.insert(path.clone());
    }
    if let Some(path) = theme_stylesheet_path {
        outputs.insert(out_dir.join(path));
    }
    outputs
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

/// Writes the sitemap from every built page except the 404 page. Pages with
/// `hidden: true` metadata stay out of the navigation but remain indexed.
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
        for input_path in iter_typ_files(src_dir, true, &[])? {
            let rel = input_path.strip_prefix(src_dir).unwrap_or(&input_path);
            for extension in ["html", "pdf"] {
                let output = out_dir.join(rel).with_extension(extension);
                if output.exists() {
                    fs::remove_file(&output)
                        .with_context(|| format!("failed to remove {}", output.display()))?;
                }
            }
        }
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

fn rel_html_path(src_dir: &Path, path: &Path) -> String {
    rel_output_path(src_dir, path, "html")
}

fn rel_output_path(src_dir: &Path, path: &Path, extension: &str) -> String {
    path.strip_prefix(src_dir)
        .unwrap_or(path)
        .with_extension(extension)
        .to_string_lossy()
        .replace('\\', "/")
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
    let parent_depth = current_href
        .split('/')
        .filter(|part| !part.is_empty())
        .count()
        .saturating_sub(1);
    if parent_depth == 0 {
        target.to_string()
    } else {
        format!("{}{}", "../".repeat(parent_depth), target)
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

const WEBSITE_TOML_TEMPLATE: &str = include_str!("../assets/scaffolds/website/website.toml");
const INDEX_TYP_TEMPLATE: &str = include_str!("../assets/scaffolds/website/docs/index.typ");
const NOT_FOUND_TYP_TEMPLATE: &str = include_str!("../assets/scaffolds/website/docs/404.typ");

#[cfg(test)]
mod tests {
    use super::*;

    fn test_build_result(root: &Path, pages: &[PathBuf]) -> WebsiteBuildResult {
        WebsiteBuildResult {
            src_dir: root.to_path_buf(),
            out_dir: root.to_path_buf(),
            config_path: root.join("website.toml"),
            theme_dir: None,
            page_fingerprints: fingerprint_files(pages).unwrap(),
            nav_signature: 0,
            pages_signature: 0,
        }
    }

    #[test]
    fn configured_html_theme_prefers_explicit_html_theme_and_keeps_aliases() {
        let config = WebsiteConfig {
            html_theme: Some("html-theme".to_string()),
            theme: Some("legacy-theme".to_string()),
            template: Some("legacy-template".to_string()),
            ..WebsiteConfig::default()
        };

        assert_eq!(configured_html_theme(None, &config), "html-theme");
        assert_eq!(
            configured_html_theme(Some("cli-theme"), &config),
            "cli-theme"
        );

        let legacy_config = WebsiteConfig {
            theme: Some("legacy-theme".to_string()),
            template: Some("legacy-template".to_string()),
            ..WebsiteConfig::default()
        };
        assert_eq!(configured_html_theme(None, &legacy_config), "legacy-theme");

        let template_config = WebsiteConfig {
            template: Some("legacy-template".to_string()),
            ..WebsiteConfig::default()
        };
        assert_eq!(
            configured_html_theme(None, &template_config),
            "legacy-template"
        );
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
        let hrefs = BTreeSet::from([
            "index.html".to_string(),
            "guide/usage.html".to_string(),
        ]);

        write_sitemap(temp.path(), Some("https://example.com/project/"), &hrefs).unwrap();

        let sitemap = fs::read_to_string(temp.path().join("sitemap.xml")).unwrap();
        assert!(sitemap.contains("<loc>https://example.com/project/index.html</loc>"));
        assert!(sitemap.contains("<loc>https://example.com/project/guide/usage.html</loc>"));
    }

    #[test]
    fn theme_context_rewrites_brand_urls_relative_to_current_page() {
        let site = SiteModel::new(
            vec![NavSectionModel {
                title: Some("Guide".to_string()),
                items: vec![NavItemModel {
                    href: "guide/usage.html".to_string(),
                    label: "Usage".to_string(),
                }],
            }],
            SiteMetadata {
                title: Some("Example".to_string()),
                description: None,
                base_url: None,
                logo: Some("assets/logo.svg".to_string()),
                logo_alt: Some("Example".to_string()),
                home: Some("index.html".to_string()),
                github_url: None,
            },
        );

        let context = site.theme_context("guide/usage.html");

        assert_eq!(context.logo.as_deref(), Some("../assets/logo.svg"));
        assert_eq!(context.home_url.as_deref(), Some("../index.html"));
        assert_eq!(context.logo_alt.as_deref(), Some("Example"));
        assert_eq!(context.stylesheet, None);
    }

    #[test]
    fn page_relative_url_rewrites_generated_stylesheet_for_nested_pages() {
        assert_eq!(
            page_relative_url("guide/usage.html", WEBSITE_STYLESHEET_PATH),
            "../.calepin/calepin-website.css"
        );
        assert_eq!(
            page_relative_url("index.html", WEBSITE_STYLESHEET_PATH),
            ".calepin/calepin-website.css"
        );
    }

    #[test]
    fn resolve_html_theme_ref_accepts_builtin_name_or_theme_directory_path() {
        let temp = tempfile::tempdir().unwrap();

        assert_eq!(
            resolve_html_theme_ref(temp.path(), "calepin-website"),
            "calepin-website"
        );
        assert_eq!(
            resolve_html_theme_ref(temp.path(), "themes/my-theme"),
            temp.path()
                .join("themes/my-theme")
                .to_string_lossy()
                .to_string()
        );
    }

    #[test]
    fn nav_from_plans_prefers_configured_label_then_metadata_title_then_stem() {
        let src = Path::new("/site/docs");
        let labeled = PathBuf::from("/site/docs/a.typ");
        let titled = PathBuf::from("/site/docs/b-page.typ");
        let bare = PathBuf::from("/site/docs/c_page.typ");
        let sections = vec![NavSectionPlan {
            title: Some("Guide".to_string()),
            items: vec![
                NavItemPlan {
                    path: labeled.clone(),
                    configured_label: Some("Configured".to_string()),
                },
                NavItemPlan {
                    path: titled.clone(),
                    configured_label: None,
                },
                NavItemPlan {
                    path: bare,
                    configured_label: None,
                },
            ],
        }];
        let meta = PageMetaMap::from([
            (
                labeled,
                PageMeta {
                    title: Some("Ignored".to_string()),
                    ..PageMeta::default()
                },
            ),
            (
                titled,
                PageMeta {
                    title: Some("From Metadata".to_string()),
                    ..PageMeta::default()
                },
            ),
        ]);

        let nav = nav_from_plans(src, &sections, &meta);

        let labels = nav[0]
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>();
        assert_eq!(labels, vec!["Configured", "From Metadata", "c page"]);
    }

    #[test]
    fn page_meta_from_value_reads_calepin_keys_and_keeps_raw_dict() {
        let value = serde_json::json!({"title": " My Page ", "pdf": false, "date": "2026-06-10"});
        let meta = page_meta_from_value(&value);
        assert_eq!(meta.title.as_deref(), Some("My Page"));
        assert_eq!(meta.pdf, Some(false));
        assert!(!meta.hidden);
        assert_eq!(meta.raw, value);

        let blank_title = page_meta_from_value(&serde_json::json!({"title": ""}));
        assert_eq!(blank_title.title, None);

        let not_a_dict = page_meta_from_value(&serde_json::json!("not a dict"));
        assert_eq!(not_a_dict.raw, serde_json::json!({}));

        let hidden = page_meta_from_value(&serde_json::json!({"hidden": true}));
        assert!(hidden.hidden);
    }

    #[test]
    fn build_pages_index_resolves_titles_and_excludes_fallback_page() {
        let src = Path::new("/site/docs");
        let post = PathBuf::from("/site/docs/blog/first.typ");
        let home = PathBuf::from("/site/docs/index.typ");
        let fallback = PathBuf::from("/site/docs/404.typ");
        let typ_files = vec![fallback, post.clone(), home.clone()];
        let sections = vec![NavSectionPlan {
            title: None,
            items: vec![NavItemPlan {
                path: home.clone(),
                configured_label: Some("Home".to_string()),
            }],
        }];
        let raw = serde_json::json!({"title": "First Post", "date": "2026-06-10", "hidden": true});
        let meta = PageMetaMap::from([(post.clone(), page_meta_from_value(&raw))]);
        let pdf_files = BTreeSet::from([post.clone()]);

        let index = build_pages_index(src, &typ_files, &sections, &meta, &pdf_files);

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
    fn nav_from_plans_drops_hidden_pages() {
        let src = Path::new("/site/docs");
        let visible = PathBuf::from("/site/docs/index.typ");
        let hidden = PathBuf::from("/site/docs/blog/post.typ");
        let sections = vec![NavSectionPlan {
            title: None,
            items: vec![
                NavItemPlan {
                    path: visible.clone(),
                    configured_label: None,
                },
                NavItemPlan {
                    path: hidden.clone(),
                    configured_label: None,
                },
            ],
        }];
        let meta = PageMetaMap::from([(
            hidden,
            page_meta_from_value(&serde_json::json!({"hidden": true})),
        )]);

        let nav = nav_from_plans(src, &sections, &meta);

        assert_eq!(nav[0].items.len(), 1);
        assert_eq!(nav[0].items[0].href, "index.html");
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
        assert_eq!(with_default, BTreeSet::from([on.clone(), plain]));

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
}
