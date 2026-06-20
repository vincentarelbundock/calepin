use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use xxhash_rust::xxh3::xxh3_64;

use crate::html::{html_theme_script, html_theme_stylesheet, minify_html_file, HtmlSyntaxTheme};
use crate::typst::compile::{compile_with_typst, CompileOptions};
use crate::typst::paths::project_relative_path;
use crate::typst::preprocess::PreprocessOutput;
use crate::utils::html::escape as html_escape;

use super::preprocess::run_parallel;
use super::site::SiteModel;
use super::url::page_relative_url;
use super::{BuildContext, PAGES_INDEX_REF, SOURCE_DATA_ID, WEBSITE_ASSET_DIR, WEBSITE_ASSET_STEM};

pub(super) fn render_documents(
    context: &BuildContext,
    typ_files: Vec<PathBuf>,
    site: &SiteModel,
    preprocessed: &BTreeMap<PathBuf, PreprocessOutput>,
) -> Result<()> {
    let progress = context
        .progress
        .bar("[render] pages", typ_files.len() as u64);
    run_parallel(
        typ_files,
        context.parallelism,
        Some(&progress),
        |input_path| {
            let rel = project_relative_path(&context.src_dir, &input_path);
            let page_progress = context.progress.spinner(format!("[render] {rel}"));
            render_document(context, site, &input_path, preprocessed)
                .with_context(|| format!("failed to render {}", input_path.display()))?;
            page_progress.finish(format!("[done] render {rel}"));
            Ok(())
        },
    )?;
    progress.finish("[done] render pages");
    Ok(())
}

pub(super) fn default_site_html_entry() -> crate::theme::HtmlEntry {
    crate::theme::resolve_html_entry(
        &crate::theme::ThemeSelection::Default,
        crate::theme::HtmlScope::Site,
    )
    .expect("default theme must resolve")
    .expect("default theme must provide a site entry")
}

