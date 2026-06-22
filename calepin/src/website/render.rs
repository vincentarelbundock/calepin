use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::html::{minify_html_file, theme_css, HtmlSyntaxTheme};
use crate::typst::compile::{compile_with_typst, CompileOptions, OutputFormat};
use crate::typst::paths::project_relative_path;
use crate::typst::preprocess::PreprocessOutput;
use crate::utils::html::escape as html_escape;

use super::preprocess::run_parallel;
use super::site::SiteModel;
use super::url::page_relative_url;
use super::{paths::normalize_asset_relative_path, BuildContext, SOURCE_DATA_ID};

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
    pub(super) scripts: Vec<String>,
}

pub(super) fn page_asset_decision(
    page_entry: Option<crate::theme::HtmlEntry>,
    generated_entry: Option<&crate::theme::HtmlEntry>,
    generated_scripts: &[String],
) -> PageAssetDecision {
    let matches_generated_entry =
        page_entry
            .as_ref()
            .zip(generated_entry)
            .is_some_and(|(page, generated)| {
                HtmlEntryAssetKey::from(page) == HtmlEntryAssetKey::from(generated)
            });
    let html_entry = page_entry;
    let scripts = if matches_generated_entry {
        generated_scripts.to_vec()
    } else {
        Vec::new()
    };

    PageAssetDecision {
        html_entry,
        scripts,
    }
}

fn ensure_parent_directory(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    Ok(())
}

fn escape_script_payload(payload: &str) -> String {
    let mut payload = payload;
    let mut escaped = String::with_capacity(payload.len());
    while let Some(offset) = find_script_tag(payload) {
        escaped.push_str(&payload[..offset]);
        escaped.push_str("<\\/");
        payload = &payload[offset + 2..];
    }
    escaped.push_str(payload);
    escaped
}

fn find_script_tag(source: &str) -> Option<usize> {
    let source = source.as_bytes();
    if source.len() < 8 {
        return None;
    }
    for i in 0..=source.len() - 8 {
        if source[i] == b'<'
            && source[i + 1] == b'/'
            && bytes_is_ascii_equal_ignore_case(&source[i + 2..i + 8], b"script")
        {
            return Some(i);
        }
    }
    None
}

fn find_case_insensitive(source: &str, needle: &str) -> Option<usize> {
    let source = source.as_bytes();
    let needle = needle.as_bytes();
    if source.len() < needle.len() {
        return None;
    }
    for i in 0..=source.len() - needle.len() {
        if bytes_is_ascii_equal_ignore_case(&source[i..i + needle.len()], needle) {
            return Some(i);
        }
    }
    None
}

fn bytes_is_ascii_equal_ignore_case(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.to_ascii_lowercase() == right.to_ascii_lowercase())
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

    let current_href = page_info.href.clone();
    let page_meta = context.page_meta.get(input_path);
    let mut site_context = site.theme_context_with_assets(
        &current_href,
        Some(page_info),
        &context.page_info,
        context.languages.as_deref(),
        context.search,
        &context.theme_assets,
    );
    site_context.revealjs = context.revealjs_options.clone();
    let page_site_entry = if let Some(layout) = page_meta.and_then(|meta| meta.layout.as_deref()) {
        crate::theme::resolve_explicit_site_html_entry(&preprocessed.theme, layout)?
    } else {
        crate::theme::resolve_html_entry(&preprocessed.theme, crate::theme::HtmlScope::Site)?
    };
    let asset_decision = page_asset_decision(
        page_site_entry,
        context.generated_theme_entry.as_ref(),
        &context.theme_scripts,
    );
    let page_site_entry = asset_decision.html_entry;
    site_context.scripts = asset_decision
        .scripts
        .iter()
        .map(|script| html_escape(&page_relative_url(&current_href, script)))
        .collect();
    rewrite_theme_asset_hrefs(&current_href, &mut site_context.theme_assets);

    let compile = |output: PathBuf,
                   format: OutputFormat,
                   html_entry: Option<&crate::theme::HtmlEntry>,
                   html_syntax_theme: Option<&HtmlSyntaxTheme>,
                   site_context: Option<&crate::html::SiteContextInput>|
     -> Result<()> {
        ensure_parent_directory(&output)?;
        compile_with_typst(
            &context.typst,
            &preprocessed.layout,
            CompileOptions {
                output: Some(output),
                format: Some(format),
                typst_args: &context.typst_args,
                theme: &preprocessed.theme,
                html_scope: crate::theme::HtmlScope::Site,
                html_entry,
                html_syntax_theme,
                site_context,
                pages_input: Some(&context.pages_index_ref),
                current_href_input: Some(&current_href),
                minify_html: false,
                progress: false,
            },
        )
    };

    compile(
        html_output.clone(),
        OutputFormat::Html,
        page_site_entry.as_ref(),
        Some(&context.syntax_theme),
        Some(&site_context),
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
        compile(pdf_output, OutputFormat::Pdf, None, None, None)?;
    }

    Ok(())
}

