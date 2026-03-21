//! Integration test: render bench/text.qmd to LaTeX, Typst, and compile to PDF.
//! Ensures the benchmark document produces valid output in all compiled formats.

use std::fs;
use std::path::Path;
use std::process::Command;

/// Path to bench/ relative to the workspace root.
fn bench_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("bench").leak()
}

#[test]
fn bench_text_latex_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let bench = bench_dir();

    // Copy source files to temp dir
    fs::copy(bench.join("text.qmd"), dir.path().join("text.qmd")).unwrap();
    fs::copy(bench.join("library.bib"), dir.path().join("library.bib")).unwrap();

    let bin = Path::new(env!("CARGO_BIN_EXE_calepin"));

    // Render to .tex
    let output = Command::new(bin)
        .args(["text.qmd", "-f", "latex", "-q"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin");
    assert!(
        output.status.success(),
        "calepin render to latex failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let tex_path = dir.path().join("text.tex");
    assert!(tex_path.exists(), "text.tex should exist");

    // Compile to PDF with pdflatex (skip if not installed).
    // Use pdflatex directly (not latexmk) because calepin resolves
    // bibliography internally via hayagriva — no BibTeX pass needed.
    let pdflatex_check = Command::new("pdflatex").arg("--version").output();
    if pdflatex_check.is_ok() && pdflatex_check.unwrap().status.success() {
        let output = Command::new("pdflatex")
            .args([
                "-interaction=nonstopmode",
                "-halt-on-error",
                "text.tex",
            ])
            .current_dir(dir.path())
            .output()
            .expect("failed to run pdflatex");
        assert!(
            output.status.success(),
            "pdflatex compile failed:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let pdf_path = dir.path().join("text.pdf");
        assert!(pdf_path.exists(), "text.pdf should exist");
        assert!(fs::metadata(&pdf_path).unwrap().len() > 0, "PDF should not be empty");
    }
}

#[test]
fn bench_text_typst_pdf() {
    let dir = tempfile::tempdir().unwrap();
    let bench = bench_dir();

    // Copy source files to temp dir
    fs::copy(bench.join("text.qmd"), dir.path().join("text.qmd")).unwrap();
    fs::copy(bench.join("library.bib"), dir.path().join("library.bib")).unwrap();

    let bin = Path::new(env!("CARGO_BIN_EXE_calepin"));

    // Render to .typ
    let output = Command::new(bin)
        .args(["text.qmd", "-f", "typst", "-q"])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin");
    assert!(
        output.status.success(),
        "calepin render to typst failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let typ_path = dir.path().join("text.typ");
    assert!(typ_path.exists(), "text.typ should exist");

    // Compile to PDF with typst (skip if not installed)
    let typst_check = Command::new("typst").arg("--version").output();
    if typst_check.is_ok() && typst_check.unwrap().status.success() {
        let pdf_path = dir.path().join("text.pdf");
        let output = Command::new("typst")
            .args(["compile", "text.typ", pdf_path.to_str().unwrap()])
            .current_dir(dir.path())
            .output()
            .expect("failed to run typst");
        assert!(
            output.status.success(),
            "typst compile failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(pdf_path.exists(), "text.pdf should exist");
        assert!(fs::metadata(&pdf_path).unwrap().len() > 0, "PDF should not be empty");
    }
}
