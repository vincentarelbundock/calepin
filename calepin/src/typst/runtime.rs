use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub const RUNTIME_SOURCE: &str = include_str!("runtime.typ");

pub fn write_runtime(root: &Path) -> Result<PathBuf> {
    let calepin_dir = root.join(".calepin");
    std::fs::create_dir_all(&calepin_dir)
        .with_context(|| format!("failed to create {}", calepin_dir.display()))?;
    let path = calepin_dir.join("calepin.typ");
    std::fs::write(&path, RUNTIME_SOURCE)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    #[test]
    fn runtime_source_exports_setup_and_chunk() {
        assert!(RUNTIME_SOURCE.contains("#let setup("));
        assert!(RUNTIME_SOURCE.contains("#let chunk(\n  engine,"));
        assert!(RUNTIME_SOURCE.contains("state(\"calepin-auto-label-index\", 1)"));
        assert!(RUNTIME_SOURCE.contains("<calepin-chunk>"));
        assert!(RUNTIME_SOURCE.contains("<calepin-config>"));
    }

    #[test]
    fn runtime_uses_code_block_styling_for_text_outputs() {
        assert_eq!(
            RUNTIME_SOURCE
                .matches("lang: \"text\"")
                .count(),
            3
        );
    }

    #[test]
    fn runtime_trims_leading_newline_from_echoed_code() {
        assert!(RUNTIME_SOURCE.contains("code.starts-with(\"\\n\")"));
        assert!(RUNTIME_SOURCE.contains("code.slice(1)"));
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

        let dir = tempfile::tempdir().unwrap();
        write_runtime(dir.path()).unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(
            &input,
            r##"#import ".calepin/calepin.typ"

#calepin.setup(cache: false)

#calepin.chunk("r")[```
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
        assert!(stdout.contains(r#""label": "chunk-2""#));
        assert!(stdout.contains(r#""engine": "python""#));
        assert!(stdout.contains(r#""text": "print(\"hello\")""#));
    }

    #[test]
    fn typst_compile_without_results_shows_code() {
        if Command::new("typst").arg("--version").output().is_err() {
            return;
        }
        if Command::new("pdftotext").arg("-v").output().is_err() {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
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
        assert!(extracted.contains("FALLBACK_12345"), "expected source code in PDF output");
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

        let dir = tempfile::tempdir().unwrap();
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
        assert!(extracted.contains("print(\"RESULT_12345\")"), "expected source code in PDF output");
        assert!(extracted.contains("RESULT_12345"), "expected execution output in PDF output");
    }
}
