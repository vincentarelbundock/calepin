mod assets;
mod relay;
mod watcher;

use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::cli::WatchArgs;
use crate::html::SiteContextInput;
use crate::typst::compile::{
    postprocess_html_output, resolve_output_format, resolve_output_path, typst_watch_args,
    validate_forwarded_typst_args, OutputFormat, ReservedInputs,
};
use crate::typst::preprocess::{
    prepare_preprocess_plan, preprocess_cached, preprocess_cached_plan, PreprocessOptions,
};
use crate::typst::runtime::publish_active_binding;
use crate::typst::version::assert_supported_typst;
use crate::utils::{process, tools};

use relay::{join_relay, relay_typst_watch_output, relay_typst_watch_output_with_events};

fn start_html_output_postprocessor(
    stop: Arc<AtomicBool>,
    output: PathBuf,
    layout: crate::typst::model::LayoutPaths,
    html_entry: Option<crate::theme::HtmlEntry>,
    site_context: Option<Arc<RwLock<SiteContextInput>>>,
    writes: Receiver<PathBuf>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut output = output;
        let mut last_written = std::fs::metadata(&output)
            .ok()
            .map(|meta| (meta.modified().ok(), meta.len()));
        let syntax_theme = crate::html::HtmlSyntaxTheme::builtin();
        let mut pending_update = false;

        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            match writes.recv_timeout(Duration::from_millis(250)) {
                Ok(new_output) => {
                    let next_output = if new_output.is_absolute() {
                        new_output
                    } else {
                        layout.root.join(new_output)
                    };
                    if next_output != output {
                        output = next_output;
                        last_written = std::fs::metadata(&output)
                            .ok()
                            .map(|meta| (meta.modified().ok(), meta.len()));
                    }
                    pending_update = true;
                }
                Err(RecvTimeoutError::Timeout) => {
                    if !pending_update {
                        continue;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
            if stop.load(Ordering::Relaxed) {
                break;
            }

            // `writing to` can arrive before file contents are fully flushed, and
            // some platforms may report unchanged timestamps for rapid updates. Track a
            // (modified_time, size) pair to avoid missing successful writes.
            let state = std::fs::metadata(&output)
                .ok()
                .map(|meta| (meta.modified().ok(), meta.len()));
            let should_refresh = match (last_written, state) {
                (None, Some(_)) => true,
                (Some(previous), Some(current)) => previous != current,
                _ => false,
            };
            if should_refresh {
                let current_site_context = site_context
                    .as_ref()
                    .and_then(|context| context.read().ok().map(|context| context.clone()));
                if let Err(error) = postprocess_html_output(
                    &output,
                    &layout,
                    html_entry.as_ref(),
                    &syntax_theme,
                    current_site_context.as_ref(),
                    None,
                    false,
                ) {
                    cwarn!("failed to postprocess watched HTML output: {}", error);
                } else {
                    last_written = state;
                    pending_update = false;
                }
            }
        }
    })
}

fn preprocess_options(args: &WatchArgs, sync_pages: bool) -> PreprocessOptions {
    PreprocessOptions {
        input: args.input.clone(),
        root: None,
        config: args.common.config.clone(),
        display_root: None,
        quiet: args.common.quiet,
        status: true,
        progress: false,
        timeout: args.common.timeout,
        sync_pages,
        theme: None,
        fallback_theme: crate::theme::ThemeSelection::Default,
        html_syntax_theme: None,
        asset_dir: None,
        config_overrides: args.common.sets.clone(),
        force: false,
    }
}

/// Cell for the `typst watch` child so the signal handler can reach it. Only
/// `run_watch` (not `--eval-only`) ever puts a child in here.
type WatchedChild = Arc<Mutex<Option<Child>>>;

