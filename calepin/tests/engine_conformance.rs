// Cross-engine conformance suite: the same set of chunk scripts, translated
// per language, run against every installed engine (R via Rscript, Python
// via python3, and a Jupyter kernel via jupyter_client) and are asserted to
// produce the same observable shape of results.json, per
// calepin/src/engines/PROTOCOL.md.
//
// Each engine is skipped independently when its tool is absent, following
// the pattern in tests/typst_preprocess.rs (has_command). A run on a machine
// with only R installed still exercises the R half of every scenario; the
// suite is meant to catch cross-engine drift on a machine with all three.

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

fn has_python_module(module: &str) -> bool {
    let code = format!("import {module}");
    Command::new("python3")
        .args(["-c", &code])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn has_jupyter_kernel(kernel: &str) -> bool {
    if !has_python_module("jupyter_client") {
        return false;
    }
    let code = format!(
        "from jupyter_client.kernelspec import KernelSpecManager; KernelSpecManager().get_kernel_spec({kernel:?})"
    );
    Command::new("python3")
        .args(["-c", &code])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn typst_accessible_tempdir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("calepin-engine-conformance-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap()
}

/// Writes `body` into a minimal document (with `echo`/`eval` defaults on),
/// compiles it to HTML, and returns the parsed `results.json`. Panics with
/// calepin's stderr if the compile itself fails (a scenario that is
/// *expected* to error mid-chunk uses `error: true` on the chunk so the
/// overall compile still succeeds; only the item-level error is asserted).
fn compile_and_read_results(dir: &Path, body: &str) -> serde_json::Value {
    std::fs::write(
        dir.join("paper.typ"),
        format!(
            "#import \".calepin/calepin.typ\" as calepin\n#calepin.setup(echo: true, eval: true)\n\n{body}\n"
        ),
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args([
            "compile",
            "paper.typ",
            "paper.html",
            "--format",
            "html",
            "--quiet",
        ])
        .current_dir(dir)
        .output()
        .expect("failed to run calepin compile");
    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let results_path = dir.join(".calepin/paper/results.json");
    serde_json::from_str(
        &std::fs::read_to_string(&results_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", results_path.display())),
    )
    .unwrap()
}

fn chunk_items<'a>(results: &'a serde_json::Value, label: &str) -> Vec<&'a serde_json::Value> {
    results["chunks"][label]["items"]
        .as_array()
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

fn items_of_type<'a>(items: &[&'a serde_json::Value], item_type: &str) -> Vec<&'a serde_json::Value> {
    items
        .iter()
        .copied()
        .filter(|item| item["type"] == item_type)
        .collect()
}

fn any_text_contains(items: &[&serde_json::Value], needle: &str) -> bool {
    items.iter().any(|item| {
        item["text"]
            .as_str()
            .map(|text| text.contains(needle))
            .unwrap_or(false)
    })
}

fn index_of_first(items: &[&serde_json::Value], item_type: &str) -> Option<usize> {
    items.iter().position(|item| item["type"] == item_type)
}

/// One engine under test: the fence language written into the `.typ`
/// document (a bare `"r"`/`"python"`, or a Jupyter kernel name), plus the
/// per-scenario source translated into that language.
struct Engine {
    name: &'static str,
    lang: &'static str,
    available: fn() -> bool,
    print_only: &'static str,
    plot_then_print: &'static str,
    error_mid_chunk: &'static str,
    error_needle: &'static str,
    failing_statement_needle: &'static str,
    multi_statement_no_output: &'static str,
    fd_level_stdout: &'static str,
    tbl_chunk_stray_plot: &'static str,
    fig_chunk_after_tbl: &'static str,
}

