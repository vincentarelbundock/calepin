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

#[test]
fn academic_website_scaffold_builds_blog_with_local_thumbnail() {
    if !has_command("typst") || !has_command("Rscript") {
        return;
    }

    let dir = typst_accessible_tempdir();
    let create = Command::new(calepin_bin())
        .args(["new", "website", "site", "--theme", "academic"])
        .current_dir(dir.path())
        .output()
        .expect("failed to create academic website scaffold");
    assert!(
        create.status.success(),
        "website scaffold creation failed:\n{}",
        String::from_utf8_lossy(&create.stderr)
    );

    let site = dir.path().join("site");
    assert!(
        site.join("assets/flowers_01.jpg").is_file(),
        "academic scaffold should include its local listing thumbnail"
    );

    let build = Command::new(calepin_bin())
        .args(["compile", "site", "--format", "html", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to build academic website scaffold");
    assert!(
        build.status.success(),
        "website build failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    assert!(
        site.join("blog.html").is_file(),
        "website build should produce blog.html"
    );
}

fn typst_accessible_tempdir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("calepin-typst-test-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap()
}

#[test]
fn script_format_extracts_languages_without_executing_chunks() {
    if !has_command("typst") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r#"#import ".calepin/calepin.typ" as calepin

#calepin.setup(fenced-chunks: true)

```python
raise RuntimeError("must not execute")
```

```r
#| eval: false
stop("must not execute")
```

```julia
error("must not execute")
```

```rust
fn main() {}
```

```mystery
BEGIN UNKNOWN END
```
"#,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "--format", "script", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to extract scripts");

    assert!(
        output.status.success(),
        "script extraction failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let python = std::fs::read_to_string(dir.path().join("paper.py")).unwrap();
    let r = std::fs::read_to_string(dir.path().join("paper.R")).unwrap();
    let julia = std::fs::read_to_string(dir.path().join("paper.jl")).unwrap();
    let rust = std::fs::read_to_string(dir.path().join("paper.rs")).unwrap();
    let unknown = std::fs::read_to_string(dir.path().join("paper.txt")).unwrap();
    assert!(python.contains("raise RuntimeError"));
    assert!(!python.contains("stop("));
    assert!(r.contains("stop(\"must not execute\")"));
    assert!(!r.contains("RuntimeError"));
    assert!(julia.contains("error(\"must not execute\")"));
    assert_eq!(julia.matches("error(\"must not execute\")").count(), 1);
    assert!(rust.starts_with("// ---- chunk-"));
    assert_eq!(rust.matches("fn main() {}").count(), 1);
    assert_eq!(unknown, "BEGIN UNKNOWN END\n");
    assert!(!dir.path().join(".calepin/paper/results.json").exists());
}

#[test]
fn script_format_routes_and_excludes_chunks() {
    if !has_command("typst") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r#"#import ".calepin/calepin.typ" as calepin

#calepin.setup(fenced-chunks: true, script: false)

```python
#| label: first
#| script: scripts/first.py
print("first")
```

```python
#| label: omitted
print("omitted")
```

```python
#| label: first-continuation
#| script: scripts/first.py
print("first continuation")
```

```python
#| label: second
#| script: scripts/second.py
print("second")
```

```python
#| label: default
#| script: true
print("default")
```

#calepin.chunk(
  "python",
  label: "function-call",
  script: "scripts/function.py",
)[```python
print("function")
```]
"#,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "--format", "script", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to extract routed scripts");

    assert!(
        output.status.success(),
        "script extraction failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let first = std::fs::read_to_string(dir.path().join("scripts/first.py")).unwrap();
    let second = std::fs::read_to_string(dir.path().join("scripts/second.py")).unwrap();
    let function = std::fs::read_to_string(dir.path().join("scripts/function.py")).unwrap();
    let default = std::fs::read_to_string(dir.path().join("paper.py")).unwrap();
    assert!(first.contains("print(\"first\")"));
    assert!(first.contains("print(\"first continuation\")"));
    assert!(
        first.find("print(\"first\")") < first.find("print(\"first continuation\")"),
        "chunks targeting one script should retain document order"
    );
    assert!(second.contains("print(\"second\")"));
    assert!(function.contains("print(\"function\")"));
    assert!(default.contains("print(\"default\")"));
    assert!(!first.contains("omitted"));
    assert!(!second.contains("omitted"));
    assert!(!default.contains("omitted"));
}

#[test]
fn compile_config_asset_dir_writes_no_dot_calepin_directory() {
    if !has_command("typst") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("tmp.typ"),
        r#"#import "/_calepin/calepin.typ" as calepin

= Runtime dir test
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("tmp.toml"),
        r#"asset-dir = "_calepin"
"#,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "tmp.typ", "--config", "tmp.toml", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");

    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.path().join("_calepin/calepin.typ").exists());
    assert!(dir.path().join("_calepin/active.typ").exists());
    assert!(dir.path().join("_calepin/tmp/calepin.typ").exists());
    assert!(dir.path().join("_calepin/tmp/runtime-config.typ").exists());
    assert!(!dir.path().join(".calepin").exists());

    // Entry files are staged beside the document while it renders and removed
    // once the render succeeds.
    let leftovers = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.starts_with(".calepin-entry."))
        .collect::<Vec<_>>();
    assert!(
        leftovers.is_empty(),
        "left behind entry files: {leftovers:?}"
    );

    let preview = Command::new("typst")
        .args(["compile", "tmp.typ", "preview.pdf", "--root", "."])
        .current_dir(dir.path())
        .output()
        .expect("failed to compile with the custom generated runtime");
    assert!(
        preview.status.success(),
        "plain Typst compile failed:\n{}",
        String::from_utf8_lossy(&preview.stderr)
    );
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
    assert!(dir.path().join(".calepin/paper/calepin.typ").exists());
    assert!(dir
        .path()
        .join(".calepin/paper/runtime-config.typ")
        .exists());

    let active = std::fs::read_to_string(dir.path().join(".calepin/active.typ")).unwrap();
    assert!(
        active.contains(r#"#import "paper/runtime-config.typ": config"#),
        "{active}"
    );

    let results_path = dir.path().join(".calepin/paper/results.json");
    let results: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(results_path).unwrap()).unwrap();
    assert_eq!(results["schema"], 2);
    assert_eq!(results["chunks"]["chunk-1"]["engine"], "python");
    assert!(results["chunks"]["chunk-1"].get("cached").is_none());
    assert_eq!(results["chunks"]["chunk-1"]["items"][0]["type"], "stream");
    assert_eq!(results["chunks"]["chunk-1"]["items"][0]["text"], "42");

    let preview = Command::new("typst")
        .args(["compile", "paper.typ", "preview.pdf", "--root", "."])
        .current_dir(dir.path())
        .output()
        .expect("failed to compile the original source with Typst");
    assert!(
        preview.status.success(),
        "plain Typst compile failed:\n{}",
        String::from_utf8_lossy(&preview.stderr)
    );
}

