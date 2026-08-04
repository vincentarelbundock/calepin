use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

use crate::typst::model::LayoutPaths;
pub use crate::utils::path::slash_path;

pub(crate) const CALEPIN_DIR: &str = ".calepin";

/// Name prefix shared by every generated Typst entry file Calepin writes beside
/// a source document. The leading dot keeps the files out of ordinary listings
/// and out of website page discovery, which skips hidden paths.
pub const ENTRY_FILE_PREFIX: &str = ".calepin-entry.";

/// Suffixes appended after the document stem, one per generated entry file.
pub const ENTRY_FILE_NAMES: &[&str] = &[
    "source.typ",
    "query-source.typ",
    "wrapper.typ",
    "query-wrapper.typ",
];

/// True when `path` names a generated entry file. Callers that walk a project
/// tree (website discovery, static copying, link checks, the watcher) use this
/// to ignore Calepin's own scratch files.
pub fn is_generated_entry_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(ENTRY_FILE_PREFIX))
}

/// Delete the generated entry files for one document. Callers run this after a
/// successful render: keeping them after a failure lets Typst's error spans,
/// which point into the entry file, still resolve.
pub fn remove_entry_files(layout: &LayoutPaths) {
    for path in layout.entry_paths() {
        let _ = std::fs::remove_file(path);
    }
}

/// Delete every generated entry file under `dir`. Used after website builds,
/// which stage one set of entry files per page, and by `calepin clean`.
pub fn remove_entry_files_under(dir: &Path) -> Result<Vec<PathBuf>> {
    let removed = find_entry_files(dir)?;
    for path in &removed {
        std::fs::remove_file(path)
            .with_context(|| format!("failed to remove {}", path.display()))?;
    }
    Ok(removed)
}

/// Every generated entry file under `dir`, sorted.
pub fn find_entry_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect_entry_files(dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn collect_entry_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };
    for entry in entries {
        let path = entry?.path();
        if path.is_dir() {
            let skip = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| crate::utils::static_files::COMMON_SKIP_DIRS.contains(&name));
            if !skip {
                collect_entry_files(&path, out)?;
            }
        } else if is_generated_entry_file(&path) {
            out.push(path);
        }
    }
    Ok(())
}

pub fn resolve_layout(input: &Path, root: Option<&Path>) -> Result<LayoutPaths> {
    resolve_layout_in_dir(input, root, Path::new(CALEPIN_DIR))
}

pub fn resolve_layout_in_dir(
    input: &Path,
    root: Option<&Path>,
    artifact_dir: &Path,
) -> Result<LayoutPaths> {
    let input_abs = canonicalize_input_file(input)?;
    let root_abs = match root {
        Some(root) => canonicalize_root_dir(root)?,
        None => input_abs
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")),
    };

    let input_rel = input_abs
        .strip_prefix(&root_abs)
        .map(Path::to_path_buf)
        .map_err(|_| {
            anyhow!(
                "input `{}` is not under root `{}`",
                input_abs.display(),
                root_abs.display()
            )
        })?;
    let stem = input_stem(&input_rel)?;
    let base = root_abs.join(artifact_dir).join(&stem);
    let results_path = base.join("results.json");
    let work_dir = input_abs
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root_abs.clone());

    Ok(LayoutPaths {
        root: root_abs,
        input: input_abs,
        input_rel: input_rel.clone(),
        render_input: input_rel.clone(),
        work_dir,
        artifact_dir: base.clone(),
        results_path,
        figures_dir: base.join("figures"),
    })
}

pub fn artifact_reference(root: &Path, path: &Path) -> Result<String> {
    let rel = path.strip_prefix(root).map_err(|_| {
        anyhow!(
            "artifact `{}` is not under root `{}`",
            path.display(),
            root.display()
        )
    })?;
    Ok(format!("/{}", slash_path(rel)))
}

pub fn project_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(slash_path)
        .unwrap_or_else(|_| display_path(path))
}

