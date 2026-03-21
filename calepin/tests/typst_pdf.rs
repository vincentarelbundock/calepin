//! Integration test: render a .qmd with heading cross-references to Typst,
//! compile to PDF, and verify success.

use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn test_typst_heading_crossrefs_compile() {
    let dir = tempfile::tempdir().unwrap();
    let qmd = dir.path().join("test.qmd");
    fs::write(&qmd, r#"---
title: Cross-ref Test
---

# Introduction {#sec-intro}

Hello world.

# Methods {#sec-methods}

As discussed in @sec-intro, we proceed.

See @sec-methods for details.
"#).unwrap();

    // Find the calepin binary
    let bin = Path::new(env!("CARGO_BIN_EXE_calepin"));

    // Render to .typ
    let output = Command::new(bin)
        .args(["render", qmd.to_str().unwrap(), "-f", "typst"])
        .output()
        .expect("failed to run calepin");
    assert!(output.status.success(), "calepin render failed: {}", String::from_utf8_lossy(&output.stderr));

    let typ_path = dir.path().join("test.typ");
    assert!(typ_path.exists(), "test.typ should exist");

    let typ_content = fs::read_to_string(&typ_path).unwrap();
    // Verify explicit heading IDs are used
    assert!(typ_content.contains("<sec-intro>"), "should have <sec-intro> label: {}", typ_content);
    assert!(typ_content.contains("<sec-methods>"), "should have <sec-methods> label: {}", typ_content);
    // Verify {#id} syntax is stripped from heading text
    assert!(!typ_content.contains("{#sec-"), "should not contain raw {{#sec- syntax: {}", typ_content);

    // Compile to PDF with typst (skip if typst not installed)
    let typst_check = Command::new("typst").arg("--version").output();
    if typst_check.is_ok() && typst_check.unwrap().status.success() {
        let pdf_path = dir.path().join("test.pdf");
        let output = Command::new("typst")
            .args(["compile", typ_path.to_str().unwrap(), pdf_path.to_str().unwrap()])
            .output()
            .expect("failed to run typst");
        assert!(
            output.status.success(),
            "typst compile failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(pdf_path.exists(), "test.pdf should exist");
        assert!(fs::metadata(&pdf_path).unwrap().len() > 0, "PDF should not be empty");
    }
}
