use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{bail, Result};

pub fn compile_to_pdf(output_path: &Path, quiet: bool) -> Result<()> {
    let ext = output_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext {
        "tex" => compile_latex(output_path, quiet),
        "typ" => compile_typst(output_path, quiet),
        _ => {
            cwarn!(
                "--compile has no effect for .{} files (only .tex and .typ are supported)",
                ext
            );
            Ok(())
        }
    }
}

/// Write full compiler output to `<stem>.log` next to the output file.
fn write_log(output_path: &Path, stderr: &[u8], stdout: &[u8]) {
    let log_path = output_path.with_extension("log");
    let mut log = String::new();
    if !stdout.is_empty() {
        log.push_str(&String::from_utf8_lossy(stdout));
    }
    if !stderr.is_empty() {
        log.push_str(&String::from_utf8_lossy(stderr));
    }
    if !log.is_empty() {
        let _ = std::fs::write(&log_path, &log);
    }
}

fn compile_latex(tex_path: &Path, quiet: bool) -> Result<()> {
    if !quiet {
        eprintln!("Compiling LaTeX with tectonic…");
    }

    let mut cmd = Command::new("tectonic");
    cmd.arg(tex_path);
    cmd.arg("--chatter=minimal");
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let output = cmd.output().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => anyhow::anyhow!(
            "tectonic not found. Install it from https://tectonic-typesetting.github.io/"
        ),
        _ => anyhow::anyhow!("Failed to run tectonic: {}", e),
    })?;

    if !output.status.success() {
        write_log(tex_path, &output.stderr, &output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let errors: Vec<&str> = stderr.lines()
            .filter(|l| l.starts_with("error:"))
            .collect();
        if !errors.is_empty() {
            for line in &errors {
                eprintln!("{}", line);
            }
        }
        bail!("tectonic exited with {} (see {})", output.status, tex_path.with_extension("log").display());
    }

    if !quiet {
        eprintln!("→ {}", tex_path.with_extension("pdf").display());
    }
    Ok(())
}

fn compile_typst(typ_path: &Path, quiet: bool) -> Result<()> {
    if !quiet {
        eprintln!("Compiling Typst…");
    }

    let mut cmd = Command::new("typst");
    cmd.args(["compile", &typ_path.to_string_lossy()]);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let output = cmd.output().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            anyhow::anyhow!("typst not found. Install it from https://typst.app/")
        }
        _ => anyhow::anyhow!("Failed to run typst: {}", e),
    })?;

    if !output.status.success() {
        write_log(typ_path, &output.stderr, &output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let errors: Vec<&str> = stderr.lines()
            .filter(|l| l.starts_with("error:") || l.contains("error:"))
            .collect();
        if !errors.is_empty() {
            for line in &errors {
                eprintln!("{}", line);
            }
        }
        bail!("typst exited with {} (see {})", output.status, typ_path.with_extension("log").display());
    }

    if !quiet {
        eprintln!("→ {}", typ_path.with_extension("pdf").display());
    }
    Ok(())
}
