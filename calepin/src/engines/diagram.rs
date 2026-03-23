// Diagram engines: stateless CLI tools that convert source code to SVG.
//
// Supported engines: mermaid (mmdc), dot (graphviz), tikz (tectonic + pdf2svg),
// d2, penrose (roger).

use anyhow::Result;
use std::ffi::OsString;
use std::path::Path;

use crate::types::{ChunkOptions, ChunkResult};

/// Diagram engine spec: input extension, CLI command, install hint.
struct DiagramSpec {
    input_ext: &'static str,
    cmd: &'static str,
    install_hint: &'static str,
}

fn diagram_spec(engine: &str) -> Option<DiagramSpec> {
    match engine {
        "mermaid" => Some(DiagramSpec {
            input_ext: "mmd",
            cmd: "mmdc",
            install_hint: "install with: npm install -g @mermaid-js/mermaid-cli",
        }),
        "dot" => Some(DiagramSpec {
            input_ext: "dot",
            cmd: "dot",
            install_hint: "install Graphviz: https://graphviz.org/download/",
        }),
        "tikz" => Some(DiagramSpec {
            input_ext: "tex",
            cmd: "tectonic",
            install_hint: "install Tectonic: https://tectonic-typesetting.github.io/",
        }),
        "d2" => Some(DiagramSpec {
            input_ext: "d2",
            cmd: "d2",
            install_hint: "install D2: https://d2lang.com/",
        }),
        _ => None,
    }
}

/// Build CLI arguments for a diagram engine: input_path -> fig_path (SVG).
fn diagram_args(engine: &str, input_path: &Path, fig_path: &Path) -> Vec<OsString> {
    match engine {
        "mermaid" => vec![
            "-i".into(), input_path.as_os_str().into(),
            "-o".into(), fig_path.as_os_str().into(),
            "-b".into(), "transparent".into(),
        ],
        "dot" => vec![
            "-Tsvg".into(),
            "-o".into(), fig_path.as_os_str().into(),
            input_path.as_os_str().into(),
        ],
        "tikz" => vec![input_path.as_os_str().into()],
        "d2" => vec![input_path.as_os_str().into(), fig_path.as_os_str().into()],
        _ => vec![],
    }
}

/// Check whether `engine` is a supported diagram engine.
pub fn is_diagram_engine(engine: &str) -> bool {
    diagram_spec(engine).is_some()
}

/// Execute a diagram engine by writing source to a temp file, calling the CLI,
/// and returning the rendered SVG as a Plot result.
pub fn execute_diagram(
    code: &str,
    engine_name: &str,
    fig_path: &std::path::PathBuf,
    source: &[String],
    options: &ChunkOptions,
) -> Result<Vec<ChunkResult>> {
    use std::io::Write;

    let mut results = Vec::new();
    if !options.eval() {
        results.push(ChunkResult::Source(source.to_vec()));
        return Ok(results);
    }
    results.push(ChunkResult::Source(source.to_vec()));

    let spec = diagram_spec(engine_name)
        .ok_or_else(|| anyhow::anyhow!("Unknown diagram engine: {}", engine_name))?;

    let tmp_dir = std::env::temp_dir();
    let input_path = tmp_dir.join(format!("calepin_diagram.{}", spec.input_ext));

    // TikZ needs a complete LaTeX document wrapper
    let full_code = if engine_name == "tikz" {
        format!(
            "\\documentclass{{standalone}}\n\\usepackage{{tikz}}\n\\begin{{document}}\n{}\n\\end{{document}}",
            code
        )
    } else {
        code.to_string()
    };

    {
        let mut f = std::fs::File::create(&input_path)?;
        f.write_all(full_code.as_bytes())?;
    }

    let run_cmd = |cmd: &str, args: &[OsString]| -> std::io::Result<std::process::Output> {
        std::process::Command::new(cmd).args(args).output()
    };

    // TikZ: compile to PDF, then convert to SVG with pdf2svg
    if engine_name == "tikz" {
        let args = diagram_args(engine_name, &input_path, fig_path);
        match run_cmd(spec.cmd, &args) {
            Ok(out) if !out.status.success() => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                results.push(ChunkResult::Error(format!("{} failed: {}", spec.cmd, stderr)));
                std::fs::remove_file(&input_path).ok();
                return Ok(results);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::fs::remove_file(&input_path).ok();
                results.push(ChunkResult::Error(format!("{} not found on PATH. {}", spec.cmd, spec.install_hint)));
                return Ok(results);
            }
            Err(e) => { std::fs::remove_file(&input_path).ok(); return Err(e.into()); }
            _ => {}
        }
        let pdf_path = input_path.with_extension("pdf");
        match run_cmd("pdf2svg", &[pdf_path.as_os_str().into(), fig_path.as_os_str().into()]) {
            Ok(out) if !out.status.success() => {
                results.push(ChunkResult::Error("pdf2svg conversion failed".to_string()));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::fs::remove_file(&input_path).ok();
                std::fs::remove_file(&pdf_path).ok();
                results.push(ChunkResult::Error("pdf2svg not found on PATH. Install: https://github.com/dawbarton/pdf2svg".to_string()));
                return Ok(results);
            }
            Err(e) => { std::fs::remove_file(&input_path).ok(); return Err(e.into()); }
            _ => {}
        }
        std::fs::remove_file(&pdf_path).ok();
    } else {
        let args = diagram_args(engine_name, &input_path, fig_path);
        match run_cmd(spec.cmd, &args) {
            Ok(out) if !out.status.success() => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                results.push(ChunkResult::Error(format!("{} failed: {}", spec.cmd, stderr)));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::fs::remove_file(&input_path).ok();
                results.push(ChunkResult::Error(format!("{} not found on PATH. {}", spec.cmd, spec.install_hint)));
                return Ok(results);
            }
            Err(e) => { std::fs::remove_file(&input_path).ok(); return Err(e.into()); }
            _ => {}
        }
    }

    if fig_path.exists() {
        results.push(ChunkResult::Plot(fig_path.to_path_buf()));
    }

    std::fs::remove_file(&input_path).ok();
    Ok(results)
}
