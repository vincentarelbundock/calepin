use std::path::Path;
use std::process::Command;

fn calepin_bin() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_calepin"))
}

fn has_command(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn has_pdftotext() -> bool {
    Command::new("pdftotext")
        .arg("-v")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn has_python_module(module: &str) -> bool {
    let code = format!("import {module}");
    Command::new("python3")
        .args(["-c", &code])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn typst_accessible_tempdir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("calepin-typst-test-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap()
}

#[test]
fn preprocess_writes_runtime_and_results() {
    if !has_command("typst") || !has_command("python3") {
        return;
    }

    let dir = typst_accessible_tempdir();
    let input = dir.path().join("paper.typ");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("python", echo: false)[```
x = 41
print(x + 1)
```]
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");

    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.path().join(".calepin/calepin.typ").exists());

    let results_path = dir.path().join(".calepin/paper/results.json");
    let results: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(results_path).unwrap()).unwrap();
    assert_eq!(results["schema"], 1);
    assert_eq!(results["chunks"]["chunk-1"]["engine"], "python");
    assert!(results["chunks"]["chunk-1"].get("cached").is_none());
    assert_eq!(results["chunks"]["chunk-1"]["items"][0]["type"], "stream");
    assert_eq!(results["chunks"]["chunk-1"]["items"][0]["text"], "42");
}

#[test]
fn compile_cache_refreshes_render_only_chunk_options() {
    if !has_command("typst") || !has_command("python3") {
        return;
    }

    let dir = typst_accessible_tempdir();
    let input = dir.path().join("paper.typ");
    let write_source = |fig_width: &str| {
        std::fs::write(
            &input,
            format!(
                r#"#import ".calepin/calepin.typ"

#calepin.setup(echo: false)

```python
#| fig-width: {fig_width}
print("cached")
```
"#
            ),
        )
        .unwrap();
    };

    write_source("70%");
    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");
    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    write_source("10%");
    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");
    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let results_path = dir.path().join(".calepin/paper/results.json");
    let results: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(results_path).unwrap()).unwrap();
    assert_eq!(results["chunks"]["chunk-1"]["options"]["fig-width"], "10%");
    assert_eq!(results["chunks"]["chunk-1"]["items"][0]["text"], "cached");
}

