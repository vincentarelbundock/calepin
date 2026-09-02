use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

pub(crate) use crate::utils::watch::is_write_event;
use anyhow::{Context, Result};
use notify::RecursiveMode;

use crate::typst::paths::is_generated_entry_file;
use crate::utils::path::absolutize_from;
use crate::utils::static_files::path_has_common_skip_dir;
use crate::utils::watch::run_debounced_watch;

pub(crate) fn is_watch_candidate(
    root: &Path,
    preview_output: &Path,
    artifact_root: &Path,
    config_path: Option<&Path>,
    path: &Path,
) -> bool {
    if path == preview_output
        || path.starts_with(artifact_root)
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

    // Calepin writes the generated entry files itself on every preprocess pass.
    // Watching them would turn each rebuild into the trigger for the next one.
    if rel.components().next().is_none()
        || path_has_common_skip_dir(rel)
        || is_generated_entry_file(rel)
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
    artifact_root: &Path,
    config_path: Option<&Path>,
    stop: Arc<AtomicBool>,
    on_change: impl FnMut(&[PathBuf]),
) -> Result<()> {
    let root = root
        .canonicalize()
        .with_context(|| format!("watch root not found: {}", root.display()))?;
    let preview_output = preview_output.to_path_buf();
    // Resolve a relative `--config` the same way the CLI does: against the
    // current directory, not the project root (see `config::resolve_config_path`).
    let config_path = config_path.map(|path| {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let path = absolutize_from(&cwd, path);
        path.canonicalize().unwrap_or(path)
    });

    let excluded = preview_output
        .canonicalize()
        .unwrap_or_else(|_| preview_output.clone());
    let artifact_root = artifact_root
        .canonicalize()
        .unwrap_or_else(|_| artifact_root.to_path_buf());

    let mut watches = vec![(root.clone(), RecursiveMode::Recursive)];
    // A `--config` file outside the project root would otherwise never be
    // watched at all: `notify` only reports events under the paths it was
    // told to watch.
    if let Some(config_path) = &config_path {
        if !config_path.starts_with(&root) {
            watches.push((config_path.clone(), RecursiveMode::NonRecursive));
        }
    }

    run_debounced_watch(
        &watches,
        Duration::from_millis(300),
        Duration::from_millis(200),
        stop,
        is_write_event,
        |path| {
            is_watch_candidate(
                &root,
                &excluded,
                &artifact_root,
                config_path.as_deref(),
                &path,
            )
            .then_some(path)
        },
        on_change,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn watcher_ignores_generated_and_repository_paths() {
        let root = Path::new("/tmp/project");
        let output = root.join("paper.pdf");
        let artifact_root = root.join("_calepin");
        let candidate = |path: &Path| is_watch_candidate(root, &output, &artifact_root, None, path);

        assert!(!candidate(&output));
        assert!(!candidate(&root.join("_calepin/paper/results.json")));
        assert!(!candidate(&root.join("XXfo4zsx")));
        assert!(!candidate(&root.join("paper.typ~")));
        assert!(!candidate(&root.join(".git/index")));
        assert!(!candidate(&root.join("target/debug/x")));
        assert!(!candidate(&root.join("node_modules/pkg/index.js")));
        assert!(!candidate(&root.join(".venv/bin/python")));
        assert!(candidate(&root.join("paper.typ")));
        assert!(candidate(&root.join("data/input.csv")));
        // Not hard-coded to any particular project layout: an ordinary
        // directory under the root is a candidate, unlike the generated
        // and vendored paths above.
        assert!(candidate(&root.join("editors/vscode/out/extension.js")));
    }

    #[test]
    fn config_path_outside_root_is_watched() {
        let root = Path::new("/tmp/project");
        let output = root.join("paper.pdf");
        let artifact_root = root.join("_calepin");
        let outside_config = Path::new("/tmp/shared/config.toml");

        assert!(is_watch_candidate(
            root,
            &output,
            &artifact_root,
            Some(outside_config),
            outside_config,
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