pub(super) fn html_entry_with_config_styles(
    entry: Option<crate::theme::HtmlEntry>,
    styles: &[crate::config::CssOverride],
) -> Option<crate::theme::HtmlEntry> {
    match entry {
        Some(mut entry) => {
            entry.append_styles(styles.to_vec());
            Some(entry)
        }
        None if !styles.is_empty() => Some(crate::theme::style_only_html_entry(styles.to_vec())),
        None => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HtmlEntryAssetKey {
    styles: Vec<(String, String)>,
    scripts: Vec<(String, String)>,
}

impl From<&crate::theme::HtmlEntry> for HtmlEntryAssetKey {
    fn from(entry: &crate::theme::HtmlEntry) -> Self {
        Self {
            styles: entry.styles.clone(),
            scripts: entry.scripts.clone(),
        }
    }
}

pub(super) struct PageAssetDecision {
    pub(super) html_entry: Option<crate::theme::HtmlEntry>,
    pub(super) stylesheet: Option<String>,
    pub(super) scripts: Vec<String>,
}

pub(super) fn page_asset_decision(
    page_entry: Option<crate::theme::HtmlEntry>,
    config_styles: &[crate::config::CssOverride],
    generated_entry: Option<&crate::theme::HtmlEntry>,
    generated_stylesheet: Option<&str>,
    generated_scripts: &[String],
) -> PageAssetDecision {
    let styled_page_entry = html_entry_with_config_styles(page_entry.clone(), config_styles);
    let page_references_site_stylesheet = styled_page_entry
        .as_ref()
        .is_some_and(html_entry_references_site_stylesheet);
    let matches_generated_entry =
        styled_page_entry
            .as_ref()
            .zip(generated_entry)
            .is_some_and(|(page, generated)| {
                HtmlEntryAssetKey::from(page) == HtmlEntryAssetKey::from(generated)
            });
    let stylesheet = if matches_generated_entry && page_references_site_stylesheet {
        generated_stylesheet.map(str::to_string)
    } else {
        None
    };
    let html_entry = if stylesheet.is_some() {
        page_entry.or_else(|| Some(crate::theme::style_only_html_entry(Vec::new())))
    } else {
        styled_page_entry
    };
    let scripts = if matches_generated_entry {
        generated_scripts.to_vec()
    } else {
        Vec::new()
    };
    PageAssetDecision {
        html_entry,
        stylesheet,
        scripts,
    }
}

fn html_entry_references_site_stylesheet(entry: &crate::theme::HtmlEntry) -> bool {
    let partials = entry.partials.iter().cloned().collect::<BTreeMap<_, _>>();
    let mut visited = BTreeSet::new();
    html_source_references_site_stylesheet(&entry.layout, &partials, &mut visited)
}

fn html_source_references_site_stylesheet(
    source: &str,
    partials: &BTreeMap<String, String>,
    visited: &mut BTreeSet<String>,
) -> bool {
    if source.contains("site.stylesheet") {
        return true;
    }
    static_html_includes(source).into_iter().any(|name| {
        visited.insert(name.clone())
            && partials.get(&name).is_some_and(|partial| {
                html_source_references_site_stylesheet(partial, partials, visited)
            })
    })
}

fn static_html_includes(source: &str) -> Vec<String> {
    let mut includes = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find("{%") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find("%}") else {
            break;
        };
        let block = rest[..end].trim().trim_matches('-').trim();
        if let Some(include) = static_include_name(block) {
            includes.push(include);
        }
        rest = &rest[end + 2..];
    }
    includes
}

fn static_include_name(block: &str) -> Option<String> {
    let rest = block.strip_prefix("include")?.trim_start();
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let quoted = &rest[quote.len_utf8()..];
    let end = quoted.find(quote)?;
    Some(quoted[..end].to_string())
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
    let page_meta = context.page_meta.get(input_path);
    let mut site_context = site.theme_context(
        &current_href,
        Some(page_info),
        &context.page_info,
        context.languages.as_deref(),
        context.search,
    );
    site_context.revealjs = context.revealjs_options.clone();
    let page_site_entry = if let Some(layout) = page_meta.and_then(|meta| meta.layout.as_deref()) {
        crate::theme::resolve_explicit_site_html_entry(&preprocessed.theme, layout)?
    } else {
        crate::theme::resolve_html_entry(&preprocessed.theme, crate::theme::HtmlScope::Site)?
    };
    let asset_decision = page_asset_decision(
        page_site_entry,
        &context.config_styles,
        context.generated_theme_entry.as_ref(),
        context.theme_stylesheet.as_deref(),
        &context.theme_scripts,
    );
    let page_site_entry = asset_decision.html_entry;
    if let Some(stylesheet) = asset_decision.stylesheet.as_deref() {
        site_context.stylesheet = Some(html_escape(&page_relative_url(&current_href, stylesheet)));
    }
    site_context.scripts = asset_decision
        .scripts
        .iter()
        .map(|script| html_escape(&page_relative_url(&current_href, script)))
        .collect();
    compile_with_typst(
        &context.typst,
        &preprocessed.layout,
        CompileOptions {
            output: Some(html_output.clone()),
            format: Some("html"),
            typst_args: &context.typst_args,
            theme: &preprocessed.theme,
            html_scope: crate::theme::HtmlScope::Site,
            html_entry: page_site_entry.as_ref(),
            config_styles: &[],
            html_syntax_theme: Some(&context.syntax_theme),
            site_context: Some(&site_context),
            pages_input: Some(PAGES_INDEX_REF),
            current_href_input: Some(&current_href),
            minify_html: false,
            progress: false,
        },
    )?;
    // Publishes the complete page source for the runtime view-source feature.
    // Authors should treat comments and code chunks in site pages as public.
    embed_source_blob(&html_output, input_path)?;
    if context.minify_html {
        minify_html_file(&html_output)?;
    }

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
                html_entry: None,
                config_styles: &[],
                html_syntax_theme: None,
                site_context: None,
                pages_input: Some(PAGES_INDEX_REF),
                current_href_input: Some(&current_href),
                minify_html: false,
                progress: false,
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

#[derive(Debug, Clone, Default)]
pub(super) struct ThemeGeneratedAssets {
    pub(super) stylesheet: Option<GeneratedThemeAsset>,
    pub(super) script: Option<GeneratedThemeAsset>,
}

#[derive(Debug, Clone)]
pub(super) struct GeneratedThemeAsset {
    pub(super) rel_path: PathBuf,
    pub(super) content: String,
}

impl ThemeGeneratedAssets {
    pub(super) fn from_entry(
        entry: &crate::theme::HtmlEntry,
        syntax_theme: &HtmlSyntaxTheme,
    ) -> Result<Self> {
        let stylesheet = html_theme_stylesheet(entry, syntax_theme)?
            .map(|content| GeneratedThemeAsset::new(WEBSITE_ASSET_STEM, "css", content));
        let script = html_theme_script(entry)
            .map(|content| GeneratedThemeAsset::new(WEBSITE_ASSET_STEM, "js", content));
        Ok(Self { stylesheet, script })
    }

    pub(super) fn output_paths(&self, out_dir: &Path) -> BTreeSet<PathBuf> {
        [&self.stylesheet, &self.script]
            .into_iter()
            .filter_map(|asset| asset.as_ref())
            .map(|asset| out_dir.join(&asset.rel_path))
            .collect()
    }

    pub(super) fn write(&self, out_dir: &Path) -> Result<()> {
        for asset in [&self.stylesheet, &self.script]
            .into_iter()
            .filter_map(|asset| asset.as_ref())
        {
            asset.write(out_dir)?;
        }
        Ok(())
    }
}

impl GeneratedThemeAsset {
    fn new(stem: &str, extension: &str, content: String) -> Self {
        let hash = xxh3_64(content.as_bytes());
        Self {
            rel_path: PathBuf::from(WEBSITE_ASSET_DIR)
                .join(format!("{stem}.{hash:016x}.{extension}")),
            content,
        }
    }

    pub(super) fn write(&self, out_dir: &Path) -> Result<()> {
        let path = out_dir.join(&self.rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&path, &self.content)
            .with_context(|| format!("failed to write {}", path.display()))
    }
}