#[test]
fn preprocess_carries_setup_figure_defaults_into_chunk_results() {
    if !has_command("typst") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

#calepin.setup(
  fig-height: 8cm,
  fig-link: "https://example.com/default",
  fig-caption: [Default caption],
  fig-cap-location: top,
  fig-alt-text: "Default alt",
  fig-subcaptions: ("Left", "Right"),
  fig-layout-columns: 2,
  fig-layout-rows: 1,
  kind: "figure",
  fenced-chunks: "julia-1.2",
)

#calepin.chunk("python", label: "inherits", eval: false)[`print("INHERITED")`]
#calepin.chunk(
  "python",
  label: "clears",
  eval: false,
  fig-link: none,
  fig-caption: none,
  fig-alt-text: none,
  fig-subcaptions: none,
)[`print("CLEARED")`]

#calepin.chunk(label: "versioned", eval: false)[```julia-1.2
.2
x = 41
```]

```julia-1.2
#| label: bare-versioned
#| eval: false
.2
x = 42
```

Setup defaults integration test.
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
    let inherited = &results["chunks"]["inherits"];
    assert_eq!(inherited["source"], "print(\"INHERITED\")");
    assert_ne!(inherited["options"]["fig-height"], serde_json::Value::Null);
    assert_eq!(
        inherited["options"]["fig-link"],
        "https://example.com/default"
    );
    assert_eq!(inherited["options"]["fig-caption"], "Default caption");
    assert_eq!(inherited["options"]["fig-cap-location"], "top");
    assert_eq!(inherited["options"]["fig-alt-text"], "Default alt");
    assert_eq!(
        inherited["options"]["fig-subcaptions"],
        serde_json::json!(["Left", "Right"])
    );
    assert_eq!(inherited["options"]["fig-layout-columns"], 2);
    assert_eq!(inherited["options"]["fig-layout-rows"], 1);
    assert_eq!(inherited["options"]["kind"], "figure");

    let cleared = &results["chunks"]["clears"]["options"];
    assert_eq!(cleared["fig-link"], serde_json::Value::Null);
    assert_eq!(cleared["fig-caption"], serde_json::Value::Null);
    assert_eq!(cleared["fig-alt-text"], serde_json::Value::Null);
    assert_eq!(cleared["fig-subcaptions"], serde_json::Value::Null);
    assert_eq!(cleared["fig-layout-columns"], 2);
    assert_eq!(cleared["fig-layout-rows"], 1);

    let versioned = &results["chunks"]["versioned"];
    assert_eq!(versioned["engine"], "julia-1.2");
    assert_eq!(versioned["source"], ".2\nx = 41\n");

    let bare_versioned = &results["chunks"]["bare-versioned"];
    assert_eq!(bare_versioned["engine"], "julia-1.2");
    assert_eq!(bare_versioned["source"], ".2\nx = 42\n");
}

#[test]
fn compile_bootstraps_notebook_facade_for_plain_typst_preview() {
    if !has_command("typst") || !has_command("python3") || !has_pdftotext() {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r#"#import "/.calepin/paper/calepin.typ" as calepin
#show: calepin.document

#calepin.setup(echo: false, fenced-chunks: true)

```python
print("bound preview output")
```

#calepin.chunk("python", echo: false)[```python
print("explicit bound output")
```]
"#,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");
    assert!(
        output.status.success(),
        "Calepin compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let preview = Command::new("typst")
        .args(["compile", "paper.typ", "preview.pdf", "--root", "."])
        .current_dir(dir.path())
        .output()
        .expect("failed to compile the original source with Typst");
    assert!(
        preview.status.success(),
        "plain Typst compile failed:\n{}",
        String::from_utf8_lossy(&preview.stderr)
    );

    let text = Command::new("pdftotext")
        .args(["preview.pdf", "-"])
        .current_dir(dir.path())
        .output()
        .expect("failed to extract preview text");
    assert!(text.status.success());
    let text = String::from_utf8_lossy(&text.stdout);
    assert!(text.contains("bound preview output"), "{text}");
    assert!(text.contains("explicit bound output"), "{text}");
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
from pathlib import Path
counter = Path("cache-runs.txt")
_ = counter.write_text(str(int(counter.read_text()) + 1 if counter.exists() else 1))
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
    assert_eq!(
        std::fs::read_to_string(dir.path().join("cache-runs.txt")).unwrap(),
        "1"
    );
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
        !html.contains("cdn.jsdelivr.net/npm/@picocss/pico@2/css/pico.min.css"),
        "{html}"
    );
    assert!(html.contains("calepin-document-main"), "{html}");
    assert!(html.contains("Theme is a compile-time concern."), "{html}");
}

#[test]
fn compile_html_assigns_safe_ids_to_all_heading_forms() {
    if !has_command("typst") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

#calepin.setup(eval: false)

= #html.elem("i", "", attrs: (class: "icon")) Labeled Research <research>

= #html.elem("i", "", attrs: (class: "icon")) Slugged Research

#heading[Dynamic label] #label("x\" onmouseover=\"alert(1)")

====== Deep heading <deep>
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
    assert!(html.contains(r#"<h2 id="research">"#), "{html}");
    assert!(html.contains(r#"<h2 id="slugged-research">"#), "{html}");
    assert!(
        html.contains(r#"<h2 id="x&quot; onmouseover=&quot;alert(1)">Dynamic label</h2>"#),
        "{html}"
    );
    assert!(
        !html.contains(r#" id="x" onmouseover="alert(1)""#),
        "{html}"
    );
    assert!(
        html.contains(r#"<div role="heading" aria-level="7" id="deep">Deep heading</div>"#),
        "{html}"
    );
    assert!(!html.contains("calepin-heading-anchor"), "{html}");
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
        html.contains(
            r#"<div class="calepin-figure-width" style="width: 37%; max-width: 100%; margin-inline: auto;"><figure"#
        ),
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
fn compile_injects_document_store_into_python() {
    if !has_command("typst") || !has_command("python3") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

#calepin.store.set("region", "NY")
#calepin.store.set("min_count", 25)

#calepin.chunk("python", echo: false, store-get: ("region", "min_count"))[```
print(region, min_count)
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
    assert_eq!(results["store"]["region"], "NY");
    assert!(!dir.path().join(".calepin/paper/vars.json").exists());
}