/// Installs one handler for Ctrl+C, SIGTERM, and SIGHUP (the `termination`
/// feature makes `ctrlc` treat all three the same on Unix). The first signal
/// just sets the stop flag so the normal shutdown path runs: the caller's
/// poll loop notices `stop`, kills the `typst watch` child, joins the
/// relay/watcher threads (which is where in-flight engine sessions close,
/// since `EnginePool` is dropped when the current preprocess call returns),
/// and removes the generated entry files.
///
/// A second signal means that normal path is stuck (most likely a long
/// chunk blocking the watcher thread), so it skips straight to killing the
/// `typst watch` child and exiting the process immediately rather than
/// waiting on it.
fn install_signal_handler(
    child: WatchedChild,
    layout: crate::typst::model::LayoutPaths,
    keep_intermediates: bool,
) -> Result<Arc<AtomicBool>> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_handler = Arc::clone(&stop);
    let signal_count = Arc::new(AtomicUsize::new(0));
    ctrlc::set_handler(move || {
        stop_for_handler.store(true, Ordering::Relaxed);
        if signal_count.fetch_add(1, Ordering::SeqCst) > 0 {
            eprintln!("received a second interrupt, stopping immediately...");
            if let Ok(mut guard) = child.lock() {
                if let Some(mut child) = guard.take() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
            if !keep_intermediates {
                crate::typst::paths::remove_entry_files(&layout);
            }
            std::process::exit(130);
        }
    })
    .context("failed to set signal handler")?;
    Ok(stop)
}

struct WatchPreprocessPaths<'a> {
    root: &'a Path,
    excluded_output: &'a Path,
    artifact_root: &'a Path,
}

fn describe_panic(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic".to_string()
    }
}

fn watch_preprocess_changes(
    args: &WatchArgs,
    paths: WatchPreprocessPaths<'_>,
    stop: Arc<AtomicBool>,
    sync_pages: bool,
    action: &'static str,
    site_context: Option<Arc<RwLock<SiteContextInput>>>,
) -> Result<()> {
    let options = preprocess_options(args, sync_pages);
    let quiet = args.common.quiet;
    // A panic inside `on_change` (for example a store shape `expect()`
    // failing) must not silently end the watcher thread while `typst watch`
    // keeps running with no one re-evaluating chunks. Catch it, record it,
    // and stop the debounced loop so the caller can surface it as an error.
    let stop_on_panic = Arc::clone(&stop);
    let panic_message: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let panic_message_for_closure = Arc::clone(&panic_message);
    watcher::watch_root(
        paths.root,
        paths.excluded_output,
        paths.artifact_root,
        args.common.config.as_deref(),
        stop,
        move |changed| {
            let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
                // Block HTML postprocessing across the rebuild so the newly
                // published wrapper and its completed store become visible
                // together.
                let mut site_context_guard = site_context
                    .as_ref()
                    .and_then(|context| context.write().ok());
                match prepare_preprocess_plan(options.clone()) {
                    Ok(plan) => {
                        if !quiet {
                            let names = changed
                                .iter()
                                .filter_map(|path| path.file_name())
                                .map(|name| name.to_string_lossy().to_string())
                                .collect::<Vec<_>>()
                                .join(", ");
                            eprintln!("{action} {names}...");
                        }
                        match preprocess_cached_plan(plan) {
                            Ok(output) => {
                                if let Some(context) = site_context_guard.as_deref_mut() {
                                    context.store = output.store.clone();
                                }
                                drop(site_context_guard);
                                if let Err(error) = publish_active_binding(&output.layout) {
                                    cwarn!("failed to publish active notebook: {}", error);
                                }
                            }
                            Err(error) => {
                                cwarn!("rebuild failed: {}", error);
                            }
                        }
                    }
                    Err(error) => {
                        cwarn!("rebuild failed: {}", error);
                    }
                }
            }));
            if let Err(payload) = outcome {
                let message = describe_panic(payload.as_ref());
                cwarn!("rebuild panicked: {}", message);
                if let Ok(mut slot) = panic_message_for_closure.lock() {
                    *slot = Some(message);
                }
                stop_on_panic.store(true, Ordering::Relaxed);
            }
        },
    )?;
    if let Some(message) = panic_message.lock().ok().and_then(|mut slot| slot.take()) {
        return Err(anyhow::anyhow!("watch rebuild panicked: {message}"));
    }
    Ok(())
}

