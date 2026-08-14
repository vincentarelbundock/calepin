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
    staged_input: &Path,
    page_meta: Option<serde_json::Value>,
    store: serde_json::Value,
) -> Result<crate::theme::NotebookTemplateContext> {
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
    let page_meta = page_meta.unwrap_or(serde_json::Value::Null);
    // Best-available title before Typst runs: the `<website-metadata>` title.
    let title = page_meta
        .get("title")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    // Inline the staged source directly at the theme's `{{ doc.body }}` seam
    // (rather than `#include`-ing it) so the body shares a file scope with the
    // theme preamble and can call its `#let`/`#import`ed helpers. The body is a
    // render-context variable, so minijinja substitutes it literally without
    // re-evaluating any `{{ }}`/`{% %}` it may contain.
    let staged_input_abs = layout.root.join(staged_input);
    let body = fs::read_to_string(&staged_input_abs)
        .with_context(|| format!("failed to read {}", staged_input_abs.display()))?;
    Ok(crate::theme::NotebookTemplateContext {
        input_path: slash_path(&layout.input_rel),
        input_dir,
        input_stem,
        title,
        body,
        page_meta,
        store,
    })
}

pub(super) fn write_render_wrapper(
    layout: &LayoutPaths,
    runtime_import: &str,
    include_input: Option<&Path>,
    jupyter_kernels: &[&str],
    notebook_theme: Option<&crate::theme::NotebookSource>,
    expected_generation: Option<&str>,
) -> Result<PathBuf> {
    write_wrapper(
        layout,
        "wrapper.typ",
        runtime_import,
        include_input,
        jupyter_kernels,
        notebook_theme,
        expected_generation,
    )
}

pub(super) fn write_query_wrapper(
    layout: &LayoutPaths,
    runtime_import: &str,
    include_input: Option<&Path>,
    notebook_theme: Option<&crate::theme::NotebookSource>,
) -> Result<PathBuf> {
    write_wrapper(
        layout,
        "query-wrapper.typ",
        runtime_import,
        include_input,
        &[],
        notebook_theme,
        None,
    )
}

fn write_wrapper(
    layout: &LayoutPaths,
    entry_name: &str,
    runtime_import: &str,
    include_input: Option<&Path>,
    jupyter_kernels: &[&str],
    notebook_theme: Option<&crate::theme::NotebookSource>,
    expected_generation: Option<&str>,
) -> Result<PathBuf> {
    let wrapper_relative = layout.entry_relative_path(entry_name);
    let wrapper = layout.root.join(&wrapper_relative);

    // The runtime exports `document` for authored `#show: calepin.document`
    // calls. Preserve Typst's element under the bare name used by inlined
    // sources and themes before the managed wrapper imports the runtime API.
    let mut lines = format!(
        "#let _calepin-document-element = document\n#import \"{runtime_import}\": *\n#let document = _calepin-document-element\n\n"
    );
    if let Some(generation) = expected_generation {
        lines.push_str(&format!(
            "#let _calepin-expected-generation = {}\n\
             #let _calepin-verify-generation() = {{\n\
             \u{20}\u{20}let path = sys.inputs.at(\"calepin-results\", default: none)\n\
             \u{20}\u{20}if path != none and path != \"\" {{\n\
             \u{20}\u{20}\u{20}\u{20}let actual = json(path).at(\"generation\", default: \"\")\n\
             \u{20}\u{20}\u{20}\u{20}if actual != _calepin-expected-generation {{\n\
             \u{20}\u{20}\u{20}\u{20}\u{20}\u{20}panic(\"Calepin results changed while this render was starting; Typst will retry with the completed build\")\n\
             \u{20}\u{20}\u{20}\u{20}}}\n\
             \u{20}\u{20}}}\n\
             }}\n\
             #_calepin-verify-generation()\n\n",
            typst_string(generation)
        ));
    }

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

    lines.push('\n');
    lines.push_str(html_raw_show_rule());

    lines.push('\n');
    lines.push_str(heading_anchor_show_rule());

    // Default chunk styling. Installed here, and only alongside a notebook
    // theme, so that `theme = "typst"` leaves the labeled carriers bare while
    // chunks still execute. Defined after the fenced-chunk rules above so its
    // own raw rule is the outermost one, and before the theme source and
    // document body so both can displace it with their own label rules.
    if notebook_theme.is_some() {
        lines.push('\n');
        lines.push_str(
            "#show: _default-chunk-chrome.with(raw-chunk-langs: _raw-chunk-langs)\n",
        );
    }

    if let Some(notebook_theme) = notebook_theme {
        lines.push_str("\n// Notebook theme\n");
        let theme_source = rewrite_runtime_imports(&notebook_theme.source, runtime_import);
        lines.push_str(&theme_source);
        if !theme_source.ends_with('\n') {
            lines.push('\n');
        }
    }

    if let Some(include_input) = include_input {
        lines.push_str(&format!("\n#include \"/{}\"\n", slash_path(include_input)));
    }

    write_if_changed(&wrapper, lines)?;
    Ok(wrapper_relative)
}

pub(super) fn raw_chunk_langs(jupyter_kernels: &[&str]) -> Vec<String> {
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
        "#show raw.where(block: true, lang: {lang}, theme: auto): it => if _disable-raw-chunk-transforms.get() {{ _html-themed-raw-block(it) }} else {{ _fenced-chunk({lang}, it) }}\n"
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
    _fenced-chunk(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}
"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::testfixtures;

    #[test]
    fn raw_show_rule_escapes_kernel_names() {
        let rule = raw_show_rule(r#"weird"kernel"#);

        assert!(rule.contains(r#"lang: "weird\"kernel""#), "{rule}");
        assert!(
            rule.contains(r#"_fenced-chunk("weird\"kernel""#),
            "{rule}"
        );
    }

    #[test]
    fn query_and_render_wrappers_are_isolated_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let layout = testfixtures::layout(dir.path());

        let query = write_query_wrapper(&layout, "/.calepin/calepin.typ", None, None).unwrap();
        let render =
            write_render_wrapper(&layout, "/.calepin/calepin.typ", None, &[], None, Some("g"))
                .unwrap();

        assert_ne!(query, render);
        assert!(layout.root.join(query).is_file());
        assert!(layout.root.join(render).is_file());
    }
}
