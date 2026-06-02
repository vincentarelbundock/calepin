use anyhow::Result;
use std::ffi::OsString;

use super::{path_arg, run_checked_tool, DiagramRun};
use crate::engines::EngineResult;
use crate::utils::tools;

pub(super) const INPUT_EXT: &str = "dot";

pub(super) fn render(run: &DiagramRun<'_>, results: &mut Vec<EngineResult>) -> Result<bool> {
    run_checked_tool(&tools::DOT, &run.executables.dot, &args(run), results)
}

fn args(run: &DiagramRun<'_>) -> Vec<OsString> {
    vec![
        "-Tsvg".into(),
        "-o".into(),
        path_arg(run.fig_path),
        path_arg(run.input_path),
    ]
}
