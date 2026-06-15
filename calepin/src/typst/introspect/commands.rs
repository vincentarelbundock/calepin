use anyhow::Result;
use std::ffi::OsString;
use std::path::Path;

use super::root_relative;
use crate::typst::model::LayoutPaths;
use crate::typst::run::{push_calepin_inputs, run_typst_capture, CalepinMode, CalepinTarget, TypstInput};

pub(super) fn typst_query(
    typst: &Path,
    layout: &LayoutPaths,
    input: &Path,
    selector: &str,
    results_input: &str,
    mode: CalepinMode,
    target: CalepinTarget,
) -> Result<String> {
    let mut args: Vec<OsString> = vec![
        "query".into(),
        input.as_os_str().into(),
        selector.into(),
        "--root".into(),
        layout.root.as_os_str().into(),
    ];
    push_calepin_inputs(&mut args, mode, results_input, target);
    // Keep introspection-compatible Typst paths working on documents that
    // reference Typst's HTML module during metadata extraction.
    args.push("--features=html".into());
    run_typst_capture(
        typst,
        "run typst query",
        &args,
        &layout.root,
        |stderr| format!("typst query {selector} failed:\n{stderr}"),
        "typst query output was not UTF-8",
    )
}

pub(super) fn query_page_anchors(
    typst: &Path,
    layout: &LayoutPaths,
    selector: &str,
    results_input: &str,
    mode: CalepinMode,
    target: CalepinTarget,
) -> Result<String> {
    let mut args: Vec<OsString> = vec![
        "query".into(),
        layout.render_input.as_os_str().into(),
        selector.into(),
        "--root".into(),
        layout.root.as_os_str().into(),
    ];
    push_calepin_inputs(&mut args, mode, results_input, target);
    run_typst_capture(
        typst,
        "run typst page sync query",
        &args,
        &layout.root,
        |stderr| format!("typst query {selector} failed:\n{stderr}"),
        "typst page sync query output was not UTF-8",
    )
}

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
