mod assets;
mod relay;
mod watcher;

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, RwLock};
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

fn install_ctrl_c_handler() -> Result<Arc<AtomicBool>> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_handler = Arc::clone(&stop);
    ctrlc::set_handler(move || {
        stop_for_handler.store(true, Ordering::Relaxed);
    })
    .context("failed to set Ctrl+C handler")?;
    Ok(stop)
}

struct WatchPreprocessPaths<'a> {
    root: &'a Path,
    excluded_output: &'a Path,
    artifact_root: &'a Path,
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
    watcher::watch_root(
        paths.root,
        paths.excluded_output,
        paths.artifact_root,
        args.common.config.as_deref(),
        stop,
        move |changed| {
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
        },
    )
}

fn run_eval_only_watch(args: WatchArgs) -> Result<()> {
    let initial = preprocess_cached(preprocess_options(&args, false))?;
    publish_active_binding(&initial.layout)?;
    let stop = install_ctrl_c_handler()?;

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

    let stop = install_ctrl_c_handler()?;

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
    let watcher = thread::spawn(move || {
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
        if let Err(error) = result {
            cwarn!("watch error: {}", error);
        }
    });

    let child_outcome = loop {
        if stop.load(Ordering::Relaxed) {
            break WatchChildOutcome::StopRequested;
        }
        match child.try_wait() {
            Ok(Some(status)) => break WatchChildOutcome::Exited(status),
            Ok(None) => thread::sleep(Duration::from_millis(200)),
            Err(error) => break WatchChildOutcome::PollFailed(error),
        }
    };

    stop.store(true, Ordering::Relaxed);
    if !matches!(&child_outcome, WatchChildOutcome::Exited(_)) {
        let _ = child.kill();
        let _ = child.wait();
    }
    join_relay("stdout", stdout_relay);
    join_relay("stderr", stderr_relay);
    let _ = watcher.join();
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
    child_outcome.into_result()
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
