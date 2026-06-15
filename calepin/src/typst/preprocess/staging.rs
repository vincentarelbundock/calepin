use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::typst::io::write_if_changed;
use crate::typst::model::LayoutPaths;
use crate::typst::paths::slash_path;

pub(super) fn paged_template_context(
    layout: &LayoutPaths,
    include_input: &Path,
    page_meta: Option<serde_json::Value>,
    params: serde_json::Value,
) -> crate::theme::PagedTemplateContext {
    let input_dir = layout
        .input_rel
        .parent()
        .map(slash_path)
        .unwrap_or_default();
    let input_stem = layout
        .input_rel
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();
    crate::theme::PagedTemplateContext {
        input_path: slash_path(&layout.input_rel),
        input_dir,
        input_stem,
        body: format!("#include \"/{}\"", slash_path(include_input)),
        page_meta: page_meta.unwrap_or(serde_json::Value::Null),
        params,
    }
}

pub(super) fn write_render_wrapper(
    layout: &LayoutPaths,
    include_input: &Path,
    jupyter_kernels: &[&str],
    paged_theme: Option<&crate::theme::PagedSource>,
) -> Result<PathBuf> {
    let wrapper_relative = layout.artifact_relative_path("calepin-wrapper.typ");
    let wrapper = layout.root.join(&wrapper_relative);

    let mut lines = String::from("#import \"/.calepin/calepin.typ\": *\n\n");

    lines.push('\n');
    lines.push('\n');

    for lang in ["typ", "typst"] {
        lines.push_str(&format!(
            "#show raw.where(block: true, lang: \"{lang}\", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))\n"
        ));
    }

    for lang in ["python", "r", "mermaid", "dot", "tikz", "d2"] {
        lines.push_str(&raw_show_rule(lang));
    }

    for kernel in jupyter_kernels {
        lines.push_str(&raw_show_rule(kernel));
    }

    lines.push_str("\n");
    lines.push_str(html_raw_show_rule());

    if let Some(paged_theme) = paged_theme {
        lines.push_str("\n// Paged theme\n");
        lines.push_str(&paged_theme.source);
        if !paged_theme.source.ends_with('\n') {
            lines.push('\n');
        }
    }

    if !paged_theme.is_some_and(|theme| theme.owns_body) {
        lines.push_str(&format!("\n#include \"/{}\"\n", slash_path(include_input)));
    }

    write_if_changed(&wrapper, lines)?;
    Ok(wrapper_relative)
}

fn raw_show_rule(lang: &str) -> String {
    format!(
        "#show raw.where(block: true, lang: \"{lang}\", theme: auto): it => if _disable-raw-chunk-transforms.get() {{ _html-themed-raw-block(it) }} else {{ chunk-from-raw-plain(\"{lang}\", it) }}\n"
    )
}

fn html_raw_show_rule() -> &'static str {
    r#"#show raw.where(block: true, theme: auto): it => {
  if _mode == "query" {
    it
  } else if not _html-target() {
    it
  } else if _disable-raw-chunk-transforms.get() {
    _html-themed-raw-block(it)
  } else if it.has("lang") and it.lang != none and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    it
  } else {
    _html-themed-raw-block(it)
  }
}
"#
}

pub(super) fn write_query_source(layout: &LayoutPaths, staged_input: &Path) -> Result<PathBuf> {
    let query_source_relative = layout.artifact_relative_path("query-source.typ");
    let query_source = layout.root.join(&query_source_relative);
    write_query_html_fallback(&layout.root)?;

    let staged_input_abs = layout.root.join(staged_input);
    let source = fs::read_to_string(&staged_input_abs)
        .with_context(|| format!("failed to read {}", staged_input_abs.display()))?;
    let mut prefixed = String::from("#import \"/.calepin/query-html.typ\" as html\n\n");
    prefixed.push_str(&source);
    write_if_changed(&query_source, prefixed)?;
    Ok(query_source_relative)
}

fn write_query_html_fallback(root: &Path) -> Result<()> {
    let path = root.join(".calepin/query-html.typ");
    let source = r#"#let elem(name, attrs: (:), body) = body
#let link(..args) = none
#let script(..args) = none
#let img(..args) = none
"#;
    write_if_changed(&path, source)
}
