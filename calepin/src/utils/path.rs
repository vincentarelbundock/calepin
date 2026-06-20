use std::path::{Path, PathBuf};

pub fn expand_home(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    let Some(rest) = text.strip_prefix("~/") else {
        return path;
    };
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(rest))
        .unwrap_or(path)
}

pub fn is_path_like(path: &Path) -> bool {
    path.components().count() > 1 || path.to_string_lossy().contains('\\')
}

pub fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            _ => out.push(component.as_os_str()),
        }
    }
    out
}

pub fn absolutize_from(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

pub fn slash_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::absolutize_from;
    use std::path::{Path, PathBuf};

    #[test]
    fn absolutize_from_joins_only_relative_paths() {
        assert_eq!(
            absolutize_from(Path::new("/project"), Path::new("config.toml")),
            PathBuf::from("/project/config.toml")
        );
        assert_eq!(
            absolutize_from(Path::new("/project"), Path::new("/tmp/config.toml")),
            PathBuf::from("/tmp/config.toml")
        );
    }
}
