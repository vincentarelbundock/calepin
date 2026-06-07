use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::utils::tools;


pub const PYTHON_EXECUTABLE_ENV_VAR: &str = "CALEPIN_PYTHON";
#[cfg(windows)]
pub const PROJECT_VENV_PYTHON_RELATIVE_PATH: &str = ".venv/Scripts/python.exe";
#[cfg(not(windows))]
pub const PROJECT_VENV_PYTHON_RELATIVE_PATH: &str = ".venv/bin/python";

pub const DEFAULT_THEMES_DIR: &str = "themes";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalepinConfig {
    pub executables: ExecutablePaths,
    pub themes_dir: PathBuf,
}

impl CalepinConfig {
    pub fn load(root: &Path, config_path: Option<&Path>) -> Result<Self> {
        let Some(path) = config_path else {
            return Ok(Self::default_for_root(root));
        };
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            root.join(path)
        };
        if !path.exists() {
            return Err(anyhow!("config file not found: {}", path.display()));
        }
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let raw: RawCalepinConfig = toml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        // Relative paths written in the config resolve against the config
        // file's own directory, so a config can live anywhere and still point
        // at its siblings without `../` gymnastics.
        let config_dir = path.parent().unwrap_or(root);
        Ok(Self {
            executables: ExecutablePaths::from_raw(root, config_dir, raw.executables),
            themes_dir: resolve_themes_dir(root, config_dir, raw.themes_dir),
        })
    }

    fn default_for_root(root: &Path) -> Self {
        Self {
            executables: ExecutablePaths::from_raw(root, root, RawExecutablePaths::default()),
            themes_dir: resolve_themes_dir(root, root, None),
        }
    }
}