#[test]
fn compile_rejects_preview_package_import_with_migration_message() {
    if !has_command("typst") || !has_command("python3") {
        return;
    }

    let dir = typst_accessible_tempdir();
    let input = dir.path().join("paper.typ");
    std::fs::write(
        &input,
        r##"#import "@preview/calepin:0.0.1" as cp

#cp.chunk("python", echo: false)[```
print(42)
```]
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");

    assert!(!output.status.success(), "compile unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unsupported Calepin Typst package import"),
        "{stderr}"
    );
    assert!(
        stderr.contains(r#"#import "/.calepin/calepin.typ" as cp"#),
        "{stderr}"
    );
}

#[test]
fn compile_runs_preprocess_and_typst_compile() {
    if !has_command("typst") || !has_command("python3") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

= Calepin

#calepin.chunk("python", echo: false, results: "typst")[```
print("#strong[42]")
```]
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");

    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let pdf = dir.path().join("paper.pdf");
    assert!(pdf.exists());
    assert!(std::fs::metadata(pdf).unwrap().len() > 0);
}

#[test]
fn compile_html_uses_default_theme() {
    if !has_command("typst") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

#calepin.setup(
  eval: false,
)

Theme is a compile-time concern.
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args([
            "compile",
            "paper.typ",
            "paper.html",
            "--format",
            "html",
            "--quiet",
        ])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");

    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let html = std::fs::read_to_string(dir.path().join("paper.html")).unwrap();
    assert!(
        html.contains("href=\"https://cdn.jsdelivr.net/npm/@picocss/pico@2/css/pico.min.css\""),
        "{html}"
    );
    assert!(html.contains("calepin-document-main"), "{html}");
    assert!(html.contains("Theme is a compile-time concern."), "{html}");
}

#[test]
fn compile_html_config_styles_append_after_theme_css() {
    if !has_command("typst") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::create_dir_all(dir.path().join("styles")).unwrap();
    std::fs::write(
        dir.path().join("paper.typ"),
        r#"#set document(title: [Paper])
Hello
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("calepin.toml"),
        r#"
theme = "academic"
styles = ["styles/site.css"]
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("styles/site.css"),
        ":root { --config-style-marker: yes; }",
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args([
            "compile",
            "paper.typ",
            "paper.html",
            "--format",
            "html",
            "--config",
            "calepin.toml",
            "--quiet",
        ])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");
    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let html = std::fs::read_to_string(dir.path().join("paper.html")).unwrap();
    let theme_pos = html.find("--calepin-color-background").unwrap();
    let config_pos = html.find("--config-style-marker").unwrap();
    assert!(config_pos > theme_pos, "{html}");
}

#[test]
fn compile_html_theme_false_still_applies_config_styles() {
    if !has_command("typst") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::create_dir_all(dir.path().join("styles")).unwrap();
    std::fs::write(
        dir.path().join("paper.typ"),
        r#"#set document(title: [Paper])
Hello
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("calepin.toml"),
        r#"
theme = false
styles = ["styles/raw.css"]
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("styles/raw.css"),
        "body { --raw-style-marker: yes; }",
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args([
            "compile",
            "paper.typ",
            "paper.html",
            "--format",
            "html",
            "--config",
            "calepin.toml",
            "--quiet",
        ])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");
    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let html = std::fs::read_to_string(dir.path().join("paper.html")).unwrap();
    assert!(html.contains("--raw-style-marker"), "{html}");
    assert!(!html.contains("calepin-copy-code"), "{html}");
}

#[test]
fn compile_pdf_ignores_config_styles() {
    if !has_command("typst") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::create_dir_all(dir.path().join("styles")).unwrap();
    std::fs::write(
        dir.path().join("paper.typ"),
        r#"#set document(title: [Paper])
Hello
"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("calepin.toml"), r#"styles = ["styles/site.css"]"#)
        .unwrap();
    std::fs::write(dir.path().join("styles/site.css"), "body { color: red; }").unwrap();

    let output = Command::new(calepin_bin())
        .args([
            "compile",
            "paper.typ",
            "paper.pdf",
            "--format",
            "pdf",
            "--config",
            "calepin.toml",
            "--quiet",
        ])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");
    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.path().join("paper.pdf").is_file());
}

#[test]
fn compile_html_respects_canonical_figure_display_dimensions() {
    if !has_command("typst")
        || Command::new("dot")
            .arg("-V")
            .output()
            .map(|output| !output.status.success())
            .unwrap_or(true)
    {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

See @fig-graph.

#calepin.chunk(
  "dot",
  label: "fig-graph",
  echo: false,
  fig-width: "37%",
  fig-height: "44px",
  fig-caption: [HTML graph],
)[```
digraph {
  a -> b
}
```]
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args([
            "compile",
            "paper.typ",
            "paper.html",
            "--format",
            "html",
            "--quiet",
        ])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");

    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let html = std::fs::read_to_string(dir.path().join("paper.html")).unwrap();
    assert!(
        html.contains(r#"<div style="width: 37%; max-width: 100%; margin-inline: auto;"><figure"#),
        "expected display width on captioned figure in HTML output:\n{html}"
    );
    assert!(
        html.contains(r##"<a href="#fig-graph">Figure"##),
        "expected labeled HTML figure cross-reference to resolve:\n{html}"
    );
    assert!(
        html.contains(r#"<img src="data:image/svg+xml;base64,"#),
        "expected captioned figure image to be embedded as a data URI:\n{html}"
    );
    assert!(
        html.contains(r#"alt style="display: block; width: 100%; height: 44px;">"#),
        "expected captioned figure image to fill styled figure:\n{html}"
    );
    assert!(
        !html.contains(r#"src="/.calepin/paper/figures/chunk-1.svg""#),
        "HTML compile should inline generated figure assets:\n{html}"
    );
    assert!(
        html.contains("height: 44px"),
        "expected display height in HTML output:\n{html}"
    );
    assert!(html.contains("HTML graph"));
}

#[test]
fn compile_accepts_canonical_figure_options() {
    if !has_command("typst") || !has_command("python3") || !has_python_module("matplotlib") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

#calepin.chunk(
  "python",
  label: "fig-line",
  echo: false,
  fig-device-format: "png",
  fig-device-width: 4,
  fig-device-height: 3,
  fig-device-aspect: 0.75,
  fig-device-dpi: 100,
  fig-display-width: 60%,
  fig-display-align: center,
  fig-display-responsive: true,
  fig-display-link: "https://example.com",
  fig-caption: [Canonical caption],
  fig-short-caption: "Short caption",
  fig-cap-location: top,
  fig-alt-text: "Line plot alt text",
  fig-subcaptions: ("A", "B"),
  fig-layout-columns: (1fr, 1fr),
  fig-layout-rows: auto,
)[` 
import matplotlib.pyplot as plt
plt.plot([1, 2, 3], [1, 4, 9])
`]
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");

    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(dir.path().join("paper.pdf").exists());
    assert!(dir
        .path()
        .join(".calepin/paper/figures/fig-line.png")
        .exists());
    let results_path = dir.path().join(".calepin/paper/results.json");
    let results: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(results_path).unwrap()).unwrap();
    assert_eq!(
        results["chunks"]["fig-line"]["items"][0]["data"]["image/png"]["path"],
        "/.calepin/paper/figures/fig-line.png"
    );

    if has_pdftotext() {
        let text = Command::new("pdftotext")
            .arg(dir.path().join("paper.pdf"))
            .arg("-")
            .output()
            .expect("failed to run pdftotext");
        assert!(text.status.success());
        let extracted = String::from_utf8(text.stdout).unwrap();
        assert!(extracted.contains("Canonical caption"), "{extracted}");
    }
}

#[test]
fn compile_captures_returned_matplotlib_figure_expression() {
    if !has_command("typst") || !has_command("python3") || !has_python_module("matplotlib") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("python", label: "fig-returned", echo: false, fig-caption: [Returned figure])[
```
from matplotlib.figure import Figure

fig = Figure()
ax = fig.subplots()
ax.plot([1, 2, 3], [1, 4, 9])
fig
```
]
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args([
            "compile",
            "paper.typ",
            "paper.html",
            "--format",
            "html",
            "--quiet",
        ])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");

    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(dir
        .path()
        .join(".calepin/paper/figures/fig-returned.svg")
        .exists());
    let results_path = dir.path().join(".calepin/paper/results.json");
    let results: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(results_path).unwrap()).unwrap();
    assert_eq!(
        results["chunks"]["fig-returned"]["items"][0]["data"]["image/svg+xml"]["path"],
        "/.calepin/paper/figures/fig-returned.svg"
    );
    let html = std::fs::read_to_string(dir.path().join("paper.html")).unwrap();
    assert!(
        html.contains(r#"<img src="data:image/svg+xml;base64,"#),
        "{html}"
    );
    assert!(!html.contains(r#"src="/.calepin/paper/figures/fig-returned.svg""#));
    assert!(
        !html.contains("&lt;Figure size"),
        "figure repr should not be rendered as text:\n{html}"
    );
}

#[test]
fn compile_resolves_captionless_fig_label_crossref() {
    if !has_command("typst")
        || !has_command("python3")
        || !has_pdftotext()
        || !has_python_module("matplotlib")
    {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

See @fig-cross.

#calepin.chunk("python", label: "fig-cross", echo: false)[```
import matplotlib.pyplot as plt
plt.plot([1, 2, 3], [1, 4, 9])
```
]
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");

    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let results_path = dir.path().join(".calepin/paper/results.json");
    let results: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(results_path).unwrap()).unwrap();
    assert_eq!(
        results["chunks"]["fig-cross"]["crossref-labels"][0]["name"],
        "fig-cross"
    );
    assert_eq!(
        results["chunks"]["fig-cross"]["crossref-labels"][0]["kind"],
        "fig"
    );

    let text = Command::new("pdftotext")
        .arg(dir.path().join("paper.pdf"))
        .arg("-")
        .output()
        .expect("failed to run pdftotext");
    assert!(text.status.success());
    let extracted = String::from_utf8(text.stdout).unwrap();
    assert!(extracted.contains("See Figure 1."), "{extracted}");
}

#[test]
fn compile_resolves_trailing_fence_fig_label_crossref() {
    if !has_command("typst")
        || !has_command("python3")
        || !has_pdftotext()
        || !has_python_module("matplotlib")
    {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

See @fig-trailing.

```python
import matplotlib.pyplot as plt
plt.plot([1, 2, 3], [1, 4, 9])
```<fig-trailing>
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");

    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let staged = std::fs::read_to_string(dir.path().join(".calepin/paper/source.typ")).unwrap();
    assert!(staged.contains(r#"#metadata((label: "fig-trailing")) <calepin-fence-label>"#));
    assert!(!staged.contains("```<fig-trailing>"));

    let results_path = dir.path().join(".calepin/paper/results.json");
    let results: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(results_path).unwrap()).unwrap();
    assert_eq!(
        results["chunks"]["fig-trailing"]["crossref-labels"][0]["name"],
        "fig-trailing"
    );
    assert_eq!(
        results["chunks"]["fig-trailing"]["crossref-labels"][0]["kind"],
        "fig"
    );

    let text = Command::new("pdftotext")
        .arg(dir.path().join("paper.pdf"))
        .arg("-")
        .output()
        .expect("failed to run pdftotext");
    assert!(text.status.success());
    let extracted = String::from_utf8(text.stdout).unwrap();
    assert!(extracted.contains("See Figure 1."), "{extracted}");
}

