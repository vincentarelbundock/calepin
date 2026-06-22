use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::html::{minify_html_file, HtmlSyntaxTheme};
use crate::typst::compile::{compile_with_typst, CompileOptions, OutputFormat};
use crate::typst::paths::project_relative_path;
use crate::typst::preprocess::PreprocessOutput;

use super::preprocess::run_parallel;
use super::site::SiteModel;
use super::{BuildContext, SOURCE_DATA_ID};

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
    site_context.vars = context.vars.clone();
    site_context.head_include = context.html.head.clone();
    site_context.body_include = context.html.body.clone();
    site_context.toc_depth = Some(crate::theme::resolve_toc_depth(
        page_meta.and_then(|meta| meta.toc.as_ref()),
        &context.toc,
        &preprocessed.theme,
    ));
    let page_site_entry = if let Some(layout) = page_meta.and_then(|meta| meta.layout.as_deref()) {
        crate::theme::resolve_explicit_site_html_entry(&preprocessed.theme, layout)?
    } else {
        crate::theme::resolve_html_entry(&preprocessed.theme, crate::theme::HtmlScope::Site)?
    };
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
}
