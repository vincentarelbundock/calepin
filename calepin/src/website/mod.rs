mod serve;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
use crate::html::{SiteContextInput, SiteNavEntry, SiteNavSection};
use crate::typst::compile::{compile_with_typst, CompileOptions};
use crate::typst::preprocess::{preprocess, PreprocessOptions};

const DEFAULT_CONFIG: &str = "website.toml";
const DEFAULT_SRC_DIR: &str = "docs";
const DEFAULT_TEMPLATE: &str = "calepin-website";
const FALLBACK_PAGE: &str = "404.typ";
const SITE_METADATA_LABEL: &str = "website-metadata";
const SOURCE_DATA_ID: &str = "calepin-website-source-data";
const MANIFEST_PATH: &str = ".calepin/website-manifest.json";

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
        None => true,
        Some(CompileFormat::Html) => false,
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
        template: args.template,
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
        template: None,
        parallelism: None,
        render_pdf: true,
        quiet: args.common.quiet,
        timeout: args.common.timeout,
        params: args.common.params.clone(),
        typst_args: args.typst_args.clone(),
        incremental_inputs: None,
        clean: true,
    };
    let initial = build_site(options.clone())?;
    let reload_version = Arc::new(AtomicU64::new(1));
    let server = if args.serve {
        Some(serve::start(
            &initial.out_dir,
            &args.host,
            args.port,
            Arc::clone(&reload_version),
        )?)
    } else {
        None
    };

    let result = watch_site(options, initial, reload_version, args.common.quiet);
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
    template: Option<String>,
    parallelism: Option<usize>,
    render_pdf: bool,
    quiet: bool,
    timeout: Option<u64>,
    params: Vec<String>,
    typst_args: Vec<String>,
    incremental_inputs: Option<Vec<PathBuf>>,
    clean: bool,
}

#[derive(Debug, Clone)]
struct WebsiteBuildResult {
    src_dir: PathBuf,
    out_dir: PathBuf,
    config_path: PathBuf,
    themes_dir: PathBuf,
    page_fingerprints: BTreeMap<PathBuf, u64>,
    nav_signature: u64,
}

fn build_site(args: WebsiteBuildOptions) -> Result<WebsiteBuildResult> {
    let current_dir = std::env::current_dir()?;
    let config_path = resolve_config_path(&current_dir, Some(args.config.as_path()))?;
    let config = load_website_config(&config_path, true)?;
    let config_dir = config_path.parent().unwrap_or(&current_dir);

    let src_dir = resolve_configured_path(
        config_dir,
        args.src.as_ref().or(config.src.as_ref()),
        Path::new(DEFAULT_SRC_DIR),
    );
    let out_dir = resolve_configured_path(
        config_dir,
        args.out.as_ref().or(config.out.as_ref()),
        src_dir.as_path(),
    );
    let template = args
        .template
        .as_deref()
        .or(config.template.as_deref())
        .unwrap_or(DEFAULT_TEMPLATE)
        .to_string();

    if !src_dir.is_dir() {
        return Err(anyhow!("source directory not found: {}", src_dir.display()));
    }

    let calepin_config = CalepinConfig::load(&src_dir, Some(&config_path))?;
    let (nav_sections, mut typ_files) = build_navigation(&src_dir, config.sidebar.as_ref())?;
    let fallback = src_dir.join(FALLBACK_PAGE);
    if fallback.is_file() {
        typ_files.push(fallback);
    }
    typ_files.sort_by_key(|path| rel_posix(&src_dir, path));
    typ_files.dedup();
    let page_fingerprints = fingerprint_files(&typ_files)?;
    let nav_signature = navigation_signature(&nav_sections);
    let metadata = SiteMetadata::from_config(&config);
    let sitemap_path = metadata
        .base_url
        .as_ref()
        .map(|_| out_dir.join("sitemap.xml"));
    let expected_outputs = expected_generated_outputs(
        &src_dir,
        &out_dir,
        &typ_files,
        args.render_pdf,
        &sitemap_path,
    );
    let previous_manifest = load_manifest(&out_dir)?;

    if let Some(inputs) = &args.incremental_inputs {
        let wanted = inputs.iter().cloned().collect::<BTreeSet<_>>();
        typ_files.retain(|path| wanted.contains(path));
    }

    if args.clean {
        clear_previous_outputs(&src_dir, &out_dir)?;
    }
    reconcile_manifest_outputs(&out_dir, &previous_manifest, &expected_outputs)?;
    if args.clean {
        remove_unexpected_rendered_outputs(&out_dir, &expected_outputs)?;
    }
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;

    if out_dir != src_dir {
        if args.incremental_inputs.is_none() {
            copy_assets(&src_dir, &out_dir)?;
        }
        let source_files = if let Some(inputs) = &args.incremental_inputs {
            inputs.clone()
        } else if config.sidebar.is_some() {
            typ_files.clone()
        } else {
            iter_typ_files(&src_dir, false, &[])?
        };
        copy_typ_sources(&src_dir, &out_dir, &source_files)?;
    }

    let site = Arc::new(SiteModel::new(nav_sections, metadata.clone()));
    compile_documents(
        BuildContext {
            src_dir: src_dir.clone(),
            out_dir: out_dir.clone(),
            config_path: config_path.clone(),
            typst: calepin_config.executables.typst,
            template,
            quiet: args.quiet,
            timeout: args.timeout,
            params: args.params,
            render_pdf: args.render_pdf,
            themes_dir: calepin_config.themes_dir.clone(),
            parallelism: args.parallelism,
            typst_args: args.typst_args,
        },
        typ_files,
        Arc::clone(&site),
    )?;
    write_sitemap(&out_dir, metadata.base_url.as_deref(), site.nav_sections())?;
    write_manifest(&out_dir, &expected_outputs)?;
    Ok(WebsiteBuildResult {
        src_dir,
        out_dir,
        config_path,
        themes_dir: calepin_config.themes_dir,
        page_fingerprints,
        nav_signature,
    })
}