#[test]
fn compile_runs_bare_fences_with_document_raw_show_rule() {
    if !has_command("typst") || !has_command("python3") || !has_pdftotext() {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

#show raw.where(block: true): set text(size: .5em)

#calepin.setup(echo: false, results: "verbatim")

```python
print("BARE_RAW_SHOW_12345")
```

#calepin.chunk("python", echo: false, results: "verbatim")[
```python
print("EXPLICIT_RAW_SHOW_12345")
```
]
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");

    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let staged = std::fs::read_to_string(dir.path().join(".calepin/paper/source.typ")).unwrap();
    assert_eq!(
        staged
            .matches("#calepin_runtime.chunk_from_raw_plain(\"python\"")
            .count(),
        1,
        "only the bare fence should be rewritten:\n{staged}"
    );

    let text = Command::new("pdftotext")
        .arg(dir.path().join("paper.pdf"))
        .arg("-")
        .output()
        .expect("failed to run pdftotext");
    assert!(text.status.success());
    let extracted = String::from_utf8(text.stdout).unwrap();
    assert!(extracted.contains("BARE_RAW_SHOW_12345"), "{extracted}");
    assert!(extracted.contains("EXPLICIT_RAW_SHOW_12345"), "{extracted}");
    assert!(
        !extracted.contains("print(\"BARE_RAW_SHOW_12345\")"),
        "{extracted}"
    );
}

