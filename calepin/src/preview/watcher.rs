use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use notify::{RecursiveMode, Watcher};

/// Watch a file for modifications. Calls `on_change` when the file is modified.
/// Blocks until Ctrl+C is pressed or the watcher errors.
pub fn watch(path: &Path, stop: Arc<AtomicBool>, on_change: impl Fn()) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(tx)
        .context("Failed to create file watcher")?;

    let watch_dir = path.parent().unwrap_or(path);
    watcher.watch(watch_dir, RecursiveMode::NonRecursive)
        .with_context(|| format!("Failed to watch {}", watch_dir.display()))?;

    let watch_path = path.to_path_buf();

    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(event)) => {
                let dominated = event.paths.iter().any(|p| p == &watch_path);
                if dominated && matches!(event.kind, notify::EventKind::Modify(_) | notify::EventKind::Create(_)) {
                    // Debounce: drain events for 100ms
                    std::thread::sleep(Duration::from_millis(100));
                    while rx.try_recv().is_ok() {}
                    on_change();
                }
            }
            Ok(Err(e)) => {
                cwarn!("Watch error: {}", e);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}

/// Watch a directory recursively for `.qmd` file modifications.
/// Calls `on_change` with the paths of changed files.
/// Collects all changed paths during the debounce window.
/// Blocks until Ctrl+C is pressed or the watcher errors.
pub fn watch_dir(dir: &Path, stop: Arc<AtomicBool>, on_change: impl Fn(&[std::path::PathBuf])) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(tx)
        .context("Failed to create file watcher")?;

    let canon_dir = dir.canonicalize()
        .with_context(|| format!("Watch directory not found: {}", dir.display()))?;
    watcher.watch(&canon_dir, RecursiveMode::Recursive)
        .with_context(|| format!("Failed to watch {}", canon_dir.display()))?;

    fn is_qmd(p: &Path) -> bool {
        p.extension().and_then(|e| e.to_str()) == Some("qmd")
    }

    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(event)) => {
                // Only react to content modifications and file creation.
                // macOS FSEvents may report generic Modify without a sub-kind,
                // so accept Modify(Any) and Modify(Data) but not Modify(Metadata).
                let dominated = matches!(
                    event.kind,
                    notify::EventKind::Modify(notify::event::ModifyKind::Data(_))
                    | notify::EventKind::Modify(notify::event::ModifyKind::Any)
                    | notify::EventKind::Create(_)
                );
                if !dominated {
                    continue;
                }
                let qmd_paths: Vec<std::path::PathBuf> = event.paths.iter()
                    .filter(|p| is_qmd(p))
                    .cloned()
                    .collect();
                if !qmd_paths.is_empty() {
                    // Debounce: drain events for 100ms, collecting more changed paths
                    std::thread::sleep(Duration::from_millis(100));
                    let mut all_paths = qmd_paths;
                    while let Ok(Ok(extra)) = rx.try_recv() {
                        for p in &extra.paths {
                            if is_qmd(p) && !all_paths.contains(p) {
                                all_paths.push(p.clone());
                            }
                        }
                    }
                    on_change(&all_paths);
                }
            }
            Ok(Err(e)) => {
                cwarn!("Watch error: {}", e);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}