fn engines() -> Vec<Engine> {
    vec![
        Engine {
            name: "r",
            lang: "r",
            available: || has_command("Rscript"),
            print_only: r#"cat("hello")"#,
            plot_then_print: "plot(1:3)\ncat(\"done\")",
            error_mid_chunk: "cat(1)\nstop(\"boom\")",
            error_needle: "boom",
            failing_statement_needle: "stop(\"boom\")",
            multi_statement_no_output: "x <- 1\ny <- 2",
            fd_level_stdout: r#"system("echo hi")"#,
            tbl_chunk_stray_plot: "plot(1:3)",
            fig_chunk_after_tbl: r#"cat("ok")"#,
        },
        Engine {
            name: "python",
            lang: "python",
            available: || has_command("python3") && has_python_module("matplotlib"),
            print_only: r#"print("hello")"#,
            plot_then_print: "import matplotlib.pyplot as plt\nplt.plot([1, 2, 3])\nprint(\"done\")",
            error_mid_chunk: "print(1)\nraise ValueError(\"boom\")",
            error_needle: "boom",
            failing_statement_needle: "raise ValueError(\"boom\")",
            multi_statement_no_output: "x = 1\ny = 2",
            fd_level_stdout: "import subprocess\nsubprocess.run(['echo', 'hi'])",
            tbl_chunk_stray_plot: "import matplotlib.pyplot as plt\nplt.plot([1, 2, 3])",
            fig_chunk_after_tbl: r#"print("ok")"#,
        },
        // A Jupyter kernel is a fundamentally different execution model from
        // the native r/python engines: the whole chunk is sent to the kernel
        // as one `execute_request` (see `_execute` in jupyter.rs), so there
        // is exactly one `_SOURCE:` part covering the whole chunk rather
        // than one per statement. The scenarios below are still meaningful
        // (a plot must still show up after an explicit `display()` call, an
        // error still aborts the remaining statements, fd-level output is
        // still captured), just verified as a chunk-wide property instead of
        // a per-statement one. Uses the "python3" Jupyter kernel (distinct
        // from calepin's native `python` engine) so it isn't gated on the
        // `sh`/`julia` kernel-aliasing behavior, which is out of scope here.
        Engine {
            name: "jupyter (python3 kernel)",
            lang: "python3",
            available: || has_jupyter_kernel("python3") && has_python_module("matplotlib"),
            print_only: r#"print("hello")"#,
            plot_then_print: concat!(
                "import matplotlib.pyplot as plt\n",
                "from IPython.display import display\n",
                "fig = plt.figure()\n",
                "plt.plot([1, 2, 3])\n",
                "display(fig)\n",
                "print(\"done\")",
            ),
            error_mid_chunk: "print(1)\nraise ValueError(\"boom\")",
            error_needle: "boom",
            failing_statement_needle: "raise ValueError(\"boom\")",
            multi_statement_no_output: "x = 1\ny = 2",
            fd_level_stdout: "import subprocess\nsubprocess.run(['echo', 'hi'])",
            tbl_chunk_stray_plot: concat!(
                "import matplotlib.pyplot as plt\n",
                "from IPython.display import display\n",
                "fig = plt.figure()\n",
                "plt.plot([1, 2, 3])\n",
                "display(fig)",
            ),
            fig_chunk_after_tbl: r#"print("ok")"#,
        },
    ]
}

fn chunk_doc(lang: &str, label: &str, code: &str, extra_options: &str) -> String {
    format!(
        "#calepin.chunk(\"{lang}\", label: \"{label}\"{extra_options})[```\n{code}\n```]\n"
    )
}

#[test]
fn print_only_produces_one_source_and_one_stdout_stream() {
    if !has_command("typst") {
        return;
    }
    for engine in engines() {
        if !(engine.available)() {
            continue;
        }
        let dir = typst_accessible_tempdir();
        let body = chunk_doc(engine.lang, "print-only", engine.print_only, "");
        let results = compile_and_read_results(dir.path(), &body);
        let items = chunk_items(&results, "print-only");

        assert!(
            !items_of_type(&items, "source").is_empty(),
            "[{}] expected a source item, got {items:#?}",
            engine.name
        );
        let streams = items_of_type(&items, "stream");
        assert!(
            any_text_contains(&streams, "hello"),
            "[{}] expected a stdout stream containing `hello`, got {items:#?}",
            engine.name
        );
        let source_at = index_of_first(&items, "source").unwrap();
        let stream_at = index_of_first(&items, "stream").unwrap();
        assert!(
            source_at < stream_at,
            "[{}] source must be echoed before the output it produced, got {items:#?}",
            engine.name
        );
    }
}

#[test]
fn a_plot_is_never_emitted_before_the_source_that_drew_it() {
    if !has_command("typst") {
        return;
    }
    for engine in engines() {
        if !(engine.available)() {
            continue;
        }
        let dir = typst_accessible_tempdir();
        let body = chunk_doc(engine.lang, "fig-plot", engine.plot_then_print, "");
        let results = compile_and_read_results(dir.path(), &body);
        let items = chunk_items(&results, "fig-plot");

        let displays = items_of_type(&items, "display");
        assert!(
            !displays.is_empty(),
            "[{}] expected a display (plot) item, got {items:#?}",
            engine.name
        );
        let streams = items_of_type(&items, "stream");
        assert!(
            any_text_contains(&streams, "done"),
            "[{}] expected a stdout stream containing `done`, got {items:#?}",
            engine.name
        );

        let first_source = index_of_first(&items, "source")
            .unwrap_or_else(|| panic!("[{}] expected a source item, got {items:#?}", engine.name));
        let first_display = index_of_first(&items, "display").unwrap();
        assert!(
            first_source < first_display,
            "[{}] the plot must never be emitted before its source, got {items:#?}",
            engine.name
        );

        // The figure was drawn before the print("done") call in every
        // translation above, so the plot must come before that output.
        let first_stream = index_of_first(&items, "stream").unwrap();
        assert!(
            first_display < first_stream,
            "[{}] the plot was drawn before the print() call in the source, so it must \
             appear before that output too, got {items:#?}",
            engine.name
        );
    }
}

