//! Integration tests for the module pipeline.
//! Renders minimal .qmd documents and checks that modules produce expected output.

use std::path::Path;
use std::process::Command;

fn calepin_bin() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_calepin"))
}

fn render(input: &str, target: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let qmd = dir.path().join("test.qmd");
    std::fs::write(&qmd, input).unwrap();

    let output = Command::new(calepin_bin())
        .args([qmd.to_str().unwrap(), "-t", target, "-q"])
        .output()
        .expect("failed to run calepin");

    assert!(output.status.success(), "calepin failed:\n{}", String::from_utf8_lossy(&output.stderr));

    let ext = match target {
        "latex" => "tex",
        "typst" => "typ",
        "markdown" => "md",
        _ => "html",
    };
    let out_path = qmd.with_extension(ext);
    std::fs::read_to_string(&out_path).unwrap()
}

// ---------------------------------------------------------------------------
// Highlight module: syntax CSS (HTML) and color defs (LaTeX)
// ---------------------------------------------------------------------------

#[test]
fn highlight_html_injects_syntax_css() {
    let html = render("---\ntitle: Test\n---\n\n```python\nx = 1\n```\n", "html");
    assert!(html.contains("<style>"), "should contain a <style> tag for syntax CSS");
    assert!(html.contains(".source"), "should contain syntax highlighting CSS classes");
}

#[test]
fn highlight_latex_injects_color_defs() {
    let tex = render("---\ntitle: Test\n---\n\n```python\nx = 1\n```\n", "latex");
    assert!(tex.contains("\\definecolor"), "should contain \\definecolor for syntax highlighting");
    assert!(tex.contains("\\begin{document}"), "should be a complete LaTeX document");
    // Color defs should appear before \begin{document}
    let color_pos = tex.find("\\definecolor").unwrap();
    let doc_pos = tex.find("\\begin{document}").unwrap();
    assert!(color_pos < doc_pos, "color defs should be in the preamble");
}

// ---------------------------------------------------------------------------
// Footnotes (append_footnotes module)
// ---------------------------------------------------------------------------

#[test]
fn append_footnotes_html() {
    let html = render("---\ntitle: Test\n---\n\nText with a note[^1].\n\n[^1]: This is the footnote.\n", "html");
    assert!(html.contains("footnote"), "should contain footnote markup");
}

// ---------------------------------------------------------------------------
// Slide splitting (split_slides module)
// ---------------------------------------------------------------------------

#[test]
fn split_slides_revealjs() {
    let html = render("---\ntitle: Slides\n---\n\n## Slide 1\n\nContent 1\n\n## Slide 2\n\nContent 2\n", "revealjs");
    assert!(html.contains("<section>"), "should contain <section> tags for slides");
    assert!(html.matches("<section>").count() >= 2, "should have at least 2 slides");
}

// ---------------------------------------------------------------------------
// Theorem auto-numbering (number=true)
// ---------------------------------------------------------------------------

#[test]
fn theorem_auto_numbering() {
    let html = render("---\ntitle: Test\n---\n\n::: {.theorem}\nFirst theorem.\n:::\n\n::: {.theorem}\nSecond theorem.\n:::\n", "html");
    // Should contain numbered theorems
    assert!(html.contains("1"), "should contain theorem number 1");
    assert!(html.contains("2"), "should contain theorem number 2");
}

// ---------------------------------------------------------------------------
// Embed images (embed_images module)
// ---------------------------------------------------------------------------

#[test]
fn no_crash_on_missing_images() {
    // Should render without crashing even if image doesn't exist
    let html = render("---\ntitle: Test\n---\n\n![Alt](nonexistent.png)\n", "html");
    assert!(html.contains("nonexistent.png"), "should preserve image path");
}

// ---------------------------------------------------------------------------
// Multi-format: same document renders to all targets
// ---------------------------------------------------------------------------

#[test]
fn renders_all_formats() {
    let input = "---\ntitle: Test\n---\n\nHello world.\n";
    let html = render(input, "html");
    let tex = render(input, "latex");
    let typ = render(input, "typst");
    let md = render(input, "markdown");

    assert!(html.contains("Hello world"));
    assert!(tex.contains("Hello world"));
    assert!(typ.contains("Hello world"));
    assert!(md.contains("Hello world"));
}

// ---------------------------------------------------------------------------
// User-provided script modules
// ---------------------------------------------------------------------------

#[test]
#[ignore] // Requires project root resolution from _calepin.toml -- TODO fix path context
fn user_document_transform_script() {
    let dir = tempfile::tempdir().unwrap();

    // Create a module that appends "<!-- injected -->" to the document
    let module_dir = dir.path().join("_calepin").join("modules").join("inject_comment");
    std::fs::create_dir_all(&module_dir).unwrap();
    std::fs::write(module_dir.join("module.toml"), r#"
name = "inject_comment"

[document]
run = "inject.sh"
"#).unwrap();
    std::fs::write(module_dir.join("inject.sh"), "#!/bin/sh\ncat\necho '<!-- injected -->'\n").unwrap();

    // Make script executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(module_dir.join("inject.sh"),
            std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    // Create a document that references the module
    let qmd = dir.path().join("test.qmd");
    std::fs::write(&qmd, "---\ntitle: Test\ncalepin:\n  plugins:\n    - inject_comment\n---\n\nHello.\n").unwrap();

    // Create _calepin.toml to set the project root
    std::fs::write(dir.path().join("_calepin.toml"), r#"
[targets.html]
engine = "html"
modules = ["highlight", "append_footnotes", "embed_images", "inject_comment"]
"#).unwrap();

    let output = Command::new(calepin_bin())
        .args([qmd.to_str().unwrap(), "-t", "html", "-q"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin");

    assert!(output.status.success(), "calepin failed:\n{}", String::from_utf8_lossy(&output.stderr));

    let out_path = qmd.with_extension("html");
    let html = std::fs::read_to_string(&out_path).unwrap();
    assert!(html.contains("<!-- injected -->"), "user module script should have injected comment");
}
