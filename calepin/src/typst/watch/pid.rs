use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};

use crate::typst::io::write_if_changed;

const WATCH_PID_FILENAME: &str = "watch.pid";

pub(super) fn stop_watch_from_pid_file(path: &Path) -> Result<bool> {
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

pub(super) fn write_watch_pid_file(path: &Path, pid: u32) -> Result<()> {
    write_if_changed(path, pid.to_string())
        .with_context(|| format!("failed to write watch pid file {}", path.display()))
}

pub(super) fn remove_watch_pid_file(path: &Path) -> Result<()> {
    fs::remove_file(path).context("failed to remove watch pid file")?;
    Ok(())
}

pub(super) fn watch_pid_file_path(results_path: &Path) -> PathBuf {
    let parent = results_path
        .parent()
        .expect("results path should have a parent directory");
    parent.join(WATCH_PID_FILENAME)
}

pub(super) fn collect_watch_pid_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
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
