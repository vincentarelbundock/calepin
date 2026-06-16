mod assets;
mod pid;
mod relay;
mod watcher;

use std::io;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

use crate::cli::{StopArgs, WatchArgs};
use crate::typst::compile::{
    reject_reserved_typst_inputs, resolve_output_path, typst_watch_args, ReservedInputs,
};
use crate::typst::paths::resolve_layout;
use crate::typst::preprocess::{
    execute_preprocess_plan, prepare_preprocess_plan, preprocess, PreprocessOptions,
};
use crate::typst::version::assert_supported_typst;
use crate::utils::{process, tools};

use pid::{
    collect_watch_pid_files, remove_watch_pid_file, stop_watch_from_pid_file, watch_pid_file_path,
    write_watch_pid_file,
};
use relay::{join_relay, relay_typst_watch_output};

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
        param_overrides: args.common.params.clone(),
    }
}

pub fn run_watch(args: WatchArgs) -> Result<()> {
    let format = args.format.map(|format| format.as_str().to_string());
    let sync_pages = format.as_deref().unwrap_or("pdf") == "pdf";
    reject_reserved_typst_inputs(&args.typst_args)?;

    let initial = preprocess(preprocess_options(&args, sync_pages))?;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_for_handler = Arc::clone(&stop);
    ctrlc::set_handler(move || {
        stop_for_handler.store(true, Ordering::Relaxed);
    })
    .context("failed to set Ctrl+C handler")?;

    let resolved_output =
        resolve_output_path(&initial.layout, args.output.as_deref(), format.as_deref());
    let root = initial.layout.root.clone();
    let asset_server = if format.as_deref() == Some("html") {
        let server = assets::start(root.clone(), Arc::clone(&stop))?;
        if !args.common.quiet {
            eprintln!("serving Calepin assets at {}", server.base_url());
        }
        Some(server)
    } else {
        None
    };

    let watch_args = typst_watch_args(
        &initial.layout,
        args.output.as_deref(),
        format.as_deref(),
        &args.typst_args,
        ReservedInputs {
            asset_base: asset_server.as_ref().map(|server| server.base_url()),
            ..ReservedInputs::default()
        },
    );

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
            if let Some(server) = asset_server {
                server.join();
            }
            return Err(error);
        }
    };
    let watch_pid_path = watch_pid_file_path(&initial.layout.results_path);
    if let Err(error) = write_watch_pid_file(&watch_pid_path, child.id()) {
        cwarn!(
            "failed to write watch pid file {}: {}",
            watch_pid_path.display(),
            error
        );
    }
    let stdout = child
        .stdout
        .take()
        .context("failed to capture typst watch stdout")?;
    let stderr = child
        .stderr
        .take()
        .context("failed to capture typst watch stderr")?;
    let stdout_relay = thread::spawn(move || relay_typst_watch_output(stdout, io::stdout()));
    let stderr_relay = thread::spawn(move || relay_typst_watch_output(stderr, io::stderr()));

    let watcher_stop = Arc::clone(&stop);
    let watcher_args = args.clone();
    let watcher_root = root.clone();
    let watcher_output = resolved_output.clone();
    let quiet = args.common.quiet;
    let watcher = thread::spawn(move || {
        let options = preprocess_options(&watcher_args, sync_pages);
        let mut last_fingerprint = initial.fingerprint;
        let result = watcher::watch_root(
            &watcher_root,
            &watcher_output,
            watcher_args.common.config.as_deref(),
            Arc::clone(&watcher_stop),
            move |changed| match prepare_preprocess_plan(options.clone()) {
                Ok(plan) => {
                    if plan.fingerprint == last_fingerprint && plan.layout.results_path.exists() {
                        return;
                    }
                    if !quiet {
                        let names = changed
                            .iter()
                            .filter_map(|path| path.file_name())
                            .map(|name| name.to_string_lossy().to_string())
                            .collect::<Vec<_>>()
                            .join(", ");
                        eprintln!("rebuilding {names}...");
                    }
                    match execute_preprocess_plan(plan) {
                        Ok(output) => {
                            last_fingerprint = output.fingerprint;
                        }
                        Err(error) => {
                            cwarn!("rebuild failed: {}", error);
                        }
                    }
                }
                Err(error) => {
                    cwarn!("rebuild failed: {}", error);
                }
            },
        );
        if let Err(error) = result {
            cwarn!("watch error: {}", error);
        }
    });

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match child.try_wait() {
            Ok(Some(_status)) => break,
            Ok(None) => thread::sleep(Duration::from_millis(200)),
            Err(error) => {
                cwarn!("failed to poll typst watch: {}", error);
                break;
            }
        }
    }

    stop.store(true, Ordering::Relaxed);
    let _ = child.kill();
    let _ = child.wait();
    let _ = remove_watch_pid_file(&watch_pid_path);
    join_relay("stdout", stdout_relay);
    join_relay("stderr", stderr_relay);
    let _ = watcher.join();
    if let Some(server) = asset_server {
        server.join();
    }
    Ok(())
}

pub fn run_stop(args: StopArgs) -> Result<()> {
    match args.input {
        Some(input) => {
            let layout = resolve_layout(&input, None).with_context(|| {
                format!("failed to resolve watch context from {}", input.display())
            })?;
            let watch_pid_path = watch_pid_file_path(&layout.results_path);
            stop_watch_from_pid_file(&watch_pid_path)?;
        }
        None => {
            let calepin_dir = std::env::current_dir()?.join(".calepin");
            let mut watch_pid_paths = Vec::new();
            collect_watch_pid_files(&calepin_dir, &mut watch_pid_paths)?;
            if watch_pid_paths.is_empty() {
                return Err(anyhow!("no running calepin watch found"));
            }
            for path in watch_pid_paths {
                stop_watch_from_pid_file(&path)?;
            }
        }
    }
    Ok(())
}