fn canonicalize_input_file(path: &Path) -> Result<PathBuf> {
    let path = canonicalize_existing_path(path, "input")?;
    let metadata = std::fs::metadata(&path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    if !metadata.is_file() {
        return Err(anyhow!("input `{}` must be a file", path.display()));
    }
    Ok(path)
}

fn canonicalize_root_dir(path: &Path) -> Result<PathBuf> {
    let path = canonicalize_existing_path(path, "root")?;
    let metadata = std::fs::metadata(&path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    if !metadata.is_dir() {
        return Err(anyhow!("root `{}` must be a directory", path.display()));
    }
    Ok(path)
}

fn canonicalize_existing_path(path: &Path, label: &str) -> Result<PathBuf> {
    std::fs::canonicalize(path)
        .with_context(|| format!("failed to resolve {label} `{}`", path.display()))
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn input_stem(input_rel: &Path) -> Result<PathBuf> {
    let mut stem = input_rel.to_path_buf();
    if stem.extension().and_then(|extension| extension.to_str()) != Some("typ") {
        return Err(anyhow!(
            "input `{}` must have a .typ extension",
            input_rel.display()
        ));
    }
    stem.set_extension("");
    Ok(stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lays_out_root_relative_nested_input() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("chapters").join("intro.typ");
        std::fs::create_dir_all(input.parent().unwrap()).unwrap();
        std::fs::write(&input, "").unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();

        let layout = resolve_layout(&input, Some(dir.path())).unwrap();

        assert_eq!(layout.input_rel, PathBuf::from("chapters/intro.typ"));
        assert_eq!(layout.artifact_root(), root.join(".calepin"));
        assert_eq!(
            layout.results_path,
            root.join(".calepin/chapters/intro/results.json")
        );
        assert_eq!(
            layout.figures_dir,
            root.join(".calepin/chapters/intro/figures")
        );
    }

    #[test]
    fn defaults_root_to_input_directory() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();

        let layout = resolve_layout(&input, None).unwrap();

        assert_eq!(layout.input_rel, PathBuf::from("paper.typ"));
        assert_eq!(
            layout.results_path,
            root.join(".calepin/paper/results.json")
        );
    }

    #[test]
    fn custom_artifact_dir_changes_generated_paths() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();

        let layout = resolve_layout_in_dir(&input, None, Path::new("_calepin")).unwrap();

        assert_eq!(layout.artifact_dir, root.join("_calepin/paper"));
        assert_eq!(layout.artifact_root(), root.join("_calepin"));
        assert_eq!(
            layout.results_path,
            root.join("_calepin/paper/results.json")
        );
        assert_eq!(layout.figures_dir, root.join("_calepin/paper/figures"));
    }

    #[test]
    fn rejects_missing_input() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("missing.typ");

        let err = resolve_layout(&input, Some(dir.path()))
            .unwrap_err()
            .to_string();

        assert!(err.contains("failed to resolve input"), "{err}");
        assert!(err.contains("missing.typ"), "{err}");
    }

    #[test]
    fn rejects_directory_input() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::create_dir(&input).unwrap();

        let err = resolve_layout(&input, Some(dir.path()))
            .unwrap_err()
            .to_string();

        assert!(err.contains("must be a file"), "{err}");
        assert!(err.contains("paper.typ"), "{err}");
    }

    #[test]
    fn rejects_missing_root() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        let root = dir.path().join("missing-root");
        std::fs::write(&input, "").unwrap();

        let err = resolve_layout(&input, Some(&root)).unwrap_err().to_string();

        assert!(err.contains("failed to resolve root"), "{err}");
        assert!(err.contains("missing-root"), "{err}");
    }

    #[test]
    fn rejects_file_root() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        let root = dir.path().join("calepin.toml");
        std::fs::write(&input, "").unwrap();
        std::fs::write(&root, "").unwrap();

        let err = resolve_layout(&input, Some(&root)).unwrap_err().to_string();

        assert!(err.contains("must be a directory"), "{err}");
        assert!(err.contains("calepin.toml"), "{err}");
    }

    #[test]
    fn rejects_missing_parent_segment_input_before_layout() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        std::fs::create_dir(&root).unwrap();
        let input = root.join("../outside.typ");

        let err = resolve_layout(&input, Some(&root)).unwrap_err().to_string();

        assert!(err.contains("failed to resolve input"), "{err}");
        assert!(err.contains("outside.typ"), "{err}");
    }

    #[test]
    fn artifact_refs_are_root_relative_with_slashes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".calepin/paper/figures/fig.svg");
        assert_eq!(
            artifact_reference(dir.path(), &path).unwrap(),
            "/.calepin/paper/figures/fig.svg"
        );
    }

    #[test]
    fn artifact_reference_rejects_paths_outside_root() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let path = outside.path().join("fig.svg");

        let err = artifact_reference(root.path(), &path)
            .unwrap_err()
            .to_string();

        assert!(err.contains("is not under root"), "{err}");
        assert!(err.contains("fig.svg"), "{err}");
    }

    #[test]
    fn project_relative_paths_are_short_for_humans() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".calepin/paper/results.json");
        assert_eq!(
            project_relative_path(dir.path(), &path),
            ".calepin/paper/results.json"
        );
    }

    #[test]
    fn project_relative_path_normalizes_outside_backslash_paths() {
        assert_eq!(
            project_relative_path(Path::new("/project"), Path::new(r"C:\project\paper.typ")),
            "C:/project/paper.typ"
        );
    }

    #[test]
    fn project_relative_path_preserves_single_root_for_absolute_fallback() {
        assert_eq!(
            project_relative_path(Path::new("/project"), Path::new("/tmp/paper.typ")),
            "/tmp/paper.typ"
        );
    }
}