fn watch_site(
    options: WebsiteBuildOptions,
    initial: WebsiteBuildResult,
    reload_version: serve::ReloadVersion,
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
    if current.themes_dir.is_dir() {
        debouncer
            .watch(&current.themes_dir, RecursiveMode::Recursive)
            .with_context(|| format!("failed to watch {}", current.themes_dir.display()))?;
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
                        reload_version.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(None) => {}
                    Err(error) => {
                        cwarn!("website rebuild failed: {}", error);
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
    if initial.themes_dir.is_dir() && path.starts_with(&initial.themes_dir) {
        return true;
    }
    if !path.starts_with(&initial.src_dir) {
        return false;
    }
    let rel = path.strip_prefix(&initial.src_dir).unwrap_or(path);
    let Some(first) = rel.components().next() else {
        return false;
    };
    if matches!(
        first.as_os_str().to_str(),
        Some(".calepin" | ".git" | "target" | "node_modules" | ".venv")
    ) {
        return false;
    }
    if path.starts_with(&initial.out_dir) {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("html" | "pdf") => return false,
            _ => {}
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
        let next = build_site(options.clone())?;
        return Ok(Some(next));
    };
    if pages.is_empty() {
        return Ok(None);
    }

    let mut incremental_options = options.clone();
    incremental_options.incremental_inputs = Some(pages);
    incremental_options.clean = false;
    let next = build_site(incremental_options)?;
    if next.nav_signature != current.nav_signature
        || next.page_fingerprints.keys().collect::<Vec<_>>()
            != current.page_fingerprints.keys().collect::<Vec<_>>()
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
    src: Option<PathBuf>,
    out: Option<PathBuf>,
    template: Option<String>,
    title: Option<String>,
    description: Option<String>,
    base_url: Option<String>,
    github_url: Option<String>,
    sidebar: Option<SidebarConfig>,
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
    github_url: Option<String>,
}

impl SiteMetadata {
    fn from_config(config: &WebsiteConfig) -> Self {
        Self {
            title: clean_optional_string(config.title.as_deref()),
            description: clean_optional_string(config.description.as_deref()),
            base_url: clean_optional_string(config.base_url.as_deref())
                .map(|url| url.trim_end_matches('/').to_string()),
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

    fn nav_sections(&self) -> &[NavSectionModel] {
        &self.sections
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
            github_url: self.metadata.github_url.as_deref().map(html_escape),
            current_url: self
                .metadata
                .base_url
                .as_deref()
                .map(|base_url| html_escape(&absolute_site_url(base_url, current_href))),
            page_title,
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
    config_path: PathBuf,
    typst: PathBuf,
    template: String,
    quiet: bool,
    timeout: Option<u64>,
    params: Vec<String>,
    render_pdf: bool,
    themes_dir: PathBuf,
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

fn resolve_configured_path(config_dir: &Path, value: Option<&PathBuf>, default: &Path) -> PathBuf {
    let path = value.map(PathBuf::as_path).unwrap_or(default);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        config_dir.join(path)
    }
}

fn build_navigation(
    src_dir: &Path,
    sidebar: Option<&SidebarConfig>,
) -> Result<(Vec<NavSectionModel>, Vec<PathBuf>)> {
    match sidebar {
        Some(sidebar) => build_manual_navigation(src_dir, sidebar),
        None => build_auto_navigation(src_dir, false),
    }
}

fn build_auto_navigation(
    src_dir: &Path,
    include_hidden: bool,
) -> Result<(Vec<NavSectionModel>, Vec<PathBuf>)> {
    let files = iter_typ_files(src_dir, include_hidden, &[PathBuf::from(FALLBACK_PAGE)])?;
    let items = files
        .iter()
        .map(|path| {
            Ok(NavItemModel {
                href: rel_html_path(src_dir, path),
                label: title_from_typst_file(path)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((vec![NavSectionModel { title: None, items }], files))
}

fn build_manual_navigation(
    src_dir: &Path,
    sidebar: &SidebarConfig,
) -> Result<(Vec<NavSectionModel>, Vec<PathBuf>)> {
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
                let label = match configured_label {
                    Some(label) => label.to_string(),
                    None => title_from_typst_file(&path)?,
                };
                items.push(NavItemModel {
                    href: rel_html_path(src_dir, &path),
                    label,
                });
                build_files.push(path);
            }
        }
        sections.push(NavSectionModel {
            title: section_config.title.clone(),
            items,
        });
    }

    Ok((sections, build_files))
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

fn title_from_typst_file(path: &Path) -> Result<String> {
    Ok(title_from_metadata(path)?.unwrap_or_else(|| {
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .replace(['-', '_'], " ")
    }))
}

fn title_from_metadata(path: &Path) -> Result<Option<String>> {
    let output = Command::new("typst")
        .args([
            "query",
            path.as_os_str().to_string_lossy().as_ref(),
            &format!("label(\"{SITE_METADATA_LABEL}\")"),
            "--field",
            "value",
        ])
        .output();
    let output = match output {
        Ok(output) if output.status.success() => output,
        _ => return Ok(None),
    };
    let values: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(values) => values,
        Err(_) => return Ok(None),
    };
    let title = values
        .as_array()
        .and_then(|values| values.first())
        .and_then(|value| value.get("title"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_string);
    Ok(title)
}

fn compile_documents(
    context: BuildContext,
    typ_files: Vec<PathBuf>,
    site: Arc<SiteModel>,
) -> Result<()> {
    if typ_files.is_empty() {
        return Ok(());
    }
    let worker_count = context
        .parallelism
        .unwrap_or_else(|| {
            thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1)
                .min(32)
        })
        .max(1)
        .min(typ_files.len().max(1));
    let queue = Arc::new(Mutex::new(VecDeque::from(typ_files)));

    thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..worker_count {
            let queue = Arc::clone(&queue);
            let site = Arc::clone(&site);
            let context = context.clone();
            handles.push(scope.spawn(move || -> Result<()> {
                loop {
                    let Some(input_path) = queue.lock().unwrap().pop_front() else {
                        return Ok(());
                    };
                    compile_document(&context, &site, &input_path)
                        .with_context(|| format!("failed to render {}", input_path.display()))?;
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
    })
}

fn compile_document(context: &BuildContext, site: &SiteModel, input_path: &Path) -> Result<()> {
    let rel_output = input_path
        .strip_prefix(&context.src_dir)
        .unwrap_or(input_path)
        .with_extension("");
    let html_output = context.out_dir.join(rel_output.with_extension("html"));
    if let Some(parent) = html_output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let preprocessed = preprocess(PreprocessOptions {
        input: input_path.to_path_buf(),
        config: Some(context.config_path.clone()),
        quiet: context.quiet,
        timeout: context.timeout,
        sync_pages: false,
        param_overrides: context.params.clone(),
    })?;
    let current_href = rel_output
        .with_extension("html")
        .to_string_lossy()
        .replace('\\', "/");
    let site_context = site.theme_context(&current_href);
    compile_with_typst(
        &context.typst,
        &preprocessed.layout,
        CompileOptions {
            output: Some(html_output.clone()),
            format: Some("html"),
            typst_args: &context.typst_args,
            template_theme: Some(&context.template),
            themes_dir: &context.themes_dir,
            site_context: Some(&site_context),
        },
    )?;
    embed_source_blob(&html_output, input_path)?;

    if context.render_pdf {
        let pdf_output = context.out_dir.join(rel_output.with_extension("pdf"));
        compile_with_typst(
            &context.typst,
            &preprocessed.layout,
            CompileOptions {
                output: Some(pdf_output),
                format: Some("pdf"),
                typst_args: &context.typst_args,
                template_theme: None,
                themes_dir: &context.themes_dir,
                site_context: None,
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
    render_pdf: bool,
    sitemap_path: &Option<PathBuf>,
) -> BTreeSet<PathBuf> {
    let mut outputs = BTreeSet::new();
    for input_path in typ_files {
        let rel = input_path.strip_prefix(src_dir).unwrap_or(input_path);
        outputs.insert(out_dir.join(rel).with_extension("html"));
        if render_pdf {
            outputs.insert(out_dir.join(rel).with_extension("pdf"));
        }
    }
    if let Some(path) = sitemap_path {
        outputs.insert(path.clone());
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
        if matches!(
            first.as_os_str().to_str(),
            Some(".calepin" | ".git" | "target" | "node_modules" | ".venv" | "assets")
        ) {
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

fn write_sitemap(
    out_dir: &Path,
    base_url: Option<&str>,
    nav_sections: &[NavSectionModel],
) -> Result<()> {
    let path = out_dir.join("sitemap.xml");
    let Some(base_url) = base_url else {
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove stale sitemap {}", path.display()))?;
        }
        return Ok(());
    };

    let mut hrefs = BTreeSet::new();
    for section in nav_sections {
        for item in &section.items {
            hrefs.insert(item.href.clone());
        }
    }

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
            if path.is_file() && path.file_name().and_then(|name| name.to_str()) != Some(".gitkeep")
            {
                fs::remove_file(&path)
                    .with_context(|| format!("failed to remove {}", path.display()))?;
            } else if path.is_dir() {
                fs::remove_dir_all(&path)
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
    path.strip_prefix(src_dir)
        .unwrap_or(path)
        .with_extension("html")
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
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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

const WEBSITE_TOML_TEMPLATE: &str = r#"src = "docs"
out = "public"
template = "pico"
title = "My Calepin Site"
description = "A website built with Calepin."
# base_url = "https://example.com"
# github_url = "https://github.com/user/repo"

[sidebar]

[[sidebar.section]]
title = "Pages"

  [[sidebar.section.item]]
  path = "index.typ"
  label = "Home"
"#;

const INDEX_TYP_TEMPLATE: &str = r#"#import "@preview/calepin:0.0.1" as calepin

#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
  fenced-chunks: true,
)

= Home

Welcome to your Calepin website.
"#;

const NOT_FOUND_TYP_TEMPLATE: &str = r#"= Not found

The requested page does not exist.
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn test_build_result(root: &Path, pages: &[PathBuf]) -> WebsiteBuildResult {
        WebsiteBuildResult {
            src_dir: root.to_path_buf(),
            out_dir: root.to_path_buf(),
            config_path: root.join("website.toml"),
            themes_dir: root.join("themes"),
            page_fingerprints: fingerprint_files(pages).unwrap(),
            nav_signature: 0,
        }
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
    fn write_sitemap_uses_absolute_navigation_urls() {
        let temp = tempfile::tempdir().unwrap();
        let sections = vec![NavSectionModel {
            title: Some("Guide".to_string()),
            items: vec![
                NavItemModel {
                    href: "index.html".to_string(),
                    label: "Home".to_string(),
                },
                NavItemModel {
                    href: "guide/usage.html".to_string(),
                    label: "Usage".to_string(),
                },
            ],
        }];

        write_sitemap(temp.path(), Some("https://example.com/project/"), &sections).unwrap();

        let sitemap = fs::read_to_string(temp.path().join("sitemap.xml")).unwrap();
        assert!(sitemap.contains("<loc>https://example.com/project/index.html</loc>"));
        assert!(sitemap.contains("<loc>https://example.com/project/guide/usage.html</loc>"));
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
