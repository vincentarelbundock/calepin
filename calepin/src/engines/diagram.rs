// Diagram engines: stateless CLI tools that convert source code to SVG.
//
// Supported engines: mermaid (mmdc), dot (graphviz), tikz (tectonic + pdf2svg), d2.

use anyhow::Result;
use std::ffi::OsString;
use std::path::Path;

use crate::utils::tools::{self, Tool};
use crate::types::{ChunkOptions, ChunkResult};

/// Diagram engine spec: input file extension and primary tool.
struct DiagramSpec {
    input_ext: &'static str,
    tool: &'static Tool,
}

fn diagram_spec(engine: &str) -> Option<DiagramSpec> {
    match engine {
        "mermaid" => Some(DiagramSpec { input_ext: "mmd", tool: &tools::MMDC }),
        "dot" => Some(DiagramSpec { input_ext: "dot", tool: &tools::DOT }),
        "tikz" => Some(DiagramSpec { input_ext: "tex", tool: &tools::TECTONIC }),
        "d2" => Some(DiagramSpec { input_ext: "d2", tool: &tools::D2 }),
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

/// Run a command, returning output or a ChunkResult::Error for not-found.
/// Returns Ok(None) with error pushed to results if the tool is missing.
fn run_tool(
    tool: &Tool,
    args: &[OsString],
    results: &mut Vec<ChunkResult>,
) -> Result<Option<std::process::Output>> {
    match std::process::Command::new(tool.cmd).args(args).output() {
        Ok(out) => Ok(Some(out)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            results.push(ChunkResult::Error(tools::not_found_message(tool)));
            Ok(None)
        }
        Err(e) => Err(e.into()),
    }
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
    let stem = fig_path.file_stem().unwrap_or_default().to_string_lossy();
    let input_path = tmp_dir.join(format!(
        "calepin_{}_{}.{}",
        std::process::id(),
        stem,
        spec.input_ext
    ));

    // TikZ needs a complete LaTeX document wrapper.
    // Extract preamble commands (\usetikzlibrary, \tikzset, \usepackage)
    // from the chunk body and place them before \begin{document}.
    let full_code = if engine_name == "tikz" {
        let mut preamble = Vec::new();
        let mut body = Vec::new();
        for line in code.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("\\usetikzlibrary")
                || trimmed.starts_with("\\tikzset")
                || trimmed.starts_with("\\usepackage")
                || trimmed.starts_with("\\tikzstyle")
            {
                preamble.push(line);
            } else {
                body.push(line);
            }
        }
        format!(
            "\\documentclass{{standalone}}\n\\usepackage{{tikz}}\n\\usepackage{{amsmath}}\n{}\n\\begin{{document}}\n{}\n\\end{{document}}",
            preamble.join("\n"),
            body.join("\n")
        )
    } else {
        code.to_string()
    };

    {
        let mut f = std::fs::File::create(&input_path)?;
        f.write_all(full_code.as_bytes())?;
    }

    // TikZ: compile to PDF, then convert to SVG with pdf2svg
    if engine_name == "tikz" {
        let args = diagram_args(engine_name, &input_path, fig_path);
        match run_tool(spec.tool, &args, &mut results)? {
            Some(out) if !out.status.success() => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                results.push(ChunkResult::Error(format!("{} failed: {}", spec.tool.cmd, stderr)));
                std::fs::remove_file(&input_path).ok();
                return Ok(results);
            }
            None => { std::fs::remove_file(&input_path).ok(); return Ok(results); }
            _ => {}
        }
        let pdf_path = input_path.with_extension("pdf");
        let pdf2svg_args: Vec<OsString> = vec![pdf_path.as_os_str().into(), fig_path.as_os_str().into()];
        match run_tool(&tools::PDF2SVG, &pdf2svg_args, &mut results)? {
            Some(out) if !out.status.success() => {
                results.push(ChunkResult::Error("pdf2svg conversion failed".to_string()));
            }
            None => {
                std::fs::remove_file(&input_path).ok();
                std::fs::remove_file(&pdf_path).ok();
                return Ok(results);
            }
            _ => {}
        }
        std::fs::remove_file(&pdf_path).ok();
        // Clean up auxiliary files generated by tectonic/LaTeX
        for ext in &["aux", "log", "fls", "synctex.gz", "fdb_latexmk"] {
            std::fs::remove_file(input_path.with_extension(ext)).ok();
        }
    } else {
        let args = diagram_args(engine_name, &input_path, fig_path);
        match run_tool(spec.tool, &args, &mut results)? {
            Some(out) if !out.status.success() => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                results.push(ChunkResult::Error(format!("{} failed: {}", spec.tool.cmd, stderr)));
            }
            None => { std::fs::remove_file(&input_path).ok(); return Ok(results); }
            _ => {}
        }
    }

    if fig_path.exists() {
        results.push(ChunkResult::Plot(fig_path.to_path_buf()));
    }

    std::fs::remove_file(&input_path).ok();
    Ok(results)
}