fn run_eval_only_watch(args: WatchArgs) -> Result<()> {
    let initial = preprocess_cached(preprocess_options(&args, false))?;
    publish_active_binding(&initial.layout)?;
    // `--eval-only` never spawns a `typst watch` child, so the signal
    // handler has nothing to kill on a second signal beyond the entry files.
    let stop = install_signal_handler(
        Arc::new(Mutex::new(None)),
        initial.layout.clone(),
        args.common.keep_intermediates,
    )?;

    if !args.common.quiet {
        eprintln!(
            "watching {} for computational changes...",
            initial.layout.input_rel.display()
        );
    }

    let result = watch_preprocess_changes(
        &args,
        WatchPreprocessPaths {
            root: &initial.layout.root,
            excluded_output: &initial.layout.results_path,
            artifact_root: &initial.layout.artifact_root(),
        },
        stop,
        false,
        "checking",
        None,
    );
    // Tinymist renders the document itself, so nothing outside this session
    // needs the generated entry files once the watcher stops.
    if !args.common.keep_intermediates {
        crate::typst::paths::remove_entry_files(&initial.layout);
    }
    result
}

pub fn run_watch(args: WatchArgs) -> Result<()> {
    if args.eval_only {
        return run_eval_only_watch(args);
    }

    let format = resolve_output_format(args.format.map(OutputFormat::from), args.output.as_deref());
    let sync_pages = format.unwrap_or(OutputFormat::Pdf) == OutputFormat::Pdf;
    validate_forwarded_typst_args(&args.typst_args, format)?;

    let initial = preprocess_cached(preprocess_options(&args, sync_pages))?;
    publish_active_binding(&initial.layout)?;

    // Installed before the child spawns so a signal during setup is still
    // caught; the cell starts empty and is filled in once `typst watch` is
    // running.
    let child_cell: WatchedChild = Arc::new(Mutex::new(None));
    let stop = install_signal_handler(
        Arc::clone(&child_cell),
        initial.layout.clone(),
        args.common.keep_intermediates,
    )?;

    let resolved_output = resolve_output_path(&initial.layout, args.output.as_deref(), format);
    let root = initial.layout.root.clone();
    let is_html = format == Some(OutputFormat::Html);
    let asset_server = if is_html {
        let server = assets::start(root.clone(), Arc::clone(&stop))?;
        if !args.common.quiet {
            eprintln!("serving Calepin assets at {}", server.base_url());
        }
        Some(server)
    } else {
        None
    };
    let mut html_postprocessor = None;
    let mut write_events = None;
    let html_site_context = is_html.then(|| {
        Arc::new(RwLock::new(SiteContextInput {
            store: initial.store.clone(),
            ..SiteContextInput::default()
        }))
    });
    if is_html {
        let html_entry =
            crate::theme::resolve_html_entry(&initial.theme, crate::theme::HtmlScope::Document)?;
        let (sender, receiver) = mpsc::channel();
        write_events = Some(sender);
        html_postprocessor = Some(start_html_output_postprocessor(
            Arc::clone(&stop),
            resolved_output.clone(),
            initial.layout.clone(),
            html_entry,
            html_site_context.clone(),
            receiver,
        ));
    }

    let watch_args = typst_watch_args(
        &initial.layout,
        args.output.as_deref(),
        format,
        &args.typst_args,
        ReservedInputs {
            asset_base: asset_server.as_ref().map(|server| server.base_url()),
            ..ReservedInputs::default()
        },
    )?;

    assert_supported_typst(&initial.executables.typst)?;
    process::validate_executable(
        &initial.executables.typst,
        "start typst watch",
        Some(&tools::TYPST),
    )?;
    let child = Command::new(&initial.executables.typst)
        .args(&watch_args)
        .current_dir(&root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            process::spawn_error(
                &initial.executables.typst,
                "start typst watch",
                error,
                Some(&tools::TYPST),
            )
        });
    let mut child = match child {
        Ok(child) => child,
        Err(error) => {
            stop.store(true, Ordering::Relaxed);
            if let Some(postprocessor) = html_postprocessor.take() {
                let _ = postprocessor.join();
            }
            if let Some(server) = asset_server {
                server.join();
            }
            return Err(error);
        }
    };
    let stdout = child
        .stdout
        .take()
        .context("failed to capture typst watch stdout")?;
    let stderr = child
        .stderr
        .take()
        .context("failed to capture typst watch stderr")?;
    // From here on the signal handler can also reach this child: a second
    // signal kills it directly and exits the process without waiting for
    // the poll loop below.
    *child_cell.lock().unwrap() = Some(child);
    let stdout_relay = if let Some(sender) = write_events.clone() {
        thread::spawn(move || relay_typst_watch_output_with_events(stdout, io::stdout(), sender))
    } else {
        thread::spawn(move || relay_typst_watch_output(stdout, io::stdout()))
    };
    let stderr_relay = if let Some(sender) = write_events {
        thread::spawn(move || relay_typst_watch_output_with_events(stderr, io::stderr(), sender))
    } else {
        thread::spawn(move || relay_typst_watch_output(stderr, io::stderr()))
    };

    let watcher_stop = Arc::clone(&stop);
    let watcher_args = args.clone();
    let watcher_root = root.clone();
    let watcher_output = resolved_output.clone();
    let watcher_artifact_root = initial.layout.artifact_root();
    let watcher = thread::spawn(move || -> Result<()> {
        let result = watch_preprocess_changes(
            &watcher_args,
            WatchPreprocessPaths {
                root: &watcher_root,
                excluded_output: &watcher_output,
                artifact_root: &watcher_artifact_root,
            },
            Arc::clone(&watcher_stop),
            sync_pages,
            "rebuilding",
            html_site_context,
        );
        if let Err(error) = &result {
            cwarn!("watch error: {}", error);
            // A dead watcher thread means edits stop re-evaluating even
            // though `typst watch` keeps running; treat that as a stop
            // rather than leaving the process hung with no visible signal.
            watcher_stop.store(true, Ordering::Relaxed);
        }
        result
    });

    let child_outcome = loop {
        if stop.load(Ordering::Relaxed) {
            break WatchChildOutcome::StopRequested;
        }
        let poll = {
            let mut guard = child_cell.lock().unwrap();
            match guard.as_mut() {
                Some(child) => child.try_wait(),
                // Taken by the signal handler's forced-shutdown path; the
                // process is exiting regardless of what we do here.
                None => break WatchChildOutcome::StopRequested,
            }
        };
        match poll {
            Ok(Some(status)) => break WatchChildOutcome::Exited(status),
            Ok(None) => thread::sleep(Duration::from_millis(200)),
            Err(error) => break WatchChildOutcome::PollFailed(error),
        }
    };

    stop.store(true, Ordering::Relaxed);
    if !matches!(&child_outcome, WatchChildOutcome::Exited(_)) {
        if let Ok(mut guard) = child_cell.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    join_relay("stdout", stdout_relay);
    join_relay("stderr", stderr_relay);
    let watcher_result = watcher.join();
    if let Some(postprocessor) = html_postprocessor.take() {
        let _ = postprocessor.join();
    }
    if let Some(server) = asset_server {
        server.join();
    }
    // The generated entry files are only needed while `typst watch` is running.
    if !args.common.keep_intermediates {
        crate::typst::paths::remove_entry_files(&initial.layout);
    }

    match watcher_result {
        Ok(Ok(())) => child_outcome.into_result(),
        Ok(Err(error)) => Err(error),
        Err(_) => Err(anyhow::anyhow!("watch thread panicked")),
    }
}

enum WatchChildOutcome {
    StopRequested,
    Exited(ExitStatus),
    PollFailed(io::Error),
}

impl WatchChildOutcome {
    fn into_result(self) -> Result<()> {
        match self {
            Self::StopRequested => Ok(()),
            Self::Exited(status) if status.success() => Ok(()),
            Self::Exited(status) => Err(anyhow::anyhow!("typst watch exited with {status}")),
            Self::PollFailed(error) => Err(error).context("failed to poll typst watch"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requested_watch_stop_is_successful() {
        assert!(WatchChildOutcome::StopRequested.into_result().is_ok());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn failed_typst_watch_exit_is_reported() {
        #[cfg(unix)]
        use std::os::unix::process::ExitStatusExt;
        #[cfg(windows)]
        use std::os::windows::process::ExitStatusExt;

        #[cfg(unix)]
        let status = ExitStatus::from_raw(23 << 8);
        #[cfg(windows)]
        let status = ExitStatus::from_raw(23);

        let error = WatchChildOutcome::Exited(status)
            .into_result()
            .unwrap_err()
            .to_string();

        assert!(error.contains("typst watch exited with"), "{error}");
    }
}
