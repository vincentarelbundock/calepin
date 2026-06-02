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

#[test]
fn preprocess_writes_runtime_and_results() {
    if !has_command("typst") || !has_command("python3") {
        return;
    }

    let dir = tempfile::tempdir().unwrap();
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
            "preprocess",
            "paper.typ",
            "--root",
            dir.path().to_str().unwrap(),
            "--quiet",
        ])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin preprocess");

    assert!(
        output.status.success(),
        "preprocess failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.path().join(".calepin/calepin.typ").exists());

    let results_path = dir.path().join(".calepin/paper/results.json");
    let results: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(results_path).unwrap()).unwrap();
    assert_eq!(results["schema"], 1);
    assert_eq!(results["chunks"]["answer"]["engine"], "python");
    assert_eq!(results["chunks"]["answer"]["items"][0]["type"], "stream");
    assert_eq!(results["chunks"]["answer"]["items"][0]["text"], "42");
}

#[test]
fn compile_runs_preprocess_and_typst_compile() {
    if !has_command("typst") || !has_command("python3") {
        return;
    }

    let dir = tempfile::tempdir().unwrap();
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
            "--root",
            dir.path().to_str().unwrap(),
            "--quiet",
            "--no-cache",
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
