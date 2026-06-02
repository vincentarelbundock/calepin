use anyhow::Result;
use std::ffi::OsString;

use super::{path_arg, run_checked_tool, DiagramRun};
use crate::engines::EngineResult;
use crate::utils::tools;

pub(super) const INPUT_EXT: &str = "d2";

pub(super) fn render(run: &DiagramRun<'_>, results: &mut Vec<EngineResult>) -> Result<bool> {
    run_checked_tool(&tools::D2, &run.executables.d2, &args(run), results)
}

fn args(run: &DiagramRun<'_>) -> Vec<OsString> {
    vec![path_arg(run.input_path), path_arg(run.fig_path)]
}
