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
    let written = std::fs::read_to_string(path).unwrap();
    let expected = runtime_source().unwrap();
    assert_eq!(written, expected);
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