fn rewrite_theme_asset_hrefs(current_href: &str, theme_assets: &mut [crate::html::SiteThemeAsset]) {
    for asset in theme_assets.iter_mut() {
        let href = page_relative_url(current_href, &asset.href);
        asset.href = html_escape(&href);
    }
}

fn embed_source_blob(html_output: &Path, source_path: &Path) -> Result<()> {
    let source = fs::read_to_string(source_path)
        .with_context(|| format!("failed to read {}", source_path.display()))?;
    let payload = escape_script_payload(&serde_json::to_string(&source)?);
    let mut html = fs::read_to_string(html_output)
        .with_context(|| format!("failed to read {}", html_output.display()))?;
    let script =
        format!("\n<script id=\"{SOURCE_DATA_ID}\" type=\"application/json\">{payload}</script>\n");
    if let Some(pos) = find_case_insensitive(&html, "</head>") {
        html.insert_str(pos, &script);
    } else {
        html.push_str(&script);
    }
    fs::write(html_output, html)
        .with_context(|| format!("failed to write {}", html_output.display()))
}

#[derive(Debug, Clone, Default)]
pub(super) struct ThemeGeneratedAssets {
    pub(super) stylesheets: Vec<GeneratedThemeAsset>,
    pub(super) scripts: Vec<GeneratedThemeAsset>,
    pub(super) assets: Vec<GeneratedThemeAsset>,
    pub(super) theme_assets: Vec<ThemeAssetInfo>,
}

#[derive(Debug, Clone)]
pub(crate) struct ThemeAssetInfo {
    pub(crate) name: String,
    pub(crate) rel_path: PathBuf,
    pub(crate) kind: String,
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
        asset_dir: &Path,
    ) -> Result<Self> {
        let mut stylesheets = Vec::new();
        let mut scripts = Vec::new();
        let mut assets = Vec::new();
        let mut theme_assets = Vec::new();
        for (name, source) in &entry.styles {
            let css = theme_css(source, syntax_theme);
            if css.trim().is_empty() {
                continue;
            }
            let asset = GeneratedThemeAsset::new_with_name(asset_dir, name, "css", css);
            theme_assets.push(ThemeAssetInfo {
                name: name.clone(),
                rel_path: asset.rel_path.clone(),
                kind: "css".to_string(),
            });
            stylesheets.push(asset);
        }
        for (name, source) in &entry.scripts {
            let kind = Path::new(name)
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("js")
                .to_string();
            let asset = GeneratedThemeAsset::new_with_name(asset_dir, name, &kind, source.clone());
            theme_assets.push(ThemeAssetInfo {
                name: name.clone(),
                rel_path: asset.rel_path.clone(),
                kind: kind.clone(),
            });
            scripts.push(asset);
        }
        for (name, source) in &entry.assets {
            let kind = Path::new(name)
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("")
                .to_string();
            let rel_kind = if kind.is_empty() { "txt" } else { &kind };
            let asset =
                GeneratedThemeAsset::new_with_name(asset_dir, name, rel_kind, source.clone());
            theme_assets.push(ThemeAssetInfo {
                name: name.clone(),
                rel_path: asset.rel_path.clone(),
                kind: rel_kind.to_string(),
            });
            assets.push(asset);
        }

        Ok(Self {
            stylesheets,
            scripts,
            assets,
            theme_assets,
        })
    }

    fn assets(&self) -> impl Iterator<Item = &GeneratedThemeAsset> {
        self.stylesheets
            .iter()
            .chain(self.scripts.iter())
            .chain(self.assets.iter())
    }

    pub(crate) fn theme_assets(&self) -> &[ThemeAssetInfo] {
        &self.theme_assets
    }

    pub(super) fn output_paths(&self, out_dir: &Path) -> BTreeSet<PathBuf> {
        self.assets()
            .map(|asset| out_dir.join(&asset.rel_path))
            .collect()
    }

    pub(super) fn write(&self, out_dir: &Path) -> Result<()> {
        for asset in self.assets() {
            asset.write(out_dir)?;
        }
        Ok(())
    }
}

