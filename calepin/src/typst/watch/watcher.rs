use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use notify::RecursiveMode;
use notify_debouncer_full::new_debouncer;


pub(crate) fn is_write_event(kind: &notify::EventKind) -> bool {
    matches!(
        kind,
        notify::EventKind::Create(_)
            | notify::EventKind::Modify(notify::event::ModifyKind::Data(_))
            | notify::EventKind::Modify(notify::event::ModifyKind::Name(_))
            | notify::EventKind::Modify(notify::event::ModifyKind::Any)
    )
}

pub(crate) fn is_watch_candidate(
    root: &Path,
    preview_output: &Path,
    config_path: Option<&Path>,
    path: &Path,
) -> bool {
    if path == preview_output
        || is_typst_temporary_output(preview_output, path)
        || is_editor_backup(path)
    {
        return false;
    }

    let rel = path.strip_prefix(root).unwrap_or(path);
    
    if let Some(config_path) = config_path {
        if path == config_path {
            return true;
        }
    }

    let Some(first) = rel.components().next() else {
        return false;
    };
    let Component::Normal(name) = first else {
        return true;
    };

    if matches!(
        name.to_str(),
        Some(".calepin" | ".git" | "target" | "node_modules" | ".venv")
    ) {
        return false;
    }

    if rel.starts_with("editors/vscode/bin")
        || rel.starts_with("editors/vscode/dist")
        || rel.starts_with("editors/vscode/media")
        || rel.starts_with("editors/vscode/node_modules")
        || rel.starts_with("editors/vscode/out")
    {
        return false;
    }

    true
}

fn is_typst_temporary_output(preview_output: &Path, path: &Path) -> bool {
    if path.parent() != preview_output.parent() {
        return false;
    }

    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    path.extension().is_none() && name.starts_with("XX")
}

fn is_editor_backup(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with('~'))
}

pub(crate) fn watch_root(
    root: &Path,
    preview_output: &Path,
    config_path: Option<&Path>,
    stop: Arc<AtomicBool>,
    mut on_change: impl FnMut(&[PathBuf]),
) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let mut debouncer = new_debouncer(Duration::from_millis(300), None, tx)
        .context("failed to create file watcher")?;

    let root = root
        .canonicalize()
        .with_context(|| format!("watch root not found: {}", root.display()))?;
    let preview_output = preview_output.to_path_buf();
    let config_path = config_path.map(|path| {
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.join(path)
        };
        path.canonicalize().unwrap_or(path)
    });

    debouncer
        .watch(&root, RecursiveMode::Recursive)
        .with_context(|| format!("failed to watch {}", root.display()))?;

    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(events)) => {
                let excluded = preview_output
                    .canonicalize()
                    .unwrap_or_else(|_| preview_output.clone());
                let mut changed = Vec::new();
                for event in events {
                    if !is_write_event(&event.event.kind) {
                        continue;
                    }
                    for path in event.event.paths {
                        let path = path.canonicalize().unwrap_or(path);
                        if is_watch_candidate(&root, &excluded, config_path.as_deref(), &path) && !changed.contains(&path) {
                            changed.push(path);
                        }
                    }
                }
                if !changed.is_empty() {
                    on_change(&changed);
                }
            }
            Ok(Err(errors)) => {
                for error in errors {
                    cwarn!("Watch error: {}", error);
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn watcher_ignores_generated_and_repository_paths() {
        let root = Path::new("/tmp/project");
        let output = root.join("paper.pdf");
        assert!(!is_watch_candidate(root, &output, None, &output));
        assert!(!is_watch_candidate(root, &output, None, &root.join("XXfo4zsx")));
        assert!(!is_watch_candidate(root, &output, None, &root.join("paper.typ~")));
        assert!(!is_watch_candidate(root, &output, None, &root.join(".git/index")));
        assert!(!is_watch_candidate(
            root,
            &output,
            None,
            &root.join("target/debug/x")
        ));
        assert!(!is_watch_candidate(
            root,
            &output,
            None,
            &root.join("node_modules/pkg/index.js")
        ));
        assert!(!is_watch_candidate(
            root,
            &output,
            None,
            &root.join(".venv/bin/python")
        ));
        assert!(!is_watch_candidate(
            root,
            &output,
            None,
            &root.join("editors/vscode/out/extension.js")
        ));
        assert!(!is_watch_candidate(
            root,
            &output,
            None,
            &root.join("editors/vscode/media/pdfjs/build/pdf.min.mjs")
        ));
        assert!(!is_watch_candidate(
            root,
            &output,
            None,
            &root.join("editors/vscode/dist/calepin.vsix")
        ));
        assert!(is_watch_candidate(root, &output, None, &root.join("paper.typ")));
        assert!(is_watch_candidate(
            root,
            &output,
            None,
            &root.join("data/input.csv")
        ));
    }

    #[test]
    fn watcher_accepts_write_like_events() {
        assert!(is_write_event(&notify::EventKind::Create(
            notify::event::CreateKind::File,
        )));
        assert!(is_write_event(&notify::EventKind::Modify(
            notify::event::ModifyKind::Data(notify::event::DataChange::Content),
        )));
        assert!(is_write_event(&notify::EventKind::Modify(
            notify::event::ModifyKind::Name(notify::event::RenameMode::Both),
        )));
        assert!(!is_write_event(&notify::EventKind::Access(
            notify::event::AccessKind::Read,
        )));
    }
}
