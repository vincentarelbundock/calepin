use anyhow::Result;
use std::ffi::OsString;
use std::path::Path;

use super::root_relative;
use crate::typst::model::LayoutPaths;
use crate::typst::run::{run_typst_capture, TypstInput};

pub(super) fn typst_eval(
    typst: &Path,
    layout: &LayoutPaths,
    input: &Path,
    expression: &str,
    inputs: &[TypstInput],
) -> Result<String> {
    let input = root_relative(input, &layout.root);
    let mut args: Vec<OsString> = vec![
        "eval".into(),
        expression.into(),
        "--in".into(),
        input.as_os_str().into(),
        "--root".into(),
        layout.root.as_os_str().into(),
        "--format".into(),
        "json".into(),
        // Documents may use Typst's HTML module even during metadata
        // introspection; enable the feature just as the final HTML compile does.
        "--features=html".into(),
    ];
    for input in inputs {
        input.push_to(&mut args);
    }
    run_typst_capture(
        typst,
        "run typst eval",
        &args,
        &layout.root,
        |stderr| format!("typst eval failed:\n{stderr}"),
        "typst eval output was not UTF-8",
    )
}