impl GeneratedThemeAsset {
    fn new_with_name(asset_dir: &Path, name: &str, extension: &str, content: String) -> Self {
        let stem = Path::new(name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(name);
        let rel_path = if extension.is_empty() {
            asset_dir.join(stem)
        } else {
            asset_dir.join(format!("{stem}.{extension}"))
        };
        let rel_path = normalize_asset_relative_path(&rel_path.to_string_lossy());
        Self {
            rel_path: PathBuf::from(rel_path),
            content,
        }
    }

    pub(super) fn write(&self, out_dir: &Path) -> Result<()> {
        let path = out_dir.join(&self.rel_path);
        ensure_parent_directory(&path)?;
        fs::write(&path, &self.content)
            .with_context(|| format!("failed to write {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn embed_source_blob_inserts_source_data_before_head_and_escapes_script_end_tag_case_insensitive(
    ) {
        let dir = tempdir().unwrap();
        let source_path = dir.path().join("page.typ");
        let html_output = dir.path().join("page.html");
        std::fs::write(&source_path, "x\n</SCRIPT>").unwrap();
        std::fs::write(&html_output, "<html><HEAD></HEAD><body>x</body></html>").unwrap();

        embed_source_blob(&html_output, &source_path).unwrap();
        let output = std::fs::read_to_string(&html_output).unwrap();

        let script_pos = output
            .find(&format!(
                "<script id=\"{SOURCE_DATA_ID}\" type=\"application/json\">"
            ))
            .unwrap();
        let head_pos = find_case_insensitive(&output, "</head>").unwrap();

        assert!(script_pos < head_pos);
        assert!(output.contains("<\\/SCRIPT>"));
    }

    #[test]
    fn rewrite_theme_asset_hrefs_is_relative_to_page() {
        let mut theme_assets = vec![
            crate::html::SiteThemeAsset {
                name: "10_pico.css".to_string(),
                href: ".calepin/10_pico.css".to_string(),
                kind: "css".to_string(),
            },
            crate::html::SiteThemeAsset {
                name: "site.css".to_string(),
                href: "_calepin/site.css".to_string(),
                kind: "css".to_string(),
            },
        ];

        rewrite_theme_asset_hrefs("getting-started/install.html", &mut theme_assets);

        assert_eq!(theme_assets[0].href, "../.calepin/10_pico.css");
        assert_eq!(theme_assets[1].href, "../_calepin/site.css");
    }

    #[test]
    fn generated_theme_asset_rel_path_canonicalizes_duplicate_asset_prefix() {
        let asset = GeneratedThemeAsset::new_with_name(
            Path::new(".calepin"),
            ".calepin/theme-toggle.js",
            "js",
            "console.log(1)".to_string(),
        );

        assert_eq!(asset.rel_path, Path::new(".calepin/theme-toggle.js"));
    }

    #[test]
    fn generated_asset_paths_preserve_distinct_prefix_segments() {
        assert_eq!(
            normalize_asset_relative_path(".calepin/.calepin/site.css"),
            ".calepin/site.css"
        );
        assert_eq!(
            normalize_asset_relative_path("assets/_calepin/_calepin/site.css"),
            "assets/_calepin/site.css"
        );
        assert_eq!(
            normalize_asset_relative_path("assets/assets/site.css"),
            "assets/assets/site.css"
        );
    }
}
