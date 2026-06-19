use std::process::Command;

use super::*;

fn typst_accessible_tempdir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("calepin-runtime-test-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap()
}

macro_rules! skip_if_no_typst {
    () => {
        if !typst_available() {
            return;
        }
    };
}

fn typst_available() -> bool {
    Command::new("typst").arg("--version").output().is_ok()
}

fn typst_query(root: &Path, input: &Path, selector: &str) -> String {
    typst_query_with_inputs(root, input, selector, &["calepin-mode=query"])
}

fn typst_query_with_inputs(root: &Path, input: &Path, selector: &str, inputs: &[&str]) -> String {
    let mut command = Command::new("typst");
    command
        .arg("query")
        .arg(input)
        .arg(selector)
        .arg("--root")
        .arg(root);
    for input in inputs {
        command.arg("--input").arg(input);
    }
    let output = command.arg("--pretty").output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn typst_compile(root: &Path, input: &Path, output: &Path, args: &[&str]) {
    let mut command = Command::new("typst");
    command
        .arg("compile")
        .arg(input)
        .arg(output)
        .arg("--root")
        .arg(root);
    for arg in args {
        command.arg(arg);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn pdf_text(path: &Path) -> String {
    let text = Command::new("pdftotext")
        .arg(path)
        .arg("-")
        .output()
        .unwrap();
    assert!(text.status.success());
    String::from_utf8(text.stdout).unwrap()
}

#[test]
fn write_runtime_writes_calepin_typ() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_runtime(dir.path()).unwrap();
    assert_eq!(path, dir.path().join(".calepin").join("calepin.typ"));
    let facade = std::fs::read_to_string(path).unwrap();
    assert!(facade.contains("#import \"runtime/core/state.typ\" as state"));
    assert!(facade.contains("#let elements = elementmod"));
    assert!(dir
        .path()
        .join(".calepin/runtime/00_syntax-theme.typ")
        .is_file());
    assert!(dir
        .path()
        .join(".calepin/runtime/elements/mod.typ")
        .is_file());
}

#[test]
fn typst_compile_html_elements_columns_uses_plain_pico_grid() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.html");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.elements.columns(
  columns: 2,
  wrap: false,
  html.elem("section")[One],
  html.elem("section")[Two],
)
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &["--features", "html", "--input", "calepin-target=html"],
    );
    let html = std::fs::read_to_string(output).unwrap();
    assert!(
        html.contains("class=grid") || html.contains(r#"class="grid""#),
        "expected Pico grid class in HTML output:\n{html}"
    );
    assert!(
        !html.contains("grid-template-columns"),
        "expected columns helper not to emit explicit grid tracks:\n{html}"
    );
    assert!(
        html.contains("<div class=grid><section>One</section><section>Two</section></div>")
            || html.contains(
                r#"<div class="grid"><section>One</section><section>Two</section></div>"#
            ),
        "expected grid children to be emitted without item wrappers:\n{html}"
    );
}

#[test]
fn typst_compile_html_elements_columns_wraps_items_by_default() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.html");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.elements.columns(
  columns: 2,
  html.elem("section")[One],
  html.elem("section")[Two],
)
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &["--features", "html", "--input", "calepin-target=html"],
    );
    let html = std::fs::read_to_string(output).unwrap();
    assert!(
        html.contains("<div class=grid><div><section>One</section></div><div><section>Two</section></div></div>")
            || html.contains(r#"<div class="grid"><div><section>One</section></div><div><section>Two</section></div></div>"#),
        "expected grid children to be wrapped by default:\n{html}"
    );
}

#[test]
fn typst_compile_elements_columns_rejects_track_list() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.html");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.elements.columns(
  columns: (2fr, 1fr),
  [One],
  [Two],
)
"##,
    )
    .unwrap();

    let output = Command::new("typst")
        .arg("compile")
        .arg(&input)
        .arg(&output)
        .arg("--root")
        .arg(dir.path())
        .arg("--features")
        .arg("html")
        .arg("--input")
        .arg("calepin-target=html")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("calepin.elements.columns: columns must be a positive integer"),
        "{stderr}"
    );
}

