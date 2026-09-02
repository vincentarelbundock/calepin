use std::collections::HashSet;
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
                let mut unique = HashSet::new();
                for event in events {
                    if !event_filter(&event.event.kind) {
                        continue;
                    }
                    for path in event.event.paths {
                        let path = path.canonicalize().unwrap_or(path);
                        if let Some(path) = path_filter(path) {
                            if unique.insert(path.clone()) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Mutex;
    use std::thread;

    type RecordedChanges = Arc<Mutex<Vec<Vec<PathBuf>>>>;

    /// Watches `dir` on a background thread, recording every change batch.
    /// Callers should give the watcher a moment to attach before generating
    /// filesystem events, then `stop` it and `join` before asserting.
    fn spawn_watch(
        dir: &Path,
        debounce: Duration,
    ) -> (
        Arc<AtomicBool>,
        RecordedChanges,
        thread::JoinHandle<Result<()>>,
    ) {
        let stop = Arc::new(AtomicBool::new(false));
        let calls: Arc<Mutex<Vec<Vec<PathBuf>>>> = Arc::new(Mutex::new(Vec::new()));
        let calls_for_thread = Arc::clone(&calls);
        let stop_for_thread = Arc::clone(&stop);
        let dir = dir.to_path_buf();
        let handle = thread::spawn(move || {
            run_debounced_watch(
                &[(dir, RecursiveMode::Recursive)],
                debounce,
                Duration::from_millis(50),
                stop_for_thread,
                is_write_event,
                Some,
                move |changed| {
                    calls_for_thread.lock().unwrap().push(changed.to_vec());
                },
            )
        });
        (stop, calls, handle)
    }

    #[test]
    fn rapid_writes_to_one_file_are_debounced_into_fewer_changes_than_writes() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "0").unwrap();

        let (stop, calls, handle) = spawn_watch(dir.path(), Duration::from_millis(300));
        // Let the watcher attach before generating events it should see.
        thread::sleep(Duration::from_millis(300));

        const WRITE_COUNT: usize = 10;
        for i in 0..WRITE_COUNT {
            std::fs::write(&file, i.to_string()).unwrap();
            thread::sleep(Duration::from_millis(5));
        }
        // All writes above land well inside the 300ms debounce window; wait
        // past it for the coalesced batch (or batches) to arrive.
        thread::sleep(Duration::from_millis(600));

        stop.store(true, Ordering::Relaxed);
        handle.join().unwrap().unwrap();

        let calls = calls.lock().unwrap();
        assert!(!calls.is_empty(), "expected at least one change batch");
        // Debouncing does not guarantee a single flush (the debouncer's own
        // tick rate can split a long burst), but it must coalesce well
        // below one batch per write, and every batch it does emit reports
        // the file only once (`unique.insert` in `run_debounced_watch_until`).
        assert!(
            calls.len() < WRITE_COUNT,
            "expected debouncing to coalesce {WRITE_COUNT} rapid writes into \
             fewer change batches, got {:?}",
            *calls
        );
        for batch in calls.iter() {
            assert_eq!(batch, &vec![file.canonicalize().unwrap()]);
        }
    }

    #[test]
    fn a_change_batch_never_repeats_a_path() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("b.txt");
        std::fs::write(&file, "0").unwrap();

        let (stop, calls, handle) = spawn_watch(dir.path(), Duration::from_millis(150));
        thread::sleep(Duration::from_millis(300));

        // Several rapid writes to the same file typically surface as more
        // than one raw notify event (create, then data-change modifies);
        // the batch handed to `on_change` must still list the path once.
        for i in 0..3 {
            std::fs::write(&file, i.to_string()).unwrap();
        }
        thread::sleep(Duration::from_millis(500));

        stop.store(true, Ordering::Relaxed);
        handle.join().unwrap().unwrap();

        let calls = calls.lock().unwrap();
        assert!(!calls.is_empty(), "expected at least one change batch");
        for batch in calls.iter() {
            let unique: std::collections::HashSet<_> = batch.iter().collect();
            assert_eq!(
                unique.len(),
                batch.len(),
                "change batch repeated a path: {:?}",
                batch
            );
        }
    }

    #[test]
    fn writes_to_different_files_are_seen_and_never_repeated_within_a_batch() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("c.txt");
        let second = dir.path().join("d.txt");
        std::fs::write(&first, "0").unwrap();
        std::fs::write(&second, "0").unwrap();

        let (stop, calls, handle) = spawn_watch(dir.path(), Duration::from_millis(300));
        thread::sleep(Duration::from_millis(300));

        std::fs::write(&first, "1").unwrap();
        std::fs::write(&second, "1").unwrap();
        std::fs::write(&first, "2").unwrap();
        thread::sleep(Duration::from_millis(600));

        stop.store(true, Ordering::Relaxed);
        handle.join().unwrap().unwrap();

        let calls = calls.lock().unwrap();
        assert!(!calls.is_empty(), "expected at least one change batch");

        // Under system load the two files' events can land in separate
        // flushes rather than one coalesced batch, so this does not pin an
        // exact batch count. What must hold regardless: no batch ever
        // reports the same path twice (the `unique` dedup in
        // `run_debounced_watch_until`), and both files are eventually seen.
        let mut seen = std::collections::HashSet::new();
        for batch in calls.iter() {
            let unique: std::collections::HashSet<_> = batch.iter().collect();
            assert_eq!(
                unique.len(),
                batch.len(),
                "batch reported a path more than once: {:?}",
                batch
            );
            seen.extend(batch.iter().cloned());
        }
        assert_eq!(
            seen,
            [
                first.canonicalize().unwrap(),
                second.canonicalize().unwrap()
            ]
            .into_iter()
            .collect::<std::collections::HashSet<_>>(),
        );
    }
}
