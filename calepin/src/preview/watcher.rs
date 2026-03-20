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

    let canon = path.to_path_buf();

    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(event)) => {
                let dominated = event.paths.iter().any(|p| p == &canon);
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

