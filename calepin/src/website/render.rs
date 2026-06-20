use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use xxhash_rust::xxh3::xxh3_64;

use crate::html::{html_theme_script, html_theme_stylesheet, minify_html_file, HtmlSyntaxTheme};
use crate::typst::compile::{compile_with_typst, CompileOptions, OutputFormat};
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
    if html_source_references_site_stylesheet_token(source) {
        return true;
    }
    static_html_includes(source).into_iter().any(|name| {
        visited.insert(name.clone())
            && partials.get(&name).is_some_and(|partial| {
                html_source_references_site_stylesheet(partial, partials, visited)
            })
    })
}

fn html_source_references_site_stylesheet_token(source: &str) -> bool {
    template_tag_references_site_stylesheet(source, "{{", "}}")
        || template_tag_references_site_stylesheet(source, "{%", "%}")
}

fn template_tag_references_site_stylesheet(source: &str, open: &str, close: &str) -> bool {
    let mut rest = source;
    while let Some(start) = rest.find(open) {
        let after_open = &rest[start + open.len()..];
        let Some(end) = after_open.find(close) else {
            break;
        };
        let block = &after_open[..end];
        if template_tag_has_site_stylesheet(block) {
            return true;
        }
        rest = &after_open[end + close.len()..];
    }
    false
}

fn template_tag_has_site_stylesheet(block: &str) -> bool {
    let target = b"site.stylesheet";
    let mut in_single = false;
    let mut in_double = false;
    let bytes = block.as_bytes();

    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_single {
            if byte == b'\'' && !is_template_token_escaped(bytes, index) {
                in_single = false;
            } else if byte == b'\\' && index + 1 < bytes.len() {
                index += 1;
            }
            index += 1;
            continue;
        }
        if in_double {
            if byte == b'"' && !is_template_token_escaped(bytes, index) {
                in_double = false;
            } else if byte == b'\\' && index + 1 < bytes.len() {
                index += 1;
            }
            index += 1;
            continue;
        }

        if byte == b'\'' {
            in_single = true;
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_double = true;
            index += 1;
            continue;
        }
        if bytes.len() >= index + target.len()
            && bytes_is_ascii_equal_ignore_case(&bytes[index..index + target.len()], target)
            && (index == 0 || is_template_token_separator_byte(bytes[index - 1]))
            && (index + target.len() == bytes.len()
                || is_template_token_separator_byte(bytes[index + target.len()]))
        {
            return true;
        }
        index += 1;
    }
    false
}

fn is_template_token_escaped(bytes: &[u8], index: usize) -> bool {
    let mut i = index;
    let mut backslashes = 0;
    while i > 0 {
        i -= 1;
        if bytes[i] == b'\\' {
            backslashes += 1;
        } else {
            break;
        }
    }
    backslashes % 2 == 1
}

fn is_template_token_separator_byte(byte: u8) -> bool {
    !byte.is_ascii_alphanumeric() && byte != b'.' && byte != b'_'
}

fn static_html_includes(source: &str) -> Vec<String> {
    let mut includes = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find("{%") {
        let rest_after_start = &rest[start + 2..];
        let Some(end) = rest_after_start.find("%}") else {
            break;
        };
        let block = trim_template_tokens(&rest_after_start[..end]);
        if let Some(include) = static_include_name(block) {
            includes.push(include);
        }
        rest = &rest_after_start[end + 2..];
    }
    includes
}

fn trim_template_tokens(block: &str) -> &str {
    block
        .trim()
        .trim_start_matches(|char| char == '-' || char == '+')
        .trim_end_matches(|char| char == '-' || char == '+')
        .trim()
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
                config_styles: &[],
                html_syntax_theme,
                site_context,
                pages_input: Some(PAGES_INDEX_REF),
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

    fn assets(&self) -> impl Iterator<Item = &GeneratedThemeAsset> {
        self.stylesheet.iter().chain(self.script.iter())
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
        ensure_parent_directory(&path)?;
        fs::write(&path, &self.content)
            .with_context(|| format!("failed to write {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    use crate::theme::HtmlEntry;

    #[test]
    fn html_entry_references_site_stylesheet_requires_template_token() {
        let entry = HtmlEntry {
            theme_name: "local".into(),
            layout: r#"<script>var source = "site.stylesheet";</script>"#.into(),
            partials: Vec::new(),
            styles: Vec::new(),
            scripts: Vec::new(),
            is_default: false,
        };

        assert!(!html_entry_references_site_stylesheet(&entry));
    }

    #[test]
    fn html_entry_references_site_stylesheet_follows_whitespace_control_includes() {
        let entry = HtmlEntry {
            theme_name: "local".into(),
            layout: r#"{{ doc.head }}{%- include 'partials/styles.html' +%}"#.into(),
            partials: vec![(
                "partials/styles.html".to_string(),
                "{{ if x and site.stylesheet }}".into(),
            )],
            styles: Vec::new(),
            scripts: Vec::new(),
            is_default: false,
        };

        assert!(html_entry_references_site_stylesheet(&entry));
    }

    #[test]
    fn html_entry_references_site_stylesheet_ignores_quoted_values() {
        let entry = HtmlEntry {
            theme_name: "local".into(),
            layout: "{{ \"a \\\"site.stylesheet\\\" token\" }}".into(),
            partials: Vec::new(),
            styles: Vec::new(),
            scripts: Vec::new(),
            is_default: false,
        };

        assert!(!html_entry_references_site_stylesheet(&entry));
    }

    #[test]
    fn static_html_includes_parses_whitespace_control_markers() {
        let includes = static_html_includes(
            r#"{%+ include 'partials/wrapper.html' +%}{%- include "partials/styles.html" -%}"#,
        );

        assert_eq!(
            includes,
            vec![
                "partials/wrapper.html".to_string(),
                "partials/styles.html".to_string(),
            ]
        );
    }

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
}
