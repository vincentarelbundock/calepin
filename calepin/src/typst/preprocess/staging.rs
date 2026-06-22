use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::typst::io::write_if_changed;
use crate::typst::model::LayoutPaths;
use crate::typst::paths::slash_path;
use crate::typst::source_rewrite::rewrite_runtime_imports;

const BUILTIN_RAW_CHUNK_LANGS: &[&str] = &["python", "r", "mermaid", "dot", "tikz", "d2"];

pub(super) fn notebook_template_context(
    layout: &LayoutPaths,
    include_input: &Path,
    page_meta: Option<serde_json::Value>,
    params: serde_json::Value,
) -> crate::theme::NotebookTemplateContext {
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
    crate::theme::NotebookTemplateContext {
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
    runtime_import: &str,
    include_input: &Path,
    jupyter_kernels: &[&str],
    notebook_theme: Option<&crate::theme::NotebookSource>,
) -> Result<PathBuf> {
    let wrapper_relative = layout.artifact_relative_path("calepin-wrapper.typ");
    let wrapper = layout.root.join(&wrapper_relative);

    let mut lines = format!("#import \"{runtime_import}\": *\n\n");

    lines.push('\n');
    lines.push('\n');
    lines.push_str(&format!(
        "#let _raw-chunk-langs = ({})\n",
        raw_chunk_langs(jupyter_kernels)
            .iter()
            .map(|lang| typst_string(lang))
            .collect::<Vec<_>>()
            .join(", ")
    ));

    for lang in ["typ", "typst"] {
        lines.push_str(&format!(
            "#show raw.where(block: true, lang: \"{lang}\", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))\n"
        ));
    }

    for lang in BUILTIN_RAW_CHUNK_LANGS {
        lines.push_str(&raw_show_rule(lang));
    }

    for kernel in jupyter_kernels {
        lines.push_str(&raw_show_rule(kernel));
    }

    lines.push_str("\n");
    lines.push_str(html_raw_show_rule());

    lines.push_str("\n");
    lines.push_str(heading_anchor_show_rule());

    if let Some(notebook_theme) = notebook_theme {
        lines.push_str("\n// Notebook theme\n");
        let theme_source = rewrite_runtime_imports(&notebook_theme.source, runtime_import);
        lines.push_str(&theme_source);
        if !theme_source.ends_with('\n') {
            lines.push('\n');
        }
    }

    if !notebook_theme.is_some_and(|theme| theme.owns_body) {
        lines.push_str(&format!("\n#include \"/{}\"\n", slash_path(include_input)));
    }

    write_if_changed(&wrapper, lines)?;
    Ok(wrapper_relative)
}

fn raw_chunk_langs(jupyter_kernels: &[&str]) -> Vec<String> {
    let mut langs = Vec::new();
    for lang in BUILTIN_RAW_CHUNK_LANGS {
        langs.push(lang.to_string());
    }
    for kernel in jupyter_kernels {
        if !langs.iter().any(|lang| lang == kernel) {
            langs.push((*kernel).to_string());
        }
    }
    langs
}

fn typst_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn raw_show_rule(lang: &str) -> String {
    let lang = typst_string(lang);
    format!(
        "#show raw.where(block: true, lang: {lang}, theme: auto): it => if _disable-raw-chunk-transforms.get() {{ _html-themed-raw-block(it) }} else {{ chunk_from_raw_plain({lang}, it) }}\n"
    )
}

/// In HTML export, Typst drops a heading's explicit label, so the post-render
/// id assignment in `html::theme` cannot honor it. Emit a tiny marker element
/// carrying the label immediately before each labeled heading; the HTML
/// post-processor reads it to set the heading `id` and then strips it. Only the
/// HTML target is affected; paged/query passes re-emit the heading untouched.
fn heading_anchor_show_rule() -> &'static str {
    r#"#show heading: it => {
  if _is-html() and "label" in it.fields() {
    std.html.elem("calepin-heading-anchor", attrs: (data-id: str(it.label)))
  }
  it
}
"#
}

fn html_raw_show_rule() -> &'static str {
    r#"#show raw.where(block: true, theme: auto): it => {
  if _is-query() {
    it
  } else if _disable-raw-chunk-transforms.get() {
    _html-themed-raw-block(it)
  } else if it.has("lang") and it.lang != none and _raw-chunk-langs.contains(it.lang) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}
"#
}

pub(super) fn write_query_source(layout: &LayoutPaths, staged_input: &Path) -> Result<PathBuf> {
    let query_source_relative = layout.artifact_relative_path("query-source.typ");
    let query_source = layout.root.join(&query_source_relative);

    let staged_input_abs = layout.root.join(staged_input);
    let source = fs::read_to_string(&staged_input_abs)
        .with_context(|| format!("failed to read {}", staged_input_abs.display()))?;
    write_if_changed(&query_source, source)?;
    Ok(query_source_relative)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_show_rule_escapes_kernel_names() {
        let rule = raw_show_rule(r#"weird"kernel"#);

        assert!(rule.contains(r#"lang: "weird\"kernel""#), "{rule}");
        assert!(
            rule.contains(r#"chunk_from_raw_plain("weird\"kernel""#),
            "{rule}"
        );
    }
}