#[test]
fn typst_store_set_is_bound_without_rewriting_string_literals() {
    if !has_command("typst") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import "/.calepin/calepin.typ" as calepin

#let literal = "#calepin.store.set(\"wrong\", 1)"
#calepin.store.set("region", "NY")
#assert(calepin.store.get("region", default: "NY") == "NY")
#literal
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
    assert_eq!(results["store"], serde_json::json!({"region": "NY"}));
}

#[test]
fn compile_store_override_beats_document_initializer() {
    if !has_command("typst") || !has_command("python3") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

#calepin.store.set("region", "NY")

#calepin.chunk("python", echo: false, store-get: "region")[```
print(region)
```]
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args([
            "compile",
            "paper.typ",
            "paper.pdf",
            "--set",
            "store.region=CA",
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
fn computed_store_values_expand_later_chunks() {
    if !has_command("typst") || !has_command("python3") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

#calepin.chunk("python", store-set: "labels", results: "hide")[
```python
from pathlib import Path
counter = Path("writer-runs.txt")
_ = counter.write_text(str(int(counter.read_text()) + 1 if counter.exists() else 1))
labels = ["A", "B"]
```
]

#for label in calepin.store.get("labels", default: ()) {
  calepin.chunk("python", raw("print(" + json.encode(label) + ")"), echo: false)
}
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

    let cached = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "paper.pdf", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run cached calepin compile");
    assert!(
        cached.status.success(),
        "cached compile failed:\n{}",
        String::from_utf8_lossy(&cached.stderr)
    );

    let results: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join(".calepin/paper/results.json")).unwrap(),
    )
    .unwrap();
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join(".calepin/paper/expansion.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(results["store"]["labels"], serde_json::json!(["A", "B"]));
    assert_eq!(results["chunks"].as_object().unwrap().len(), 3);
    assert_eq!(results["chunks"]["chunk-2"]["items"][0]["text"], "A");
    assert_eq!(results["chunks"]["chunk-3"]["items"][0]["text"], "B");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("writer-runs.txt")).unwrap(),
        "1"
    );
    assert_eq!(results["generation"], manifest["generation"]);
}

#[test]
fn computed_store_values_refresh_minijinja_query_theme() {
    if !has_command("typst") || !has_command("python3") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::create_dir_all(dir.path().join("mytheme/layouts")).unwrap();
    std::fs::write(
        dir.path().join("mytheme/theme.toml"),
        "extends = \"typst\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("mytheme/layouts/pdf.typ"),
        r#"{% if store.enable_followup | default(false) %}
#let theme-enables-followup = true
{% else %}
#let theme-enables-followup = false
{% endif %}
{{ doc.body }}
"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("paper.toml"), "theme = \"mytheme/\"\n").unwrap();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import "/.calepin/calepin.typ" as calepin

#calepin.chunk("python", store-set: "enable_followup", results: "hide")[
```python
enable_followup = True
```
]

#if theme-enables-followup {
  calepin.chunk(
    "python",
    raw("print('THEME_STORE_OK')", block: true),
    echo: false,
  )
}
"##,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args([
            "compile",
            "paper.typ",
            "paper.pdf",
            "--config",
            "paper.toml",
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
    assert_eq!(results["chunks"].as_object().unwrap().len(), 2);
    assert_eq!(
        results["chunks"]["chunk-2"]["items"][0]["text"],
        "THEME_STORE_OK"
    );
}

#[test]
fn r_store_preserves_whole_valued_doubles() {
    if !has_command("typst") || !has_command("Rscript") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import "/.calepin/calepin.typ" as calepin

#calepin.chunk("r", store-set: "answer", results: "hide")[
```r
answer <- 2.0
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

    let results: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join(".calepin/paper/results.json")).unwrap(),
    )
    .unwrap();
    assert!(
        results["store"]["answer"].as_f64().is_some()
            && results["store"]["answer"].as_i64().is_none()
    );
}

