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

#calepin.chunk("python", label: "answer", echo: false)[```
x = 41
print(x + 1)
```]
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args([
            "compile",
            "paper.typ",
            "paper.pdf",
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
    assert!(dir.path().join(".calepin/calepin.typ").exists());

    let results_path = dir.path().join(".calepin/paper/results.json");
    let results: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(results_path).unwrap()).unwrap();
    assert_eq!(results["schema"], 1);
    assert_eq!(results["chunks"]["answer"]["engine"], "python");
    assert!(results["chunks"]["answer"].get("cached").is_none());
    assert_eq!(results["chunks"]["answer"]["items"][0]["type"], "stream");
    assert_eq!(results["chunks"]["answer"]["items"][0]["text"], "42");
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

#calepin.chunk("python", label: "answer", echo: false, results: "asis")[```
print("#strong[42]")
```]
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args([
            "compile",
            "paper.typ",
            "paper.pdf",
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
    let pdf = dir.path().join("paper.pdf");
    assert!(pdf.exists());
    assert!(std::fs::metadata(pdf).unwrap().len() > 0);
}

#[test]
fn compile_html_uses_template_theme() {
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
            "--template",
            "pico",
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
    assert!(
        html.contains("<main class=\"container\">"),
        "{html}"
    );
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

#calepin.chunk(
  "dot",
  label: "fig-graph",
  echo: false,
  fig-display-width: "37%",
  fig-display-height: "44px",
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
        html.contains(r#"<figure style="width: 37%; max-width: 100%; margin-inline: auto;">"#),
        "expected display width on captioned figure in HTML output:\n{html}"
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
        !html.contains(r#"src="/.calepin/paper/figures/fig-graph.svg""#),
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
  fig-caption-position: top,
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
        .args([
            "compile",
            "paper.typ",
            "paper.pdf",
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
        .args([
            "compile",
            "paper.typ",
            "paper.pdf",
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
        .args([
            "compile",
            "paper.typ",
            "paper.pdf",
            "--quiet",
        ])
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

#calepin.chunk("r", label: "setup", echo: false)[```
x <- 41
```]

#calepin.chunk("r", label: "answer", echo: false)[```
cat(x + 1)
```]
"##,
    )
    .unwrap();

    let first = Command::new(calepin_bin())
        .args([
            "compile",
            "paper.typ",
            "paper.pdf",
            "--quiet",
        ])
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
        .args([
            "compile",
            "paper.typ",
            "paper.pdf",
            "--quiet",
        ])
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
    assert!(results["chunks"]["setup"].get("cached").is_none());
    assert!(results["chunks"]["answer"].get("cached").is_none());
    assert_eq!(results["chunks"]["answer"]["items"][0]["text"], "43");
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

#calepin.chunk("python", label: "answer", echo: false)[```
print(42)
```]
"##,
    )
    .unwrap();

    for run in ["first", "second"] {
        let output = Command::new(calepin_bin())
            .args([
                "compile",
                "paper.typ",
                "paper.pdf",
                "--quiet",
            ])
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
    assert!(results["chunks"]["answer"].get("cached").is_none());
}
