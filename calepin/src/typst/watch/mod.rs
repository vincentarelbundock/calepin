mod assets;
mod watcher;

use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

use crate::cli::{StopArgs, WatchArgs};
use crate::html::prepare_html_theme;
use crate::typst::compile::{reject_reserved_typst_inputs, resolve_output_path, typst_watch_args};
use crate::typst::paths::resolve_layout;
use crate::typst::preprocess::{
    execute_preprocess_plan, prepare_preprocess_plan, preprocess, PreprocessOptions,
};
use crate::typst::version::assert_supported_typst;
use crate::utils::{process, tools};

const WATCH_PID_FILENAME: &str = "watch.pid";

fn preprocess_options(args: &WatchArgs, sync_pages: bool) -> PreprocessOptions {
    PreprocessOptions {
        input: args.input.clone(),
        config: args.common.config.clone(),
        quiet: args.common.quiet,
        timeout: args.common.timeout,
        sync_pages,
        param_overrides: args.common.params.clone(),
    }
}

pub fn run_watch(args: WatchArgs) -> Result<()> {
    let format = args.format.map(|format| format.as_str().to_string());
    let sync_pages = format.as_deref().unwrap_or("pdf") == "pdf";
    reject_reserved_typst_inputs(&args.typst_args)?;

    let initial = preprocess(preprocess_options(&args, sync_pages))?;
    let prepared_theme =
        prepare_html_theme(&initial.layout.root, format.as_deref(), None, None, None)?;

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
        prepared_theme.raw_theme_input.as_deref(),
        asset_server.as_ref().map(|server| server.base_url()),
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
    let stdout_relay = thread::spawn(move || relay_stdout(stdout));
    let stderr_relay = thread::spawn(move || relay_stderr(stderr));

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

fn stop_watch_from_pid_file(path: &Path) -> Result<bool> {
    let pid = read_watch_pid(path)?;
    let stopped = if is_process_running(pid) {
        terminate_process(pid)?;
        true
    } else {
        false
    };
    let _ = fs::remove_file(path);
    Ok(stopped)
}

fn read_watch_pid(path: &Path) -> Result<u32> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read watch pid file {}", path.display()))?;
    contents
        .trim()
        .parse::<u32>()
        .with_context(|| format!("invalid watch pid in {}", path.display()))
}

fn write_watch_pid_file(path: &Path, pid: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, pid.to_string())
        .with_context(|| format!("failed to write watch pid file {}", path.display()))
}

fn remove_watch_pid_file(path: &Path) -> Result<()> {
    fs::remove_file(path).context("failed to remove watch pid file")?;
    Ok(())
}

fn watch_pid_file_path(results_path: &Path) -> PathBuf {
    let parent = results_path
        .parent()
        .expect("results path should have a parent directory");
    parent.join(WATCH_PID_FILENAME)
}

fn collect_watch_pid_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let entry_path = entry.path();
        if file_type.is_dir() {
            collect_watch_pid_files(&entry_path, out)?;
        } else if file_type.is_file() && entry.file_name() == WATCH_PID_FILENAME {
            out.push(entry_path);
        }
    }
    Ok(())
}

#[cfg(unix)]
fn is_process_running(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(unix)]
fn terminate_process(pid: u32) -> Result<()> {
    let status = Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status()
        .with_context(|| format!("failed to signal watch process {pid}"))?;
    if !status.success() {
        return Err(anyhow!("failed to signal watch process {pid}"));
    }
    Ok(())
}

#[cfg(windows)]
fn is_process_running(_pid: u32) -> bool {
    true
}

#[cfg(windows)]
fn terminate_process(pid: u32) -> Result<()> {
    let status = Command::new("taskkill")
        .arg("/PID")
        .arg(pid.to_string())
        .arg("/F")
        .status()
        .with_context(|| format!("failed to signal watch process {pid}"))?;
    if !status.success() {
        return Err(anyhow!("failed to signal watch process {pid}"));
    }
    Ok(())
}

fn relay_stdout<R>(mut reader: R) -> io::Result<()>
where
    R: Read,
{
    relay_typst_watch_output(&mut reader, io::stdout())
}

fn relay_stderr<R>(mut reader: R) -> io::Result<()>
where
    R: Read,
{
    relay_typst_watch_output(&mut reader, io::stderr())
}

fn relay_typst_watch_output<R, W>(reader: R, writer: W) -> io::Result<()>
where
    R: Read,
    W: Write,
{
    let mut reader = BufReader::new(reader);
    let mut relay = TypstWatchOutputRelay::new(writer);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            break;
        }
        relay.write_line(&line)?;
    }

    relay.finish()
}

struct TypstWatchOutputRelay<W> {
    writer: W,
    seen_watching: bool,
    seen_writing: bool,
    pending_blank: bool,
}

impl<W: Write> TypstWatchOutputRelay<W> {
    fn new(writer: W) -> Self {
        Self {
            writer,
            seen_watching: false,
            seen_writing: false,
            pending_blank: false,
        }
    }

    fn write_line(&mut self, line: &str) -> io::Result<()> {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            self.pending_blank = true;
            return Ok(());
        }

        let should_write = match typst_watch_line(line) {
            TypstWatchLine::Watching => {
                let first = !self.seen_watching;
                self.seen_watching = true;
                first
            }
            TypstWatchLine::Writing => {
                let first = !self.seen_writing;
                self.seen_writing = true;
                first
            }
            TypstWatchLine::Status => true,
            TypstWatchLine::Other => {
                if self.pending_blank {
                    writeln!(self.writer)?;
                }
                true
            }
        };
        self.pending_blank = false;

        if should_write {
            writeln!(self.writer, "{line}")?;
        }
        Ok(())
    }

    fn finish(mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypstWatchLine {
    Watching,
    Writing,
    Status,
    Other,
}

fn typst_watch_line(line: &str) -> TypstWatchLine {
    if line.starts_with("watching ") {
        TypstWatchLine::Watching
    } else if line.starts_with("writing to ") {
        TypstWatchLine::Writing
    } else if line.starts_with('[') && line.contains("] ") {
        TypstWatchLine::Status
    } else {
        TypstWatchLine::Other
    }
}

fn join_relay(name: &str, handle: thread::JoinHandle<io::Result<()>>) {
    match handle.join() {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            cwarn!("failed to relay typst watch {name}: {}", error);
        }
        Err(_) => {
            cwarn!("typst watch {name} relay panicked");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_typst_watch_output_compacts_status_lines() {
        let input = "\
watching example.typ
writing to example.pdf

[10:17:43] compiling ...

watching example.typ
writing to example.pdf

[10:17:43] compiled successfully in 88.22 ms

watching example.typ
writing to example.pdf

[10:17:53] compiling ...
";
        let mut output = Vec::new();

        relay_typst_watch_output(input.as_bytes(), &mut output).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "\
watching example.typ
writing to example.pdf
[10:17:43] compiling ...
[10:17:43] compiled successfully in 88.22 ms
[10:17:53] compiling ...
"
        );
    }

    #[test]
    fn relay_typst_watch_output_preserves_diagnostic_spacing() {
        let input = "\
error: failed

hint: check this
";
        let mut output = Vec::new();

        relay_typst_watch_output(input.as_bytes(), &mut output).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "\
error: failed

hint: check this
"
        );
    }
}