#[test]
fn compile_honors_hashpipe_options_on_bare_fences() {
    if !has_command("typst") || !has_command("python3") || !has_pdftotext() {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

#calepin.setup(echo: true, results: "verbatim")

```python
#| echo: false
print("HASHPIPE_ECHO_MARK")
```

```python
#| results: hide
print("HASHPIPE_HIDE_MARK")
```
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");
    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let text = Command::new("pdftotext")
        .arg(dir.path().join("paper.pdf"))
        .arg("-")
        .output()
        .expect("failed to run pdftotext");
    assert!(text.status.success());
    let extracted = String::from_utf8(text.stdout).unwrap();

    // `#| echo: false` hides the source even though the document default is
    // `echo: true`; the chunk still runs and shows its output.
    assert!(extracted.contains("HASHPIPE_ECHO_MARK"), "{extracted}");
    assert!(
        !extracted.contains("print(\"HASHPIPE_ECHO_MARK\")"),
        "`#| echo: false` should hide the source:\n{extracted}"
    );

    // `#| results: hide` keeps the echoed source (default echo) but suppresses
    // the output, so the marker appears exactly once (in the source listing).
    assert_eq!(
        extracted.matches("HASHPIPE_HIDE_MARK").count(),
        1,
        "`#| results: hide` should suppress the output:\n{extracted}"
    );
}

#[test]
fn compile_renders_inline_output_in_surrounding_text() {
    if !has_command("typst") || !has_command("python3") || !has_pdftotext() {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

#let py = calepin.inline.with("python")

The inline result is #py[`print("#strong[INLINEVALUE12345]")`] right here.
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");

    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let text = Command::new("pdftotext")
        .arg(dir.path().join("paper.pdf"))
        .arg("-")
        .output()
        .expect("failed to run pdftotext");

    assert!(text.status.success());
    let extracted = String::from_utf8(text.stdout).unwrap();
    assert!(
        extracted.contains("The inline result is #strong[INLINEVALUE12345] right here."),
        "{}",
        extracted
    );
    assert!(
        !extracted.contains("print(\"#strong[INLINEVALUE12345]\")"),
        "{}",
        extracted
    );
}

#[test]
fn compile_rejects_inline_labels() {
    if !has_command("typst") || !has_command("python3") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

#let py = calepin.inline.with("python")

#py(label: "not-allowed")[
`
print(42)
`
]
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");

    assert!(!output.status.success(), "compile unexpectedly succeeded");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unexpected argument"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn r_chunks_execute_live_without_cache_state() {
    if !has_command("typst") || !has_command("Rscript") {
        return;
    }

    let dir = typst_accessible_tempdir();
    let input = dir.path().join("paper.typ");
    std::fs::write(
        &input,
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("r", echo: false)[```
x <- 41
```]

#calepin.chunk("r", echo: false)[```
cat(x + 1)
```]
"##,
    )
    .unwrap();

    let first = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");
    assert!(
        first.status.success(),
        "first compile failed:\n{}",
        String::from_utf8_lossy(&first.stderr)
    );

    let source = std::fs::read_to_string(&input).unwrap();
    std::fs::write(&input, source.replace("cat(x + 1)", "cat(x + 2)")).unwrap();

    let second = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");
    assert!(
        second.status.success(),
        "second compile failed:\n{}",
        String::from_utf8_lossy(&second.stderr)
    );

    let results_path = dir.path().join(".calepin/paper/results.json");
    let results: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(results_path).unwrap()).unwrap();
    assert!(results["chunks"]["chunk-1"].get("cached").is_none());
    assert!(results["chunks"]["chunk-2"].get("cached").is_none());
    assert_eq!(results["chunks"]["chunk-2"]["items"][0]["text"], "43");
}

#[test]
fn python_chunks_execute_without_cache_state() {
    if !has_command("typst") || !has_command("python3") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("python", echo: false)[```
print(42)
```]
"##,
    )
    .unwrap();

    for run in ["first", "second"] {
        let output = Command::new(calepin_bin())
            .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
            .current_dir(dir.path())
            .output()
            .unwrap_or_else(|error| panic!("failed to run {run} compile: {error}"));
        assert!(
            output.status.success(),
            "{run} compile failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let results_path = dir.path().join(".calepin/paper/results.json");
    let results: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(results_path).unwrap()).unwrap();
    assert!(results["chunks"]["chunk-1"].get("cached").is_none());
}

#[test]
fn compile_injects_setup_params_into_python() {
    if !has_command("typst") || !has_command("python3") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

#calepin.setup(params: (region: "NY", min_count: 25))

#calepin.chunk("python", echo: false)[```
print(params["region"], params["min_count"])
```]
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");
    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let results: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join(".calepin/paper/results.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(results["chunks"]["chunk-1"]["items"][0]["text"], "NY 25");
    // params.json is written as the universal transport / reproducibility record.
    assert!(dir.path().join(".calepin/paper/params.json").exists());
}

#[test]
fn compile_param_override_beats_setup_params() {
    if !has_command("typst") || !has_command("python3") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

#calepin.setup(params: (region: "NY"))

#calepin.chunk("python", echo: false)[```
print(params["region"])
```]
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args([
            "compile",
            "paper.typ",
            "paper.pdf",
            "-P",
            "region=CA",
            "--quiet",
        ])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");
    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let results: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join(".calepin/paper/results.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(results["chunks"]["chunk-1"]["items"][0]["text"], "CA");
}

#[test]
fn compile_rejects_unsupported_param_type() {
    if !has_command("typst") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

#calepin.setup(params: (bad: red))

Body text.
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");

    assert!(!output.status.success(), "compile unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unsupported parameter") && stderr.contains("bad"),
        "{stderr}"
    );
}

#[test]
fn compile_relocates_hidden_chunk_output_by_plain_label() {
    if !has_command("typst") || !has_command("python3") || !has_pdftotext() {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("python", label: "summary", echo: false, results: "hide")[```
print("RELOCATED_OUTPUT_42")
```]

Body text in between.

#calepin.results("summary")
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");
    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let text = Command::new("pdftotext")
        .arg(dir.path().join("paper.pdf"))
        .arg("-")
        .output()
        .unwrap();
    let extracted = String::from_utf8(text.stdout).unwrap();
    let count = extracted.matches("RELOCATED_OUTPUT_42").count();
    assert_eq!(
        count, 1,
        "expected hidden output to appear once at the relocation: {extracted}"
    );
}