/// A configured `themes_dir` resolves relative to the config file's directory
/// (`config_dir`); the built-in default resolves relative to the project root.
/// Unlike executable names, a bare `themes` is a directory, not a `PATH`
/// lookup, so it is joined rather than left untouched.
fn resolve_themes_dir(project_root: &Path, config_dir: &Path, value: Option<PathBuf>) -> PathBuf {
    match value {
        Some(path) if path.is_absolute() => path,
        Some(path) => config_dir.join(path),
        None => project_root.join(DEFAULT_THEMES_DIR),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutablePaths {
    pub typst: PathBuf,
    pub rscript: PathBuf,
    pub python: PathBuf,
    pub mmdc: PathBuf,
    pub dot: PathBuf,
    pub tectonic: PathBuf,
    pub dvisvgm: PathBuf,
    pub pdf2svg: PathBuf,
    pub d2: PathBuf,
    pub chrome: Option<PathBuf>,
}

impl ExecutablePaths {
    pub fn defaults() -> Self {
        Self {
            typst: PathBuf::from("typst"),
            rscript: PathBuf::from(tools::RSCRIPT.cmd),
            python: PathBuf::from(tools::PYTHON.cmd),
            mmdc: PathBuf::from(tools::MMDC.cmd),
            dot: PathBuf::from(tools::DOT.cmd),
            tectonic: PathBuf::from(tools::TECTONIC.cmd),
            dvisvgm: PathBuf::from(tools::DVISVGM.cmd),
            pdf2svg: PathBuf::from(tools::PDF2SVG.cmd),
            d2: PathBuf::from(tools::D2.cmd),
            chrome: None,
        }
    }

    fn from_raw(project_root: &Path, config_dir: &Path, raw: RawExecutablePaths) -> Self {
        let defaults = Self::defaults();
        // The VS Code extension uses this as a process-local default for the
        // selected interpreter; an explicit project config still takes priority.
        let env_python = std::env::var_os(PYTHON_EXECUTABLE_ENV_VAR)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        // Paths written in the config resolve against the config file's
        // directory. The env var and the autodetected project `.venv` are not
        // values from the file, so they stay relative to the project root.
        let python = match raw.python {
            Some(path) => resolve_tool_path(config_dir, path),
            None => env_python
                .or_else(|| project_venv_python(project_root))
                .map(|path| resolve_tool_path(project_root, path))
                .unwrap_or(defaults.python),
        };
        Self {
            typst: tool_path(config_dir, raw.typst, defaults.typst),
            rscript: tool_path(config_dir, raw.rscript, defaults.rscript),
            python,
            mmdc: tool_path(config_dir, raw.mmdc, defaults.mmdc),
            dot: tool_path(config_dir, raw.dot, defaults.dot),
            tectonic: tool_path(config_dir, raw.tectonic, defaults.tectonic),
            dvisvgm: tool_path(config_dir, raw.dvisvgm, defaults.dvisvgm),
            pdf2svg: tool_path(config_dir, raw.pdf2svg, defaults.pdf2svg),
            d2: tool_path(config_dir, raw.d2, defaults.d2),
            chrome: raw.chrome.map(|path| resolve_tool_path(config_dir, path)),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawCalepinConfig {
    executables: RawExecutablePaths,
    themes_dir: Option<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawExecutablePaths {
    typst: Option<PathBuf>,
    rscript: Option<PathBuf>,
    python: Option<PathBuf>,
    mmdc: Option<PathBuf>,
    dot: Option<PathBuf>,
    tectonic: Option<PathBuf>,
    dvisvgm: Option<PathBuf>,
    pdf2svg: Option<PathBuf>,
    d2: Option<PathBuf>,
    chrome: Option<PathBuf>,
}


fn tool_path(root: &Path, value: Option<PathBuf>, default: PathBuf) -> PathBuf {
    value
        .map(|path| resolve_tool_path(root, path))
        .unwrap_or(default)
}

fn project_venv_python(root: &Path) -> Option<PathBuf> {
    let path = root.join(PROJECT_VENV_PYTHON_RELATIVE_PATH);
    path.is_file()
        .then(|| PathBuf::from(PROJECT_VENV_PYTHON_RELATIVE_PATH))
}

fn resolve_tool_path(root: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() || !is_path_like(&path) {
        return path;
    }
    root.join(path)
}

fn is_path_like(path: &Path) -> bool {
    path.components().count() > 1 || path.to_string_lossy().contains('\\')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, previous }
        }

        fn unset(key: &'static str) -> Self {
            let previous = std::env::var_os(key);
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    #[test]
    fn missing_config_uses_current_defaults() {
        let _env_lock = ENV_LOCK.lock().unwrap();
        let _env = EnvVarGuard::unset(PYTHON_EXECUTABLE_ENV_VAR);
        let dir = tempfile::tempdir().unwrap();
        let config = CalepinConfig::load(dir.path(), None).unwrap();

        assert_eq!(config.executables.typst, PathBuf::from("typst"));
        assert_eq!(config.executables.python, PathBuf::from("python3"));
        assert_eq!(config.executables.rscript, PathBuf::from("Rscript"));
    }

    #[test]
    fn missing_config_uses_default_themes_dir() {
        let _env_lock = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let config = CalepinConfig::load(dir.path(), None).unwrap();

        assert_eq!(config.themes_dir, dir.path().join("themes"));
    }

    #[test]
    fn config_overrides_themes_dir() {
        let _env_lock = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let calepin_dir = dir.path().join(".calepin");
        std::fs::create_dir(&calepin_dir).unwrap();
        std::fs::write(
            calepin_dir.join("config.toml"),
            "themes_dir = \"site/themes\"\n",
        )
        .unwrap();

        let config = CalepinConfig::load(dir.path(), Some(&calepin_dir.join("config.toml"))).unwrap();

        // `site/themes` resolves relative to the config file's directory.
        assert_eq!(config.themes_dir, calepin_dir.join("site/themes"));
    }

    #[test]
    fn missing_config_uses_python_env_default() {
        let _env_lock = ENV_LOCK.lock().unwrap();
        let _env = EnvVarGuard::set(PYTHON_EXECUTABLE_ENV_VAR, "env-python");
        let dir = tempfile::tempdir().unwrap();
        let config = CalepinConfig::load(dir.path(), None).unwrap();

        assert_eq!(config.executables.python, PathBuf::from("env-python"));
    }

    #[test]
    fn missing_config_uses_project_venv_python() {
        let _env_lock = ENV_LOCK.lock().unwrap();
        let _env = EnvVarGuard::unset(PYTHON_EXECUTABLE_ENV_VAR);
        let dir = tempfile::tempdir().unwrap();
        let python = dir.path().join(PROJECT_VENV_PYTHON_RELATIVE_PATH);
        std::fs::create_dir_all(python.parent().unwrap()).unwrap();
        std::fs::write(&python, "").unwrap();

        let config = CalepinConfig::load(dir.path(), None).unwrap();

        assert_eq!(config.executables.python, python);
    }

    #[test]
    fn config_overrides_executable_paths() {
        let _env_lock = ENV_LOCK.lock().unwrap();
        let _env = EnvVarGuard::set(PYTHON_EXECUTABLE_ENV_VAR, "env-python");
        let dir = tempfile::tempdir().unwrap();
        let calepin_dir = dir.path().join(".calepin");
        std::fs::create_dir(&calepin_dir).unwrap();
        std::fs::write(
            calepin_dir.join("config.toml"),
            r#"[executables]
typst = "typst-dev"
python = ".venv/bin/python"
chrome = "tools/chrome"
"#,
        )
        .unwrap();

        let config = CalepinConfig::load(dir.path(), Some(&calepin_dir.join("config.toml"))).unwrap();

        // Path-like values resolve relative to the config file's directory.
        assert_eq!(config.executables.typst, PathBuf::from("typst-dev"));
        assert_eq!(
            config.executables.python,
            calepin_dir.join(".venv/bin/python")
        );
        assert_eq!(
            config.executables.chrome,
            Some(calepin_dir.join("tools/chrome"))
        );
    }
}
