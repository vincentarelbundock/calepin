use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use notify::RecursiveMode;
use notify_debouncer_full::new_debouncer;

fn is_watched(p: &Path) -> bool {
    if p.extension().and_then(|e| e.to_str()) == Some("qmd") {
        return true;
    }
    let s = p.to_string_lossy();
    s.contains("/_calepin/partials/") || s.ends_with("/_calepin/config.toml")
}

/// Returns true for events that indicate file content changed.
fn is_write_event(kind: notify::EventKind) -> bool {
    matches!(kind,
        notify::EventKind::Create(_)
        | notify::EventKind::Modify(notify::event::ModifyKind::Data(_))
        | notify::EventKind::Modify(notify::event::ModifyKind::Name(_))
        | notify::EventKind::Modify(notify::event::ModifyKind::Any)
    )
}

/// Watch a single file for modifications. Calls `on_change` when the file changes.
pub fn watch(path: &Path, stop: Arc<AtomicBool>, on_change: impl Fn()) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let mut debouncer = new_debouncer(Duration::from_millis(300), None, tx)
        .context("Failed to create file watcher")?;

    let watch_dir = path.parent().unwrap_or(path);
    debouncer.watch(watch_dir, RecursiveMode::NonRecursive)
        .with_context(|| format!("Failed to watch {}", watch_dir.display()))?;

    let watch_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(events)) => {
                let dominated = events.iter().any(|de| {
                    is_write_event(de.event.kind)
                        && de.event.paths.iter().any(|p| {
                            p.canonicalize().unwrap_or_else(|_| p.clone()) == watch_path
                        })
                });
                if dominated {
                    on_change();
                }
            }
            Ok(Err(errs)) => {
                for e in errs {
                    cwarn!("Watch error: {}", e);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if stop.load(Ordering::Relaxed) { break; }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(())
}

/// Watch a directory recursively for .qmd file writes.
/// Debounces events with a 300ms window. Excludes the output directory.
pub fn watch_dir(dir: &Path, stop: Arc<AtomicBool>, exclude_dir: Option<&Path>, on_change: impl Fn(&[std::path::PathBuf])) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let mut debouncer = new_debouncer(Duration::from_millis(300), None, tx)
        .context("Failed to create file watcher")?;

    let canon_dir = dir.canonicalize()
        .with_context(|| format!("Watch directory not found: {}", dir.display()))?;
    debouncer.watch(&canon_dir, RecursiveMode::Recursive)
        .with_context(|| format!("Failed to watch {}", canon_dir.display()))?;

    let canon_exclude = exclude_dir.and_then(|p| p.canonicalize().ok());

    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(events)) => {
                let mut changed = Vec::new();
                for de in events {
                    if !is_write_event(de.event.kind) { continue; }
                    for p in &de.event.paths {
                        if !is_watched(p) { continue; }
                        let cp = p.canonicalize().unwrap_or_else(|_| p.clone());
                        if canon_exclude.as_ref().map_or(false, |ex| cp.starts_with(ex)) { continue; }
                        if !changed.contains(&cp) {
                            changed.push(cp);
                        }
                    }
                }
                if !changed.is_empty() {
                    on_change(&changed);
                }
            }
            Ok(Err(errs)) => {
                for e in errs {
                    cwarn!("Watch error: {}", e);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if stop.load(Ordering::Relaxed) { break; }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(())
}