#[test]
fn typst_compile_html_elements_tabs_emit_webawesome_markup() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.html");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.elements.tabs[
  #calepin.elements.tab("General")[
    This is the general tab panel.
  ]
  #calepin.elements.tab("Advanced", active: true)[
    This is the advanced tab panel.
  ]
  #calepin.elements.tab("Disabled", disabled: true)[
    This is a disabled tab panel.
  ]
]
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &["--features", "html", "--input", "calepin-target=html"],
    );
    let html = std::fs::read_to_string(output).unwrap();
    assert!(html.contains("components/tab-group/tab-group.js"), "{html}");
    assert!(html.contains("<wa-tab-group"), "{html}");
    assert!(html.contains("--track-width"), "{html}");
    assert!(html.contains("padding-block-start"), "{html}");
    assert!(html.contains("wa-tab[active]"), "{html}");
    assert!(html.contains("--pico-primary"), "{html}");
    assert!(html.contains("active"), "{html}");
    assert!(!html.contains("placement="), "{html}");
    assert!(!html.contains("activation="), "{html}");
    assert!(html.contains("<wa-tab"), "{html}");
    assert!(html.contains(r#"panel="calepin-tab-General""#), "{html}");
    assert!(html.contains(r#"panel="calepin-tab-Advanced""#), "{html}");
    assert!(html.contains("disabled"), "{html}");
    assert!(html.contains("<wa-tab-panel"), "{html}");
    assert!(html.contains(r#"name="calepin-tab-Advanced""#), "{html}");
    assert!(html.contains("This is the advanced tab panel."), "{html}");
}

#[test]
fn typst_query_elements_tabs_exposes_nested_chunks() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let input = dir.path().join("paper.typ");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.setup(fenced-chunks: true)

#calepin.elements.tabs[
  #calepin.elements.tab("Python", active: true)[
```python
print("hello from tab")
```
  ]
]
"##,
    )
    .unwrap();

    let stdout = typst_query(
        dir.path(),
        &input,
        "raw.where(block: true).or(<calepin-chunk>)",
    );
    assert!(stdout.contains(r#""func": "raw""#), "{stdout}");
    assert!(stdout.contains(r#""lang": "python""#), "{stdout}");
    assert!(stdout.contains("hello from tab"), "{stdout}");
}

#[test]
fn typst_query_emits_chunk_metadata() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let input = dir.path().join("paper.typ");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.setup(fig-device-format: "png", fig-device-width: 5)

#calepin.chunk("r", fig-caption: [R caption], fig-alt-text: "R alt")[```
x <- 1
```]

#calepin.chunk("python")[```
print("hello")
```]
"##,
    )
    .unwrap();

    let stdout = typst_query(dir.path(), &input, "<calepin-chunk>");
    assert!(stdout.contains(r#""label": "chunk-1""#), "{}", stdout);
    assert!(stdout.contains(r#""engine": "r""#));
    assert!(stdout.contains(r#""text": "x <- 1""#));
    assert!(stdout.contains(r#""fig-device-format": "auto""#));
    assert!(stdout.contains(r#""fig-caption": "#));
    assert!(stdout.contains(r#""text": "R caption""#));
    assert!(stdout.contains(r#""fig-alt-text": "R alt""#));
    assert!(stdout.contains(r#""label": "chunk-2""#));
    assert!(stdout.contains(r#""engine": "python""#));
    assert!(stdout.contains(r#""text": "print(\"hello\")""#));
}

#[test]
fn typst_query_infers_chunk_engine_from_fenced_language() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let input = dir.path().join("paper.typ");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk(label: "inferred")[```r
x <- 1
```]
"##,
    )
    .unwrap();

    let stdout = typst_query(dir.path(), &input, "<calepin-chunk>");
    assert!(stdout.contains(r#""label": "inferred""#), "{}", stdout);
    assert!(stdout.contains(r#""engine": "r""#), "{}", stdout);
}

#[test]
fn typst_query_supports_plain_python_raw_blocks_when_enabled() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let input = dir.path().join("paper.typ");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.setup(fenced-chunks: true)

```python
print("hello")
```
"##,
    )
    .unwrap();

    let stdout = typst_query(
        dir.path(),
        &input,
        "raw.where(block: true).or(<calepin-chunk>)",
    );
    assert!(stdout.contains(r#""func": "raw""#), "{}", stdout);
    assert!(stdout.contains(r#""lang": "python""#), "{}", stdout);
    assert!(
        stdout.contains(r#""text": "print(\"hello\")""#),
        "{}",
        stdout
    );
}

#[test]
fn typst_query_supports_fenced_chunks_engine_option() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let input = dir.path().join("paper.typ");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.setup(fenced-chunks: "python")
"##,
    )
    .unwrap();

    let stdout = typst_query(dir.path(), &input, "<calepin-config>");
    assert!(
        stdout.contains(r#""fenced-chunks": "python""#),
        "{}",
        stdout
    );
}

#[test]
fn typst_query_plain_fenced_chunks_does_not_reprocess_chunk_body() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let input = dir.path().join("paper.typ");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.setup(fenced-chunks: "r", results: "verbatim")

```r
x <- 41
print(x + 1)
```

#calepin.chunk("r", label: "no-run-r", eval: false, results: "verbatim")[
```r
y <- 2
y + 2
```]

The inline sum is #calepin.inline("r", eval: true)[`x <- 10; x + 5`]

#calepin.chunk("python", echo: false, results: "typst")[
```python
print("hello from python")
```]
"##,
    )
    .unwrap();

    let stdout = typst_query(dir.path(), &input, "<calepin-chunk>");
    assert!(stdout.contains(r#""label": "no-run-r""#), "{}", stdout);
    assert!(stdout.contains(r#""label": "inline-1""#), "{}", stdout);
    assert!(stdout.contains(r#""label": "chunk-1""#), "{}", stdout);
    assert!(!stdout.contains(r#""label": "chunk-3""#), "{}", stdout);
    assert!(!stdout.contains(r#""label": "chunk-2""#), "{}", stdout);
}

#[test]
fn typst_compile_without_results_shows_code() {
    skip_if_no_typst!();
    if Command::new("pdftotext").arg("-v").output().is_err() {
        return;
    }

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.pdf");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("python", echo: false)[```
print("FALLBACK_12345")
```]
"##,
    )
    .unwrap();

    typst_compile(dir.path(), &input, &output, &[]);
    let extracted = pdf_text(&output);
    assert!(
        extracted.contains("FALLBACK_12345"),
        "expected source code in PDF output"
    );
    assert!(!extracted.contains("Calepin output is missing."));
}

#[test]
fn typst_compile_with_results_and_echo_shows_both() {
    skip_if_no_typst!();
    if Command::new("pdftotext").arg("-v").output().is_err() {
        return;
    }

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.pdf");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.setup(echo: true)

#calepin.chunk("python")[```
print("RESULT_12345")
```]
"##,
    )
    .unwrap();

    typst_compile(dir.path(), &input, &output, &[]);
    let extracted = pdf_text(&output);
    assert!(
        extracted.contains("print(\"RESULT_12345\")"),
        "expected source code in PDF output"
    );
    assert!(
        extracted.contains("RESULT_12345"),
        "expected execution output in PDF output"
    );
}

fn svg_path_heights(svg: &str, fill: &str) -> Vec<f64> {
    let needle = format!(r#"<path fill="{fill}" fill-rule="nonzero" d=""#);
    let mut heights = Vec::new();
    for part in svg.split(&needle).skip(1) {
        let Some(path) = part.split('"').next() else {
            continue;
        };
        if let Some((_, after_v)) = path.split_once("v ") {
            if let Some(value) = after_v.split_whitespace().next() {
                let value = value.trim_end_matches(|ch: char| {
                    !(ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+'))
                });
                if let Ok(height) = value.parse::<f64>() {
                    heights.push(height.abs());
                }
            }
        }
    }
    heights
}

#[test]
fn typst_compile_raw_size_applies_to_rewritten_and_explicit_chunks() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.svg");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#set page(width: 420pt, height: 220pt, margin: 16pt)
#set text(size: 20pt)
#show raw.where(block: true): set text(size: .5em)

#calepin.setup(fenced-chunks: true, echo: true)

#calepin.chunk_from_raw_plain("python", raw("print(\"rewritten\")\n", block: true, lang: "python"))

#calepin.chunk("python", label: "explicit")[
```python
print("explicit")
```
]
"##,
    )
    .unwrap();

    typst_compile(dir.path(), &input, &output, &[]);
    let svg = std::fs::read_to_string(&output).unwrap();
    let input_heights = svg_path_heights(&svg, "#f7f7f5");
    assert_eq!(
        input_heights.len(),
        2,
        "expected two Calepin input code cells in SVG: {svg}"
    );
    assert_eq!(
        input_heights[0], input_heights[1],
        "bare and explicit chunks should use the same input cell height: {input_heights:?}"
    );
    assert!(
        input_heights.iter().all(|height| *height < 28.0),
        "raw text sizing should reduce both input cells: {input_heights:?}"
    );
}

#[test]
fn typst_compile_echo_strips_qmd_header_lines() {
    skip_if_no_typst!();
    if Command::new("pdftotext").arg("-v").output().is_err() {
        return;
    }

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.pdf");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.setup(echo: true)

#calepin.chunk("python")[```
#| label: fig-clean-echo
#| echo: true
print("VISIBLE_CODE_12345")
```]
"##,
    )
    .unwrap();

    typst_compile(dir.path(), &input, &output, &[]);
    let extracted = pdf_text(&output);
    assert!(extracted.contains("print(\"VISIBLE_CODE_12345\")"));
    assert!(!extracted.contains("#| label"));
    assert!(!extracted.contains("#| echo"));
}

#[test]
fn typst_compile_renders_canonical_figure_options_from_results() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let figures = dir.path().join(".calepin/paper/figures");
    std::fs::create_dir_all(&figures).unwrap();
    std::fs::write(
            figures.join("fig-demo.svg"),
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80"><rect width="120" height="80" fill="#88c0d0"/></svg>"##,
        )
        .unwrap();
    std::fs::write(
            figures.join("fig-demo-2.svg"),
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80"><circle cx="60" cy="40" r="30" fill="#a3be8c"/></svg>"##,
        )
        .unwrap();
    std::fs::create_dir_all(dir.path().join(".calepin/paper")).unwrap();
    std::fs::write(
        dir.path().join(".calepin/paper/results.json"),
        r#"{
  "schema": 1,
  "calepin_version": "test",
  "input": "paper.typ",
  "chunks": {
    "fig-demo": {
      "label": "fig-demo",
      "engine": "python",
      "status": "ok",
      "items": [
        {
          "type": "display",
          "data": {
            "image/svg+xml": {
              "path": "/.calepin/paper/figures/fig-demo.svg"
            }
          }
        },
        {
          "type": "display",
          "data": {
            "image/svg+xml": {
              "path": "/.calepin/paper/figures/fig-demo-2.svg"
            }
          }
        }
      ]
    }
  }
}"#,
    )
    .unwrap();

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.pdf");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk(
  "python",
  label: "fig-demo",
  echo: false,
  fig-width: 50%,
  fig-align: center,
  fig-responsive: true,
  fig-link: "https://example.com",
  fig-caption: [Runtime figure caption],
  fig-cap-location: top,
  fig-alt-text: "Runtime alt text",
  fig-subcaptions: ("A", "B"),
  fig-layout-columns: (1fr, 1fr),
  fig-layout-rows: auto,
)[`
print("ignored")
`]
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &["--input", "calepin-results=/.calepin/paper/results.json"],
    );
    assert!(output.exists());
    assert!(std::fs::metadata(&output).unwrap().len() > 0);

    if Command::new("pdftotext").arg("-v").output().is_ok() {
        let text = Command::new("pdftotext")
            .arg(&output)
            .arg("-")
            .output()
            .unwrap();
        assert!(text.status.success());
        let extracted = String::from_utf8(text.stdout).unwrap();
        assert!(extracted.contains("Runtime figure caption"), "{extracted}");
        assert!(extracted.contains("A"), "{extracted}");
        assert!(extracted.contains("B"), "{extracted}");
    }
}

#[test]
fn typst_compile_html_renders_explicit_figure_grid_layout_from_results() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let figures = dir.path().join(".calepin/paper/figures");
    std::fs::create_dir_all(&figures).unwrap();
    std::fs::write(
            figures.join("fig-demo.svg"),
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80"><rect width="120" height="80" fill="#88c0d0"/></svg>"##,
        )
        .unwrap();
    std::fs::write(
            figures.join("fig-demo-2.svg"),
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80"><circle cx="60" cy="40" r="30" fill="#a3be8c"/></svg>"##,
        )
        .unwrap();
    std::fs::create_dir_all(dir.path().join(".calepin/paper")).unwrap();
    std::fs::write(
        dir.path().join(".calepin/paper/results.json"),
        r#"{
  "schema": 1,
  "calepin_version": "test",
  "input": "paper.typ",
  "chunks": {
    "fig-demo": {
      "label": "fig-demo",
      "engine": "python",
      "status": "ok",
      "items": [
        {
          "type": "display",
          "data": {
            "image/svg+xml": {
              "path": "/.calepin/paper/figures/fig-demo.svg"
            }
          }
        },
        {
          "type": "display",
          "data": {
            "image/svg+xml": {
              "path": "/.calepin/paper/figures/fig-demo-2.svg"
            }
          }
        }
      ]
    }
  }
}"#,
    )
    .unwrap();

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.html");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk(
  "python",
  label: "fig-demo",
  echo: false,
  fig-caption: [HTML grid figure],
  fig-subcaptions: ([Panel A], [Panel B]),
  fig-layout-columns: (2fr, 1fr),
  fig-layout-rows: (auto, auto),
)[`print("ignored")`]
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &[
            "--features",
            "html",
            "--input",
            "calepin-target=html",
            "--input",
            "calepin-results=/.calepin/paper/results.json",
        ],
    );
    let html = std::fs::read_to_string(output).unwrap();
    assert!(html.contains(r#"class="calepin-figure-grid""#), "{html}");
    assert!(
        html.contains("grid-template-columns: 2fr 1fr;"),
        "expected explicit grid columns in HTML output:\n{html}"
    );
    assert!(
        html.contains("grid-template-rows: auto auto;"),
        "expected explicit grid rows in HTML output:\n{html}"
    );
    assert!(html.contains("Panel A"), "{html}");
    assert!(html.contains("Panel B"), "{html}");
}

#[test]
fn typst_compile_html_respects_figure_display_dimensions_from_results() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let figures = dir.path().join(".calepin/paper/figures");
    std::fs::create_dir_all(&figures).unwrap();
    std::fs::write(
            figures.join("fig-demo.svg"),
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80"><rect width="120" height="80" fill="#88c0d0"/></svg>"##,
        )
        .unwrap();
    std::fs::create_dir_all(dir.path().join(".calepin/paper")).unwrap();
    std::fs::write(
        dir.path().join(".calepin/paper/results.json"),
        r#"{
  "schema": 1,
  "calepin_version": "test",
  "input": "paper.typ",
  "chunks": {
    "fig-demo": {
      "label": "fig-demo",
      "engine": "python",
      "status": "ok",
      "items": [
        {
          "type": "display",
          "data": {
            "image/svg+xml": {
              "path": "/.calepin/paper/figures/fig-demo.svg"
            }
          }
        }
      ]
    }
  }
}"#,
    )
    .unwrap();

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.html");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk(
  "python",
  label: "fig-demo",
  echo: false,
  fig-width: 37%,
  fig-height: 44pt,
  fig-caption: [HTML sized figure],
)[`print("ignored")`]
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &[
            "--features",
            "html",
            "--input",
            "calepin-target=html",
            "--input",
            "calepin-results=/.calepin/paper/results.json",
            "--input",
            "calepin-assets=http://127.0.0.1:3002",
        ],
    );
    let html = std::fs::read_to_string(output).unwrap();
    assert!(
        html.contains("width: 37%;"),
        "expected display width in HTML output:\n{html}"
    );
    assert!(
        html.contains("calepin-figure-width"),
        "expected a stable sized figure class in HTML output:\n{html}"
    );
    assert!(
        html.contains("height: 44pt"),
        "expected display height in HTML output:\n{html}"
    );
    assert!(
        html.contains("http://127.0.0.1:3002/.calepin/paper/figures/fig-demo.svg"),
        "expected asset server URL in HTML output:\n{html}"
    );
}

#[test]
fn typst_compile_html_accepts_css_string_figure_display_dimensions_from_results() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let figures = dir.path().join(".calepin/paper/figures");
    std::fs::create_dir_all(&figures).unwrap();
    std::fs::write(
            figures.join("fig-demo.svg"),
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80"><rect width="120" height="80" fill="#88c0d0"/></svg>"##,
        )
        .unwrap();
    std::fs::create_dir_all(dir.path().join(".calepin/paper")).unwrap();
    std::fs::write(
        dir.path().join(".calepin/paper/results.json"),
        r#"{
  "schema": 1,
  "calepin_version": "test",
  "input": "paper.typ",
  "chunks": {
    "fig-demo": {
      "label": "fig-demo",
      "engine": "python",
      "status": "ok",
      "items": [
        {
          "type": "display",
          "data": {
            "image/svg+xml": {
              "path": "/.calepin/paper/figures/fig-demo.svg"
            }
          }
        }
      ]
    }
  }
}"#,
    )
    .unwrap();

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.html");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk(
  "python",
  label: "fig-demo",
  echo: false,
  fig-width: "37%",
  fig-height: "44px",
  fig-caption: [HTML CSS sized figure],
)[`print("ignored")`]
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &[
            "--features",
            "html",
            "--input",
            "calepin-target=html",
            "--input",
            "calepin-results=/.calepin/paper/results.json",
        ],
    );
    let html = std::fs::read_to_string(output).unwrap();
    assert!(
        html.contains("width: 37%;"),
        "expected display width in HTML output:\n{html}"
    );
    assert!(
        html.contains("height: 44px"),
        "expected display height in HTML output:\n{html}"
    );
}

#[test]
fn typst_compile_paged_respects_string_figure_width_from_results() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let figures = dir.path().join(".calepin/paper/figures");
    std::fs::create_dir_all(&figures).unwrap();
    std::fs::write(
        figures.join("fig-demo.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50"><rect width="100" height="50" fill="#88c0d0"/></svg>"##,
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join(".calepin/paper")).unwrap();
    std::fs::write(
        dir.path().join(".calepin/paper/results.json"),
        r#"{
  "schema": 1,
  "calepin_version": "test",
  "input": "paper.typ",
  "chunks": {
    "fig-demo": {
      "label": "fig-demo",
      "engine": "python",
      "status": "ok",
      "options": {
        "echo": false,
        "output": true,
        "results": "render",
        "warning": true,
        "message": true,
        "placeholder": true,
        "fig-width": "10%",
        "fig-height": null,
        "fig-align": "center",
        "fig-responsive": true,
        "fig-link": null,
        "fig-caption": null,
        "fig-cap-location": null,
        "fig-alt-text": null,
        "fig-subcaptions": null,
        "fig-layout-columns": null,
        "fig-layout-rows": null,
        "kind": null
      },
      "items": [
        {
          "type": "display",
          "data": {
            "image/svg+xml": {
              "path": "/.calepin/paper/figures/fig-demo.svg"
            }
          }
        }
      ]
    }
  }
}"#,
    )
    .unwrap();

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.svg");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#set page(width: 400pt, height: 200pt, margin: 0pt)

#calepin.chunk("python", label: "fig-demo", echo: false)[
`print("ignored")`
]
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &["--input", "calepin-results=/.calepin/paper/results.json"],
    );
    let svg = std::fs::read_to_string(output).unwrap();
    assert!(
        svg.contains(r#"width="40""#),
        "expected 10% of the 400pt page width, got:\n{svg}"
    );
}

#[test]
fn typst_query_emits_crossref_labels_for_array_label() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let input = dir.path().join("paper.typ");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("r", label: ("fig-x", "lst-y"))[```r
1+1
```]
"##,
    )
    .unwrap();

    let stdout = typst_query(dir.path(), &input, "<calepin-chunk>");
    assert!(stdout.contains(r#""crossref-labels""#), "{stdout}");
    assert!(stdout.contains(r#""fig-x""#), "{stdout}");
    assert!(stdout.contains(r#""lst-y""#), "{stdout}");
    // internal id is the primary (first) label
    assert!(stdout.contains(r#""label": "fig-x""#), "{stdout}");
}

#[test]
fn typst_query_emits_crossref_labels_for_string_label() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let input = dir.path().join("paper.typ");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("r", label: "fig-solo")[```r
1+1
```]
"##,
    )
    .unwrap();

    let stdout = typst_query(dir.path(), &input, "<calepin-chunk>");
    // a single string label produces a one-element crossref-labels list
    assert!(stdout.contains(r#""crossref-labels""#), "{stdout}");
    assert!(stdout.contains(r#""fig-solo""#), "{stdout}");
    assert!(stdout.contains(r#""label": "fig-solo""#), "{stdout}");
}

#[test]
fn typst_query_emits_crossref_labels_for_qmd_label() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let input = dir.path().join("paper.typ");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("r")[```r
#| label: fig-qmd
1+1
```]
"##,
    )
    .unwrap();

    let stdout = typst_query(dir.path(), &input, "<calepin-chunk>");
    assert!(stdout.contains(r#""crossref-labels""#), "{stdout}");
    assert!(stdout.contains(r#""fig-qmd""#), "{stdout}");
    assert!(stdout.contains(r#""label": "fig-qmd""#), "{stdout}");
}

#[test]
fn typst_query_emits_crossref_labels_for_trailing_fence_label_metadata() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let input = dir.path().join("paper.typ");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("r")[```r
1+1
``` #metadata((label: "fig-trailing")) <calepin-fence-label>]
"##,
    )
    .unwrap();

    let stdout = typst_query(dir.path(), &input, "<calepin-chunk>");
    assert!(stdout.contains(r#""crossref-labels""#), "{stdout}");
    assert!(stdout.contains(r#""fig-trailing""#), "{stdout}");
    assert!(stdout.contains(r#""label": "fig-trailing""#), "{stdout}");
}

#[test]
fn typst_compile_html_applies_default_figure_display_options_from_results() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    let figures = dir.path().join(".calepin/paper/figures");
    std::fs::create_dir_all(&figures).unwrap();
    std::fs::write(
            figures.join("fig-demo.svg"),
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80"><rect width="120" height="80" fill="#88c0d0"/></svg>"##,
        )
        .unwrap();
    std::fs::create_dir_all(dir.path().join(".calepin/paper")).unwrap();
    std::fs::write(
        dir.path().join(".calepin/paper/results.json"),
        r#"{
  "schema": 1,
  "calepin_version": "test",
  "input": "paper.typ",
  "chunks": {
    "fig-demo": {
      "label": "fig-demo",
      "engine": "python",
      "status": "ok",
      "items": [
        {
          "type": "display",
          "data": {
            "image/svg+xml": {
              "path": "/.calepin/paper/figures/fig-demo.svg"
            }
          }
        }
      ]
    }
  }
}"#,
    )
    .unwrap();

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.html");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk(
  "python",
  label: "fig-demo",
  echo: false,
  fig-caption: [HTML default sized figure],
)[`print("ignored")`]
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &[
            "--features",
            "html",
            "--input",
            "calepin-target=html",
            "--input",
            "calepin-results=/.calepin/paper/results.json",
        ],
    );
    let html = std::fs::read_to_string(output).unwrap();
    assert!(
        html.contains("width: 70%;"),
        "expected default display width in HTML output:\n{html}"
    );
    assert!(
        html.contains("margin-inline: auto;"),
        "expected centered default image in HTML output:\n{html}"
    );
}

// --- Deferred / relocated chunk output (#calepin.results) ---

fn typst_compile_output(
    root: &Path,
    input: &Path,
    output: &Path,
    args: &[&str],
) -> std::process::Output {
    let mut command = Command::new("typst");
    command
        .arg("compile")
        .arg(input)
        .arg(output)
        .arg("--root")
        .arg(root);
    for arg in args {
        command.arg(arg);
    }
    command.output().unwrap()
}

fn write_stream_results(dir: &Path, label: &str, text: &str) {
    std::fs::create_dir_all(dir.join(".calepin/paper")).unwrap();
    std::fs::write(
        dir.join(".calepin/paper/results.json"),
        format!(
            r#"{{
  "schema": 1,
  "calepin_version": "test",
  "input": "paper.typ",
  "chunks": {{
    "{label}": {{
      "label": "{label}",
      "engine": "python",
      "status": "ok",
      "items": [
        {{ "type": "stream", "name": "stdout", "text": "{text}" }}
      ]
    }}
  }}
}}"#
        ),
    )
    .unwrap();
}

fn write_figure_results(dir: &Path, label: &str) {
    let figures = dir.join(".calepin/paper/figures");
    std::fs::create_dir_all(&figures).unwrap();
    std::fs::write(
        figures.join(format!("{label}.svg")),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80"><rect width="120" height="80" fill="#88c0d0"/></svg>"##,
    )
    .unwrap();
    std::fs::create_dir_all(dir.join(".calepin/paper")).unwrap();
    std::fs::write(
        dir.join(".calepin/paper/results.json"),
        format!(
            r#"{{
  "schema": 1,
  "calepin_version": "test",
  "input": "paper.typ",
  "chunks": {{
    "{label}": {{
      "label": "{label}",
      "engine": "python",
      "status": "ok",
      "crossref-labels": [{{ "kind": "fig", "name": "{label}" }}],
      "items": [
        {{ "type": "display", "data": {{ "image/svg+xml": {{ "path": "/.calepin/paper/figures/{label}.svg" }} }} }}
      ]
    }}
  }}
}}"#
        ),
    )
    .unwrap();
}

const RELOCATE_RESULTS_INPUT: &str = "calepin-results=/.calepin/paper/results.json";

#[test]
fn typst_compile_relocates_hidden_chunk_output_once() {
    skip_if_no_typst!();
    if Command::new("pdftotext").arg("-v").output().is_err() {
        return;
    }

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    write_stream_results(dir.path(), "greeting", "RELOCATED_12345");

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.pdf");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("python", label: "greeting", echo: false, results: "hide")[`
pass
`]

Body text before relocation.

#calepin.results("greeting")
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &["--input", RELOCATE_RESULTS_INPUT],
    );
    let extracted = pdf_text(&output);
    let count = extracted.matches("RELOCATED_12345").count();
    assert_eq!(
        count, 1,
        "expected relocated output exactly once: {extracted}"
    );
}

#[test]
fn typst_compile_hidden_chunk_without_relocation_suppresses_output() {
    skip_if_no_typst!();
    if Command::new("pdftotext").arg("-v").output().is_err() {
        return;
    }

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    write_stream_results(dir.path(), "greeting", "SUPPRESSED_12345");

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.pdf");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("python", label: "greeting", echo: false, results: "hide")[`
pass
`]

Nothing should appear from the chunk.
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &["--input", RELOCATE_RESULTS_INPUT],
    );
    let extracted = pdf_text(&output);
    assert!(
        !extracted.contains("SUPPRESSED_12345"),
        "hidden chunk output should not appear inline: {extracted}"
    );
}

#[test]
fn typst_compile_hidden_results_alias_suppresses_inline_output() {
    skip_if_no_typst!();
    if Command::new("pdftotext").arg("-v").output().is_err() {
        return;
    }

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    write_stream_results(dir.path(), "greeting", "ALIAS_12345");

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.pdf");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("python", label: "greeting", echo: true, results: "hidden")[`
pass
`]

The result appears only at the relocation:

#calepin.results("greeting")
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &["--input", RELOCATE_RESULTS_INPUT],
    );
    let extracted = pdf_text(&output);
    let count = extracted.matches("ALIAS_12345").count();
    assert_eq!(count, 1, "expected relocated output once: {extracted}");
    assert!(
        extracted.contains("pass"),
        "expected visible source: {extracted}"
    );
}

#[test]
fn typst_compile_relocates_output_twice() {
    skip_if_no_typst!();
    if Command::new("pdftotext").arg("-v").output().is_err() {
        return;
    }

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    write_stream_results(dir.path(), "greeting", "TWICE_12345");

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.pdf");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("python", label: "greeting", echo: false, results: "hide")[`
pass
`]

First copy:

#calepin.results("greeting")

Second copy:

#calepin.results("greeting")
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &["--input", RELOCATE_RESULTS_INPUT],
    );
    let extracted = pdf_text(&output);
    let count = extracted.matches("TWICE_12345").count();
    assert_eq!(count, 2, "expected relocated output twice: {extracted}");
}

#[test]
fn typst_compile_relocation_before_chunk_renders() {
    skip_if_no_typst!();
    if Command::new("pdftotext").arg("-v").output().is_err() {
        return;
    }

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    write_stream_results(dir.path(), "greeting", "FORWARD_12345");

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.pdf");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

The output is shown here, before the chunk is defined:

#calepin.results("greeting")

#calepin.chunk("python", label: "greeting", echo: false, results: "hide")[`
pass
`]
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &["--input", RELOCATE_RESULTS_INPUT],
    );
    let extracted = pdf_text(&output);
    let count = extracted.matches("FORWARD_12345").count();
    assert_eq!(
        count, 1,
        "forward reference should render the output once: {extracted}"
    );
}

#[test]
fn typst_compile_relocated_hidden_figure_resolves_crossref() {
    skip_if_no_typst!();
    if Command::new("pdftotext").arg("-v").output().is_err() {
        return;
    }

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    write_figure_results(dir.path(), "fig-plot");

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.pdf");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("python", label: "fig-plot", echo: false, results: "hide", fig-caption: [Relocated plot caption])[`
pass
`]

See @fig-plot for details.

#calepin.results("fig-plot")
"##,
    )
    .unwrap();

    // Compile succeeds only if @fig-plot resolves to the relocated figure.
    typst_compile(
        dir.path(),
        &input,
        &output,
        &["--input", RELOCATE_RESULTS_INPUT],
    );
    let extracted = pdf_text(&output);
    assert!(
        extracted.contains("Relocated plot caption"),
        "expected the relocated figure caption: {extracted}"
    );
    assert!(
        extracted.contains("Figure"),
        "expected @fig-plot to resolve to a figure reference: {extracted}"
    );
}

#[test]
fn typst_compile_referencing_figure_shown_in_two_places_errors() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    write_figure_results(dir.path(), "fig-plot");

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.pdf");
    // The hidden figure is printed in two places, so its anchor is defined
    // twice; referencing it is ambiguous and Typst reports it directly.
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("python", label: "fig-plot", echo: false, results: "hide")[`
pass
`]

See @fig-plot.

#calepin.results("fig-plot")

#calepin.results("fig-plot")
"##,
    )
    .unwrap();

    let result = typst_compile_output(
        dir.path(),
        &input,
        &output,
        &["--input", RELOCATE_RESULTS_INPUT],
    );
    assert!(!result.status.success(), "expected a duplicate-label error");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("occurs multiple times"),
        "expected Typst's duplicate-label error, got: {stderr}"
    );
}

#[test]
fn typst_compile_hidden_figure_relocated_twice_unreferenced_ok() {
    skip_if_no_typst!();
    if Command::new("pdftotext").arg("-v").output().is_err() {
        return;
    }

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    write_figure_results(dir.path(), "fig-plot");

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.pdf");
    // The figure is printed in two places but never referenced, so the repeated
    // anchor is harmless and the document compiles.
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("python", label: "fig-plot", echo: false, results: "hide", fig-caption: [Repeated caption])[`
pass
`]

#calepin.results("fig-plot")

#calepin.results("fig-plot")
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &["--input", RELOCATE_RESULTS_INPUT],
    );
    let extracted = pdf_text(&output);
    let count = extracted.matches("Repeated caption").count();
    assert_eq!(count, 2, "expected the figure rendered twice: {extracted}");
}

#[test]
fn typst_compile_relocation_unknown_label_errors() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    write_stream_results(dir.path(), "greeting", "IGNORED");

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.pdf");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.results("does-not-exist")
"##,
    )
    .unwrap();

    let result = typst_compile_output(
        dir.path(),
        &input,
        &output,
        &["--input", RELOCATE_RESULTS_INPUT],
    );
    assert!(!result.status.success(), "expected a missing-label error");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("does-not-exist"),
        "expected the missing label in the error: {stderr}"
    );
}

#[test]
fn typst_compile_html_relocates_hidden_chunk_with_stored_options() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();
    std::fs::create_dir_all(dir.path().join(".calepin/paper")).unwrap();
    // Realistic results.json: the chunk's stored options carry results: "hide",
    // which the HTML merge path re-reads. The relocation must still show it.
    std::fs::write(
        dir.path().join(".calepin/paper/results.json"),
        r#"{
  "schema": 1,
  "calepin_version": "test",
  "input": "paper.typ",
  "chunks": {
    "greeting": {
      "label": "greeting",
      "engine": "python",
      "status": "ok",
      "options": {
        "echo": false, "output": true, "results": "hide", "warning": true,
        "message": true, "placeholder": true, "fig-width": null, "fig-height": null,
        "fig-align": null, "fig-responsive": null, "fig-link": null, "fig-caption": null,
        "fig-cap-location": null, "fig-alt-text": null, "fig-subcaptions": null,
        "fig-layout-columns": null, "fig-layout-rows": null, "kind": null
      },
      "items": [
        { "type": "stream", "name": "stdout", "text": "HTMLRELOCATE_12345" }
      ]
    }
  }
}"#,
    )
    .unwrap();

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.html");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("python", label: "greeting", echo: false, results: "hide")[`
pass
`]

#calepin.results("greeting")
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &[
            "--features",
            "html",
            "--input",
            "calepin-target=html",
            "--input",
            RELOCATE_RESULTS_INPUT,
        ],
    );
    let html = std::fs::read_to_string(output).unwrap();
    let count = html.matches("HTMLRELOCATE_12345").count();
    assert_eq!(count, 1, "expected relocated output once in HTML:\n{html}");
}

#[test]
fn typst_compile_html_sidenote_links_marker_to_note() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.html");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

Text#calepin.elements.sidenote[A margin remark.] continues.
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &["--features", "html", "--input", "calepin-target=html"],
    );
    let html = std::fs::read_to_string(output).unwrap();
    assert!(
        html.contains("A margin remark."),
        "expected the note body in HTML output:\n{html}"
    );
    assert!(
        html.contains("doc-noteref"),
        "expected a doc-noteref marker:\n{html}"
    );
    assert!(
        html.contains("class=sidenote") || html.contains(r#"class="sidenote""#),
        "expected a sidenote container:\n{html}"
    );
    // Marker and note are associated by a shared id.
    assert!(
        html.contains("#sn-1") && (html.contains("id=sn-1") || html.contains(r#"id="sn-1""#)),
        "expected the marker href to target the note id:\n{html}"
    );
}

#[test]
fn typst_compile_html_sidenote_unnumbered_emits_marginnote() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.html");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

Text#calepin.elements.sidenote(numbering: none)[An unnumbered aside.] ends.
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &["--features", "html", "--input", "calepin-target=html"],
    );
    let html = std::fs::read_to_string(output).unwrap();
    assert!(
        html.contains("An unnumbered aside."),
        "expected the note body in HTML output:\n{html}"
    );
    assert!(
        html.contains("class=marginnote") || html.contains(r#"class="marginnote""#),
        "expected a marginnote container:\n{html}"
    );
    assert!(
        !html.contains("doc-noteref"),
        "an unnumbered note should not emit a marker:\n{html}"
    );
}

#[test]
fn typst_compile_html_sidefigure_emits_margin_figure_with_caption() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();

    let input = dir.path().join("paper.typ");
    let output = dir.path().join("paper.html");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.elements.sidefigure(caption: [A side caption.])[
  #html.elem("img", attrs: (src: "fig.png", alt: "demo"))
]
"##,
    )
    .unwrap();

    typst_compile(
        dir.path(),
        &input,
        &output,
        &["--features", "html", "--input", "calepin-target=html"],
    );
    let html = std::fs::read_to_string(output).unwrap();
    assert!(
        html.contains("class=margin-figure") || html.contains(r#"class="margin-figure""#),
        "expected a margin-figure container:\n{html}"
    );
    assert!(
        html.contains("<figure"),
        "expected a figure element:\n{html}"
    );
    assert!(
        html.contains("<figcaption"),
        "expected a figcaption:\n{html}"
    );
    assert!(
        html.contains("A side caption."),
        "expected the caption text:\n{html}"
    );
}

#[test]
fn typst_query_paged_sidenote_falls_back_to_footnote() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();

    let input = dir.path().join("paper.typ");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

Body text#calepin.elements.sidenote[A footnote fallback remark.] here.
"##,
    )
    .unwrap();

    let json = typst_query_with_inputs(
        dir.path(),
        &input,
        "footnote",
        &["calepin-mode=render", "calepin-target=paged"],
    );
    assert!(
        json.contains("A footnote fallback remark."),
        "expected the paged sidenote to fall back to a footnote:\n{json}"
    );
}

#[test]
fn typst_query_paged_sidenote_uses_installed_margin_impl() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();

    let input = dir.path().join("paper.typ");
    // A theme installs its own margin placement; the element must use it
    // instead of the default footnote fallback.
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.elements.set-margin-impl(note: (body, numbering: auto, side: auto) => [#metadata("MARGIN_IMPL_USED")<margin-probe> #body])

Body#calepin.elements.sidenote[a remark] here.
"##,
    )
    .unwrap();

    let json = typst_query_with_inputs(
        dir.path(),
        &input,
        "<margin-probe>",
        &["calepin-mode=render", "calepin-target=paged"],
    );
    assert!(
        json.contains("MARGIN_IMPL_USED"),
        "expected the installed margin implementation to be used:\n{json}"
    );
}

#[test]
fn typst_query_margin_elements_pass_body_through_in_query_mode() {
    skip_if_no_typst!();

    let dir = typst_accessible_tempdir();
    write_runtime(dir.path()).unwrap();

    let input = dir.path().join("paper.typ");
    // In the query pass the elements must be transparent so chunk metadata
    // inside a margin element is still extracted.
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

Body#calepin.elements.sidenote[#metadata("INSIDE_NOTE")<note-probe>] and
#calepin.elements.sidefigure[#metadata("INSIDE_FIGURE")<figure-probe>].
"##,
    )
    .unwrap();

    let note = typst_query(dir.path(), &input, "<note-probe>");
    assert!(
        note.contains("INSIDE_NOTE"),
        "expected the sidenote body to pass through in query mode:\n{note}"
    );
    let figure = typst_query(dir.path(), &input, "<figure-probe>");
    assert!(
        figure.contains("INSIDE_FIGURE"),
        "expected the sidefigure body to pass through in query mode:\n{figure}"
    );
}
