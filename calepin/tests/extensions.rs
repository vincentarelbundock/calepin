//! Integration tests for the extension system.
//! Verifies that user-installed extensions provide templates, CSS assets,
//! and vars to the render pipeline.

use std::fs;
use std::path::Path;
use std::process::Command;

fn calepin_bin() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_calepin"))
}

/// Set up a temp directory with a custom extension named `myext` that inherits
/// from `html`. Returns (tempdir, qmd_path).
fn setup_extension_project(qmd_content: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();

    // test_calepin/config.toml -- set the default target to the extension name
    let sidecar = dir.path().join("test_calepin");
    fs::create_dir_all(&sidecar).unwrap();
    fs::write(sidecar.join("config.toml"), "target = \"myext\"\n").unwrap();

    // test_calepin/extensions/myext/extension.toml
    let ext_dir = sidecar.join("extensions").join("myext");
    fs::create_dir_all(&ext_dir).unwrap();
    fs::write(ext_dir.join("extension.toml"), r#"
name = "myext"
description = "Test extension"
inherits = "html"

[assets]
css = ["custom.css"]

[vars]
accent = "coral"
site_name = "Test Site"
"#).unwrap();

    // test_calepin/extensions/myext/assets/custom.css
    let assets_dir = ext_dir.join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    fs::write(assets_dir.join("custom.css"), r#"
.myext-banner { background: coral; padding: 1rem; }
.myext-footer { border-top: 2px solid coral; }
"#).unwrap();

    // test_calepin/extensions/myext/templates/html/banner.html
    let tpl_dir = ext_dir.join("templates").join("html");
    fs::create_dir_all(&tpl_dir).unwrap();
    fs::write(tpl_dir.join("banner.html"),
        "<div class=\"myext-banner\">{{ clp.content }}</div>\n"
    ).unwrap();

    // Write the .qmd file
    let qmd = dir.path().join("test.qmd");
    fs::write(&qmd, qmd_content).unwrap();

    (dir, qmd)
}

fn render_in_project(dir: &Path, qmd: &Path, target: &str) -> String {
    let output = Command::new(calepin_bin())
        .args([qmd.to_str().unwrap(), "-t", target, "-q"])
        .current_dir(dir)
        .output()
        .expect("failed to run calepin");

    assert!(
        output.status.success(),
        "calepin failed (target={}):\nstderr: {}\nstdout: {}",
        target,
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    let out_path = qmd.with_extension("html");
    fs::read_to_string(&out_path).unwrap()
}

// ---------------------------------------------------------------------------
// Extension CSS injection
// ---------------------------------------------------------------------------

#[test]
fn extension_css_injected_in_style_tag() {
    let (dir, qmd) = setup_extension_project(
        "---\ntitle = \"CSS Test\"\n---\n\nHello from extension.\n",
    );
    let html = render_in_project(dir.path(), &qmd, "myext");

    assert!(
        html.contains(".myext-banner"),
        "extension CSS class .myext-banner should appear in output"
    );
    assert!(
        html.contains(".myext-footer"),
        "extension CSS class .myext-footer should appear in output"
    );
    // CSS should be inside a <style> tag
    assert!(
        html.contains("<style>") || html.contains("<style "),
        "extension CSS should be injected via a <style> tag"
    );
}

// ---------------------------------------------------------------------------
// Extension vars
// ---------------------------------------------------------------------------

#[test]
fn extension_vars_available_in_body() {
    let (dir, qmd) = setup_extension_project(
        "---\ntitle = \"Vars Test\"\n---\n\nAccent is {{ cfg.myext.accent }} and site is {{ cfg.myext.site_name }}.\n",
    );
    let html = render_in_project(dir.path(), &qmd, "myext");

    assert!(
        html.contains("coral"),
        "extension var 'accent' should be rendered in output: {}",
        html
    );
    assert!(
        html.contains("Test Site"),
        "extension var 'site_name' should be rendered in output: {}",
        html
    );
}

// ---------------------------------------------------------------------------
// Extension custom template
// ---------------------------------------------------------------------------

#[test]
fn extension_partial_used_for_div() {
    let (dir, qmd) = setup_extension_project(
        "---\ntitle = \"Partial Test\"\n---\n\n::: {.banner}\nWelcome!\n:::\n",
    );
    let html = render_in_project(dir.path(), &qmd, "myext");

    assert!(
        html.contains("myext-banner"),
        "extension template should produce .myext-banner class in output: {}",
        html
    );
    assert!(
        html.contains("Welcome!"),
        "div content should be preserved in output"
    );
}

// ---------------------------------------------------------------------------
// Extension inherits target resolution
// ---------------------------------------------------------------------------

#[test]
fn extension_inherits_html_target() {
    let (dir, qmd) = setup_extension_project(
        "---\ntitle = \"Inheritance Test\"\n---\n\nSimple content.\n",
    );
    let html = render_in_project(dir.path(), &qmd, "myext");

    // Should produce a valid HTML document (inherited from html target)
    assert!(
        html.contains("<!doctype html>") || html.contains("<html"),
        "extension inheriting html should produce HTML output: {}",
        html
    );
    assert!(
        html.contains("Simple content"),
        "body content should be present"
    );
}

// ---------------------------------------------------------------------------
// Extension vars do not override user front matter vars
// ---------------------------------------------------------------------------

#[test]
fn user_vars_override_extension_vars() {
    // User sets [myext] table in front matter; unknown top-level keys become
    // metadata.var entries, which block extension var injection for that key.
    let (dir, qmd) = setup_extension_project(
        "---\ntitle = \"Override Test\"\n\n[myext]\naccent = \"navy\"\n---\n\nAccent is {{ cfg.myext.accent }}.\n",
    );
    let html = render_in_project(dir.path(), &qmd, "myext");

    assert!(
        html.contains("navy"),
        "user front matter var should override extension var: {}",
        html
    );
}

// ---------------------------------------------------------------------------
// Extension with vars used in custom template
// ---------------------------------------------------------------------------

#[test]
fn extension_vars_available_in_partial() {
    let dir = tempfile::tempdir().unwrap();

    // Set up extension with a template that references a var
    let sidecar = dir.path().join("test_calepin");
    fs::create_dir_all(&sidecar).unwrap();
    fs::write(sidecar.join("config.toml"), "target = \"themed\"\n").unwrap();

    let ext_dir = sidecar.join("extensions").join("themed");
    fs::create_dir_all(&ext_dir).unwrap();
    fs::write(ext_dir.join("extension.toml"), r#"
name = "themed"
description = "Themed extension"
inherits = "html"

[vars]
brand_color = "rebeccapurple"
"#).unwrap();

    let tpl_dir = ext_dir.join("templates").join("html");
    fs::create_dir_all(&tpl_dir).unwrap();
    fs::write(tpl_dir.join("note.html"),
        "<div class=\"note\" style=\"border-color: {{ cfg.themed.brand_color }}\">{{ clp.content }}</div>\n"
    ).unwrap();

    let qmd = dir.path().join("test.qmd");
    fs::write(&qmd, "---\ntitle = \"Themed Test\"\n---\n\n::: {.note}\nImportant info.\n:::\n").unwrap();

    let html = render_in_project(dir.path(), &qmd, "themed");

    assert!(
        html.contains("rebeccapurple"),
        "extension var should be available in extension template: {}",
        html
    );
    assert!(
        html.contains("Important info"),
        "div content should be preserved"
    );
}

// ---------------------------------------------------------------------------
// Multiple CSS files in extension assets
// ---------------------------------------------------------------------------

#[test]
fn extension_multiple_css_files() {
    let dir = tempfile::tempdir().unwrap();

    let sidecar = dir.path().join("test_calepin");
    fs::create_dir_all(&sidecar).unwrap();
    fs::write(sidecar.join("config.toml"), "target = \"multicss\"\n").unwrap();

    let ext_dir = sidecar.join("extensions").join("multicss");
    fs::create_dir_all(&ext_dir).unwrap();
    fs::write(ext_dir.join("extension.toml"), r#"
name = "multicss"
description = "Multi-CSS extension"
inherits = "html"

[assets]
css = ["base.css", "theme.css"]
"#).unwrap();

    let assets_dir = ext_dir.join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    fs::write(assets_dir.join("base.css"), ".multi-base { margin: 0; }\n").unwrap();
    fs::write(assets_dir.join("theme.css"), ".multi-theme { color: teal; }\n").unwrap();

    let qmd = dir.path().join("test.qmd");
    fs::write(&qmd, "---\ntitle = \"Multi CSS\"\n---\n\nContent.\n").unwrap();

    let html = render_in_project(dir.path(), &qmd, "multicss");

    assert!(
        html.contains(".multi-base"),
        "first CSS file should be injected: {}",
        html
    );
    assert!(
        html.contains(".multi-theme"),
        "second CSS file should be injected: {}",
        html
    );
}