#[test]
fn a_statement_that_errors_still_has_its_source_echoed() {
    if !has_command("typst") {
        return;
    }
    for engine in engines() {
        if !(engine.available)() {
            continue;
        }
        let dir = typst_accessible_tempdir();
        let body = chunk_doc(
            engine.lang,
            "err-tolerated",
            engine.error_mid_chunk,
            ", error: true",
        );
        let results = compile_and_read_results(dir.path(), &body);
        let items = chunk_items(&results, "err-tolerated");

        let errors = items_of_type(&items, "error");
        assert!(
            any_text_contains(&errors, engine.error_needle)
                || errors
                    .iter()
                    .any(|item| item["message"].as_str().unwrap_or("").contains(engine.error_needle)),
            "[{}] expected an error item mentioning `{}`, got {items:#?}",
            engine.name,
            engine.error_needle
        );

        let sources = items_of_type(&items, "source");
        assert!(
            any_text_contains(&sources, engine.failing_statement_needle),
            "[{}] the failing statement's source must still be echoed, got {items:#?}",
            engine.name
        );
    }
}

#[test]
fn statements_with_no_output_are_batched_into_one_source_echo() {
    if !has_command("typst") {
        return;
    }
    for engine in engines() {
        if !(engine.available)() {
            continue;
        }
        let dir = typst_accessible_tempdir();
        let body = chunk_doc(engine.lang, "silent", engine.multi_statement_no_output, "");
        let results = compile_and_read_results(dir.path(), &body);
        let items = chunk_items(&results, "silent");
        let sources = items_of_type(&items, "source");

        assert_eq!(
            sources.len(),
            1,
            "[{}] statements producing no output must not fragment the source echo, got {items:#?}",
            engine.name
        );
    }
}

#[test]
fn fd_level_stdout_is_reported_as_output_not_dropped() {
    if !has_command("typst") {
        return;
    }
    for engine in engines() {
        if !(engine.available)() {
            continue;
        }
        let dir = typst_accessible_tempdir();
        let body = chunk_doc(engine.lang, "fd-stdout", engine.fd_level_stdout, "");
        let results = compile_and_read_results(dir.path(), &body);
        let items = chunk_items(&results, "fd-stdout");
        let streams = items_of_type(&items, "stream");

        assert!(
            any_text_contains(&streams, "hi"),
            "[{}] fd-level stdout (subprocess/system) must surface as output, got {items:#?}",
            engine.name
        );
    }
}

#[test]
fn a_plot_left_open_in_a_table_chunk_does_not_leak_into_the_next_figure_chunk() {
    if !has_command("typst") {
        return;
    }
    for engine in engines() {
        if !(engine.available)() {
            continue;
        }
        let dir = typst_accessible_tempdir();
        let body = format!(
            "{}\n{}",
            chunk_doc(engine.lang, "tbl-leak", engine.tbl_chunk_stray_plot, ""),
            chunk_doc(engine.lang, "fig-after-tbl", engine.fig_chunk_after_tbl, ""),
        );
        let results = compile_and_read_results(dir.path(), &body);

        let tbl_items = chunk_items(&results, "tbl-leak");
        assert!(
            items_of_type(&tbl_items, "display").is_empty(),
            "[{}] a table chunk must not emit a plot, got {tbl_items:#?}",
            engine.name
        );

        let fig_items = chunk_items(&results, "fig-after-tbl");
        assert!(
            items_of_type(&fig_items, "display").is_empty(),
            "[{}] a stray plot from an earlier table chunk must not leak into the next figure chunk, got {fig_items:#?}",
            engine.name
        );
        let streams = items_of_type(&fig_items, "stream");
        assert!(
            any_text_contains(&streams, "ok"),
            "[{}] the figure chunk after the table chunk should still run normally, got {fig_items:#?}",
            engine.name
        );
    }
}
