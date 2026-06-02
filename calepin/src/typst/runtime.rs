use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub const RUNTIME_SOURCE: &str = concat!(
    include_str!("runtime/template.typ"),
    include_str!("runtime/themes.typ"),
    include_str!("runtime/state.typ"),
    include_str!("runtime/render.typ"),
    include_str!("runtime/options.typ"),
    include_str!("runtime/chunk.typ"),
);

pub fn write_runtime(root: &Path) -> Result<PathBuf> {
    let calepin_dir = root.join(".calepin");
    std::fs::create_dir_all(&calepin_dir)
        .with_context(|| format!("failed to create {}", calepin_dir.display()))?;
    let path = calepin_dir.join("calepin.typ");
    if std::fs::read_to_string(&path).is_ok_and(|existing| existing == RUNTIME_SOURCE) {
        return Ok(path);
    }
    std::fs::write(&path, RUNTIME_SOURCE)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    fn typst_accessible_tempdir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("calepin-runtime-test-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .unwrap()
    }

    #[test]
    fn write_runtime_writes_calepin_typ() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_runtime(dir.path()).unwrap();
        assert_eq!(path, dir.path().join(".calepin").join("calepin.typ"));
        let written = std::fs::read_to_string(path).unwrap();
        assert_eq!(written, RUNTIME_SOURCE);
    }

    #[test]
    fn typst_query_emits_chunk_metadata() {
        if Command::new("typst").arg("--version").output().is_err() {
            return;
        }

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

        let output = Command::new("typst")
            .arg("query")
            .arg(&input)
            .arg("<calepin-chunk>")
            .arg("--root")
            .arg(dir.path())
            .arg("--input")
            .arg("calepin-mode=query")
            .arg("--pretty")
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
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
        if Command::new("typst").arg("--version").output().is_err() {
            return;
        }

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

        let output = Command::new("typst")
            .arg("query")
            .arg(&input)
            .arg("<calepin-chunk>")
            .arg("--root")
            .arg(dir.path())
            .arg("--input")
            .arg("calepin-mode=query")
            .arg("--pretty")
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains(r#""label": "inferred""#), "{}", stdout);
        assert!(stdout.contains(r#""engine": "r""#), "{}", stdout);
    }

    #[test]
    fn typst_query_supports_plain_python_raw_blocks_when_enabled() {
        if Command::new("typst").arg("--version").output().is_err() {
            return;
        }

        let dir = typst_accessible_tempdir();
        write_runtime(dir.path()).unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(
            &input,
            r##"#import ".calepin/calepin.typ"

#calepin.setup(raw-chunks: true)

```python
print("hello")
```
"##,
        )
        .unwrap();

        let output = Command::new("typst")
            .arg("query")
            .arg(&input)
            .arg("raw.where(block: true).or(<calepin-chunk>)")
            .arg("--root")
            .arg(dir.path())
            .arg("--input")
            .arg("calepin-mode=query")
            .arg("--pretty")
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains(r#""func": "raw""#), "{}", stdout);
        assert!(stdout.contains(r#""lang": "python""#), "{}", stdout);
        assert!(
            stdout.contains(r#""text": "print(\"hello\")""#),
            "{}",
            stdout
        );
    }

    #[test]
    fn typst_query_supports_raw_chunks_engine_option() {
        if Command::new("typst").arg("--version").output().is_err() {
            return;
        }

        let dir = typst_accessible_tempdir();
        write_runtime(dir.path()).unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(
            &input,
            r##"#import ".calepin/calepin.typ"

#calepin.setup(raw-chunks: "python")
"##,
        )
        .unwrap();

        let output = Command::new("typst")
            .arg("query")
            .arg(&input)
            .arg("<calepin-config>")
            .arg("--root")
            .arg(dir.path())
            .arg("--input")
            .arg("calepin-mode=query")
            .arg("--pretty")
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains(r#""raw-chunks": "python""#), "{}", stdout);
    }

    #[test]
    fn typst_query_plain_raw_chunks_does_not_reprocess_chunk_body() {
        if Command::new("typst").arg("--version").output().is_err() {
            return;
        }

        let dir = typst_accessible_tempdir();
        write_runtime(dir.path()).unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(
            &input,
            r##"#import ".calepin/calepin.typ"

#calepin.setup(raw-chunks: "r", results: "verbatim")

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

#calepin.chunk("python", echo: false, results: "asis")[
```python
print("hello from python")
```]
"##,
        )
        .unwrap();

        let output = Command::new("typst")
            .arg("query")
            .arg(&input)
            .arg("<calepin-chunk>")
            .arg("--root")
            .arg(dir.path())
            .arg("--input")
            .arg("calepin-mode=query")
            .arg("--pretty")
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains(r#""label": "no-run-r""#), "{}", stdout);
        assert!(stdout.contains(r#""label": "inline-1""#), "{}", stdout);
        assert!(stdout.contains(r#""label": "chunk-1""#), "{}", stdout);
        assert!(!stdout.contains(r#""label": "chunk-3""#), "{}", stdout);
        assert!(!stdout.contains(r#""label": "chunk-2""#), "{}", stdout);
    }

    #[test]
    fn typst_compile_without_results_shows_code() {
        if Command::new("typst").arg("--version").output().is_err() {
            return;
        }
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

        let status = Command::new("typst")
            .arg("compile")
            .arg(&input)
            .arg(&output)
            .arg("--root")
            .arg(dir.path())
            .status()
            .unwrap();

        assert!(status.success());
        let text = Command::new("pdftotext")
            .arg(&output)
            .arg("-")
            .output()
            .unwrap();
        assert!(text.status.success());
        let extracted = String::from_utf8(text.stdout).unwrap();
        assert!(
            extracted.contains("FALLBACK_12345"),
            "expected source code in PDF output"
        );
        assert!(!extracted.contains("Calepin output is missing."));
    }

    #[test]
    fn typst_compile_with_results_and_echo_shows_both() {
        if Command::new("typst").arg("--version").output().is_err() {
            return;
        }
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

        let status = Command::new("typst")
            .arg("compile")
            .arg(&input)
            .arg(&output)
            .arg("--root")
            .arg(dir.path())
            .status()
            .unwrap();

        assert!(status.success());
        let text = Command::new("pdftotext")
            .arg(&output)
            .arg("-")
            .output()
            .unwrap();
        assert!(text.status.success());
        let extracted = String::from_utf8(text.stdout).unwrap();
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
    fn typst_compile_renders_canonical_figure_options_from_results() {
        if Command::new("typst").arg("--version").output().is_err() {
            return;
        }

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
        let output = dir.path().join("paper.pdf");
        std::fs::write(
            &input,
            r##"#import ".calepin/calepin.typ"

#calepin.chunk(
  "python",
  label: "fig-demo",
  echo: false,
  fig-display-width: 50%,
  fig-display-align: center,
  fig-display-responsive: true,
  fig-display-link: "https://example.com",
  fig-caption: [Runtime figure caption],
  fig-caption-position: top,
  fig-alt-text: "Runtime alt text",
  fig-subcaptions: ("A", "B"),
  fig-layout-columns: (1fr, 1fr),
  fig-layout-rows: auto,
  fig-layout-design: "A B",
)[`
print("ignored")
`]
"##,
        )
        .unwrap();

        let compile = Command::new("typst")
            .arg("compile")
            .arg(&input)
            .arg(&output)
            .arg("--root")
            .arg(dir.path())
            .arg("--input")
            .arg("calepin-results=/.calepin/paper/results.json")
            .output()
            .unwrap();

        assert!(
            compile.status.success(),
            "{}",
            String::from_utf8_lossy(&compile.stderr)
        );
        assert!(output.exists());
        assert!(std::fs::metadata(output).unwrap().len() > 0);
    }

    #[test]
    fn typst_compile_html_respects_figure_display_dimensions_from_results() {
        if Command::new("typst").arg("--version").output().is_err() {
            return;
        }

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

#calepin.html[
  #calepin.chunk(
    "python",
    label: "fig-demo",
    echo: false,
    fig-display-width: 37%,
    fig-display-height: 44pt,
    fig-caption: [HTML sized figure],
  )[`print("ignored")`]
]
"##,
        )
        .unwrap();

        let compile = Command::new("typst")
            .arg("compile")
            .arg(&input)
            .arg(&output)
            .arg("--root")
            .arg(dir.path())
            .arg("--features")
            .arg("html")
            .arg("--input")
            .arg("calepin-target=html")
            .arg("--input")
            .arg("calepin-results=/.calepin/paper/results.json")
            .arg("--input")
            .arg("calepin-assets=http://127.0.0.1:3002")
            .output()
            .unwrap();

        assert!(
            compile.status.success(),
            "{}",
            String::from_utf8_lossy(&compile.stderr)
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
        if Command::new("typst").arg("--version").output().is_err() {
            return;
        }

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

#calepin.html[
  #calepin.chunk(
    "python",
    label: "fig-demo",
    echo: false,
    fig-display-width: "37%",
    fig-display-height: "44px",
    fig-caption: [HTML CSS sized figure],
  )[`print("ignored")`]
]
"##,
        )
        .unwrap();

        let compile = Command::new("typst")
            .arg("compile")
            .arg(&input)
            .arg(&output)
            .arg("--root")
            .arg(dir.path())
            .arg("--features")
            .arg("html")
            .arg("--input")
            .arg("calepin-target=html")
            .arg("--input")
            .arg("calepin-results=/.calepin/paper/results.json")
            .output()
            .unwrap();

        assert!(
            compile.status.success(),
            "{}",
            String::from_utf8_lossy(&compile.stderr)
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
    fn typst_compile_html_applies_default_figure_display_options_from_results() {
        if Command::new("typst").arg("--version").output().is_err() {
            return;
        }

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

#calepin.html[
  #calepin.chunk(
    "python",
    label: "fig-demo",
    echo: false,
    fig-caption: [HTML default sized figure],
  )[`print("ignored")`]
]
"##,
        )
        .unwrap();

        let compile = Command::new("typst")
            .arg("compile")
            .arg(&input)
            .arg(&output)
            .arg("--root")
            .arg(dir.path())
            .arg("--features")
            .arg("html")
            .arg("--input")
            .arg("calepin-target=html")
            .arg("--input")
            .arg("calepin-results=/.calepin/paper/results.json")
            .output()
            .unwrap();

        assert!(
            compile.status.success(),
            "{}",
            String::from_utf8_lossy(&compile.stderr)
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
}