#[test]
fn compile_rejects_removed_setup_vars() {
    if !has_command("typst") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r##"#import ".calepin/calepin.typ"

#calepin.setup(vars: (bad: red))

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
        stderr.contains("unexpected argument") && stderr.contains("vars"),
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

#[test]
fn adjacent_display_fences_keep_distinct_auto_chunk_results() {
    if !has_command("typst") || !has_pdftotext() {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::write(
        dir.path().join("paper.typ"),
        r#"#import ".calepin/calepin.typ" as calepin

#calepin.setup(echo: true, eval: true)

```toml
#| eval: false
PREVIOUS_TOML_BLOCK = true
```

```sh
#| eval: false
echo CURRENT_SHELL_BLOCK
```
"#,
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
    let extracted = String::from_utf8(text.stdout).unwrap();
    assert!(
        extracted.contains("PREVIOUS_TOML_BLOCK = true"),
        "{extracted}"
    );
    assert!(
        extracted.contains("echo CURRENT_SHELL_BLOCK"),
        "{extracted}"
    );
    assert_eq!(extracted.matches("PREVIOUS_TOML_BLOCK = true").count(), 1);
}

#[test]
fn document_body_can_call_theme_exported_helpers() {
    if !has_command("typst") || !has_pdftotext() {
        return;
    }

    // A theme that hands the author a vocabulary: a `#let` defined in the theme
    // preamble. The body inlines at the theme's `{{ doc.body }}` seam and shares
    // its file scope, so it can call the helper. The body is evaluated in both
    // the query and render passes, so this only works because both passes use
    // the same themed wrapper.
    let dir = typst_accessible_tempdir();
    std::fs::create_dir_all(dir.path().join("mytheme/layouts")).unwrap();
    std::fs::write(
        dir.path().join("mytheme/theme.toml"),
        "extends = \"typst\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("mytheme/layouts/pdf.typ"),
        "#let theme-badge(body) = [BADGE: #body]\n\n{{ doc.body }}\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("paper.toml"), "theme = \"mytheme/\"\n").unwrap();
    std::fs::write(
        dir.path().join("paper.typ"),
        r#"#import "/.calepin/calepin.typ" as calepin

#theme-badge[VOCAB_OK]
"#,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "--config", "paper.toml", "--quiet"])
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
    assert!(
        extracted.contains("BADGE: VOCAB_OK"),
        "body should be able to call a theme-exported helper: {extracted}"
    );
}

#[test]
fn compile_resolves_relative_paths_from_the_document_directory() {
    if !has_command("typst") || !has_pdftotext() {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::create_dir_all(dir.path().join("lib")).unwrap();
    std::fs::write(
        dir.path().join("helpers.typ"),
        "#let shout(body) = upper(body)\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("lib/deep.typ"), "#let depth = [NESTED]\n").unwrap();
    std::fs::write(dir.path().join("numbers.csv"), "value\nCSVCELL\n").unwrap();
    std::fs::write(
        dir.path().join("paper.typ"),
        r#"#import "helpers.typ": shout
#import "lib/deep.typ": depth

#shout[sibling] #depth #csv("numbers.csv").at(1).at(0)
"#,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", "paper.typ", "--quiet"])
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
    for expected in ["SIBLING", "NESTED", "CSVCELL"] {
        assert!(
            extracted.contains(expected),
            "relative path should resolve next to the document, missing {expected}: {extracted}"
        );
    }
}

#[test]
fn website_build_resolves_relative_paths_from_each_page_directory() {
    if !has_command("typst") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::create_dir_all(dir.path().join("sub")).unwrap();
    std::fs::write(
        dir.path().join("calepin.toml"),
        "title = \"Relative paths\"\n\n[pages]\nexclude = [\"shared.typ\", \"sub/local.typ\"]\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("shared.typ"),
        "#let shared = [SHAREDHELPER]\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("sub/local.typ"),
        "#let local = [LOCALHELPER]\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("index.typ"), "= Home\n").unwrap();
    std::fs::write(
        dir.path().join("sub/page.typ"),
        r#"#import "../shared.typ": shared
#import "local.typ": local

= Page

#shared #local
"#,
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args(["compile", ".", "_site", "--quiet"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");
    assert!(
        output.status.success(),
        "website build failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let page = std::fs::read_to_string(dir.path().join("_site/sub/page.html")).unwrap();
    assert!(
        page.contains("SHAREDHELPER"),
        "parent-relative import should resolve: {page}"
    );
    assert!(
        page.contains("LOCALHELPER"),
        "sibling import should resolve: {page}"
    );

    let entry_files = collect_entry_files_recursively(dir.path());
    assert!(
        entry_files.is_empty(),
        "website build should clean up its entry files: {entry_files:?}"
    );
}

fn collect_entry_files_recursively(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.filter_map(|entry| entry.ok()) {
        let path = entry.path();
        if path.is_dir() {
            out.extend(collect_entry_files_recursively(&path));
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(".calepin-entry."))
        {
            out.push(path);
        }
    }
    out
}
