use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

use crate::typst::model::LayoutPaths;

pub fn resolve_layout(
    input: &Path,
    root: Option<&Path>,
    results: Option<&Path>,
) -> Result<LayoutPaths> {
    let input_abs =
        absolutize(input).with_context(|| format!("failed to resolve {}", input.display()))?;
    let root_abs = match root {
        Some(root) => {
            absolutize(root).with_context(|| format!("failed to resolve {}", root.display()))?
        }
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
    let base = root_abs.join(".calepin").join(&stem);
    let results_path = match results {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => root_abs.join(path),
        None => base.join("results.json"),
    };
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
        results_path,
        figures_dir: base.join("figures"),
    })
}

pub fn artifact_reference(root: &Path, path: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) => format!("/{}", slash_path(rel)),
        Err(_) => slash_path(path),
    }
}

pub fn project_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(slash_path)
        .unwrap_or_else(|_| path.display().to_string())
}

pub fn slash_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn absolutize(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return std::fs::canonicalize(path).map_err(Into::into);
    }
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(path)
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

        let layout = resolve_layout(&input, Some(dir.path()), None).unwrap();

        assert_eq!(layout.input_rel, PathBuf::from("chapters/intro.typ"));
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

        let layout = resolve_layout(&input, None, None).unwrap();

        assert_eq!(layout.input_rel, PathBuf::from("paper.typ"));
        assert_eq!(
            layout.results_path,
            root.join(".calepin/paper/results.json")
        );
    }

    #[test]
    fn artifact_refs_are_root_relative_with_slashes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".calepin/paper/figures/fig.svg");
        assert_eq!(
            artifact_reference(dir.path(), &path),
            "/.calepin/paper/figures/fig.svg"
        );
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
}
