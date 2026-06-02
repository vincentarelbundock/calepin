use anyhow::Result;
use std::borrow::Cow;
use std::ffi::OsString;
use std::path::Path;

use super::{path_arg, run_checked_tool, run_tool, tool_error, DiagramRun};
use crate::engines::EngineResult;
use crate::utils::tools;

pub(super) const INPUT_EXT: &str = "tex";

pub(super) fn prepare_source(code: &str) -> Cow<'_, str> {
    if code.contains("\\documentclass") || code.contains("\\begin{document}") {
        return Cow::Borrowed(code);
    }

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

    Cow::Owned(format!(
        "\\documentclass{{standalone}}\n\\usepackage{{tikz}}\n\\usepackage{{amsmath}}\n{}\n\\begin{{document}}\n{}\n\\end{{document}}\n",
        preamble.join("\n"),
        body.join("\n")
    ))
}

pub(super) fn render(run: &DiagramRun<'_>, results: &mut Vec<EngineResult>) -> Result<bool> {
    if !run_checked_tool(
        &tools::TECTONIC,
        &run.executables.tectonic,
        &tectonic_args(run),
        results,
    )? {
        return Ok(false);
    }

    let pdf_path = run.work_dir.join("input.pdf");
    let Some(output) = run_tool(
        &tools::DVISVGM,
        &run.executables.dvisvgm,
        &dvisvgm_args(&pdf_path, run.fig_path),
        results,
    )?
    else {
        return Ok(false);
    };
    if output.status.success() {
        return Ok(true);
    }

    let Some(fallback) = run_tool(
        &tools::PDF2SVG,
        &run.executables.pdf2svg,
        &pdf2svg_args(&pdf_path, run.fig_path),
        results,
    )?
    else {
        return Ok(false);
    };
    if fallback.status.success() {
        return Ok(true);
    }

    results.push(tool_error(&run.executables.dvisvgm, output.stderr));
    results.push(tool_error(&run.executables.pdf2svg, fallback.stderr));
    Ok(false)
}

fn tectonic_args(run: &DiagramRun<'_>) -> Vec<OsString> {
    vec![
        "--outdir".into(),
        path_arg(run.work_dir),
        path_arg(run.input_path),
    ]
}

fn dvisvgm_args(pdf_path: &Path, fig_path: &Path) -> Vec<OsString> {
    vec![
        "--pdf".into(),
        "-o".into(),
        path_arg(fig_path),
        path_arg(pdf_path),
    ]
}

fn pdf2svg_args(pdf_path: &Path, fig_path: &Path) -> Vec<OsString> {
    vec![path_arg(pdf_path), path_arg(fig_path)]
}

#[cfg(test)]
mod tests {
    use super::super::execute_diagram;
    use super::super::test_support::{
        assert_successful_plot, env_lock, write_executable, EnvVarGuard,
    };
    use super::*;
    use crate::config::ExecutablePaths;
    use crate::typst::model::EngineName;

    #[test]
    fn falls_back_to_pdf2svg_when_dvisvgm_cannot_read_pdf() {
        let _guard = env_lock();
        let temp_dir = tempfile::tempdir().unwrap();
        let bin_dir = temp_dir.path().join("bin");
        std::fs::create_dir(&bin_dir).unwrap();

        write_executable(
            &bin_dir.join("tectonic"),
            r#"#!/bin/sh
outdir=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --outdir) shift; outdir="$1" ;;
  esac
  shift
done
printf "fake pdf" > "$outdir/input.pdf"
"#,
        );
        write_executable(
            &bin_dir.join("dvisvgm"),
            r#"#!/bin/sh
echo "ERROR: can't retrieve number of pages from file $4" >&2
exit 1
"#,
        );
        write_executable(
            &bin_dir.join("pdf2svg"),
            r#"#!/bin/sh
printf "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>" > "$2"
"#,
        );
        let _path = EnvVarGuard::prepend_path(bin_dir);

        let fig_path = temp_dir.path().join("figure.svg");
        let source = vec!["\\begin{tikzpicture}".to_string()];
        let results = execute_diagram(
            "\\begin{tikzpicture}\n\\end{tikzpicture}",
            EngineName::Tikz,
            &fig_path,
            &source,
            &ExecutablePaths::defaults(),
        )
        .unwrap();

        assert_successful_plot(&results, &fig_path);
    }

    #[test]
    fn wraps_body_in_standalone_document() {
        let document = prepare_source("\\usetikzlibrary{arrows.meta}\n\\begin{tikzpicture}\n\\draw (0,0) -- (1,1);\n\\end{tikzpicture}");
        assert!(document.contains("\\documentclass{standalone}"));
        assert!(document.contains("\\usetikzlibrary{arrows.meta}"));
        assert!(document.contains("\\begin{document}"));
        assert!(document.contains("\\begin{tikzpicture}"));
    }

    #[test]
    fn leaves_complete_document_unchanged() {
        let source = "\\documentclass{standalone}\n\\begin{document}\nX\n\\end{document}";
        assert_eq!(prepare_source(source), source);
    }
}
