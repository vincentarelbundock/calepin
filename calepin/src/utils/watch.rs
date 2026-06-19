use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use notify::RecursiveMode;
use notify_debouncer_full::new_debouncer;

pub fn is_write_event(kind: &notify::EventKind) -> bool {
    matches!(
        kind,
        notify::EventKind::Create(_)
            | notify::EventKind::Modify(notify::event::ModifyKind::Data(_))
            | notify::EventKind::Modify(notify::event::ModifyKind::Name(_))
            | notify::EventKind::Modify(notify::event::ModifyKind::Any)
    )
}

pub fn is_rebuild_event(kind: &notify::EventKind) -> bool {
    is_write_event(kind) || matches!(kind, notify::EventKind::Remove(_))
}

pub fn run_debounced_watch(
    watches: &[(PathBuf, RecursiveMode)],
    debounce: Duration,
    poll: Duration,
    stop: Arc<AtomicBool>,
    event_filter: impl FnMut(&notify::EventKind) -> bool,
    path_filter: impl FnMut(PathBuf) -> Option<PathBuf>,
    mut on_change: impl FnMut(&[PathBuf]),
) -> Result<()> {
    run_debounced_watch_until(
        watches,
        debounce,
        poll,
        stop,
        event_filter,
        path_filter,
        |changed| {
            on_change(changed);
            true
        },
    )
}

pub fn run_debounced_watch_until(
    watches: &[(PathBuf, RecursiveMode)],
    debounce: Duration,
    poll: Duration,
    stop: Arc<AtomicBool>,
    mut event_filter: impl FnMut(&notify::EventKind) -> bool,
    mut path_filter: impl FnMut(PathBuf) -> Option<PathBuf>,
    mut on_change: impl FnMut(&[PathBuf]) -> bool,
) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let mut debouncer =
        new_debouncer(debounce, None, tx).context("failed to create file watcher")?;

    for (path, mode) in watches {
        debouncer
            .watch(path, *mode)
            .with_context(|| format!("failed to watch {}", path.display()))?;
    }

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match rx.recv_timeout(poll) {
            Ok(Ok(events)) => {
                let mut changed = Vec::new();
                for event in events {
                    if !event_filter(&event.event.kind) {
                        continue;
                    }
                    for path in event.event.paths {
                        let path = path.canonicalize().unwrap_or(path);
                        if let Some(path) = path_filter(path) {
                            if !changed.contains(&path) {
                                changed.push(path);
                            }
                        }
                    }
                }
                if !changed.is_empty() && !on_change(&changed) {
                    break;
                }
            }
            Ok(Err(errors)) => {
                for error in errors {
                    cwarn!("watch error: {}", error);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}
