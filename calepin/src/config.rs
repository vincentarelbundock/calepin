use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::utils::path::{absolutize_from, expand_home, is_path_like, normalize_path};
#[cfg(windows)]
use crate::utils::process;
use crate::utils::tools;

pub const PYTHON_EXECUTABLE_ENV_VAR: &str = "CALEPIN_PYTHON";
#[cfg(windows)]
pub const PROJECT_VENV_PYTHON_RELATIVE_PATH: &str = ".venv/Scripts/python.exe";
#[cfg(not(windows))]
pub const PROJECT_VENV_PYTHON_RELATIVE_PATH: &str = ".venv/bin/python";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalepinConfig {
    pub executables: ExecutablePaths,
    pub config_dir: PathBuf,
    pub theme: Option<String>,
    /// Directory (relative to project root) for generated Calepin assets.
    /// `None` means the built-in default `.calepin`.
    pub asset_dir: Option<PathBuf>,
}

impl CalepinConfig {
    pub fn load(root: &Path, config_path: Option<&Path>) -> Result<Self> {
        let Some(path) = config_path else {
            return Ok(Self::default_for_root(root));
        };
        let path = resolve_config_path(path)?;
        if !path.exists() {
            return Err(anyhow!("config file not found: {}", path.display()));
        }
        if !is_toml_file(&path) {
            return Err(anyhow!(
                "config file must be a .toml file: {}",
                path.display()
            ));
        }
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let raw_value: toml::Value = toml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        reject_removed_asset_dir_keys(&raw_value)?;
        reject_removed_styles_key(&raw_value)?;
        let raw: RawCalepinConfig = raw_value
            .try_into()
            .with_context(|| format!("failed to parse {}", path.display()))?;
        // Relative paths written in the config resolve against the config
        // file's own directory, so a config can live anywhere and still point
        // at its siblings without `../` gymnastics.
        let config_dir = path.parent().unwrap_or(root).to_path_buf();
        let asset_dir = resolve_asset_dir(raw.asset_dir)?;
        Ok(Self {
            executables: ExecutablePaths::from_raw(root, &config_dir, raw.executables),
            config_dir,
            theme: raw.theme,
            asset_dir,
        })
    }

    fn default_for_root(root: &Path) -> Self {
        Self {
            executables: ExecutablePaths::from_raw(root, root, RawExecutablePaths::default()),
            config_dir: root.to_path_buf(),
            theme: None,
            asset_dir: None,
        }
    }

    pub fn theme_selection(&self) -> Result<Option<crate::theme::ThemeSelection>> {
        match &self.theme {
            None => Ok(None),
            Some(value) => Ok(Some(crate::theme::ThemeSelection::parse(
                value,
                &self.config_dir,
            )?)),
        }
    }
}

fn resolve_config_path(path: &Path) -> Result<PathBuf> {
    Ok(absolutize_from(&std::env::current_dir()?, path))
}

fn is_toml_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
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
                .unwrap_or_else(default_python_executable),
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
    theme: Option<String>,
    #[serde(rename = "asset-dir")]
    asset_dir: Option<PathBuf>,
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

fn default_python_executable() -> PathBuf {
    #[cfg(windows)]
    {
        windows_default_python_executable().unwrap_or_else(|| PathBuf::from(tools::PYTHON.cmd))
    }
    #[cfg(not(windows))]
    {
        PathBuf::from(tools::PYTHON.cmd)
    }
}

#[cfg(windows)]
fn windows_default_python_executable() -> Option<PathBuf> {
    ["python", "py", "python3"]
        .into_iter()
        .map(PathBuf::from)
        .find(|program| process::python_interpreter_is_usable(program))
}

fn resolve_tool_path(root: &Path, path: PathBuf) -> PathBuf {
    let path = expand_home(path);
    if path.is_absolute() || !is_path_like(&path) {
        return path;
    }
    root.join(path)
}

fn reject_removed_asset_dir_keys(value: &toml::Value) -> Result<()> {
    let Some(table) = value.as_table() else {
        return Ok(());
    };
    for key in ["runtime-dir", "runtime_dir", "asset_dir"] {
        if table.contains_key(key) {
            return Err(anyhow!(
                "`{key}` is not a supported config key; use `asset-dir` instead"
            ));
        }
    }
    Ok(())
}

fn reject_removed_styles_key(value: &toml::Value) -> Result<()> {
    let Some(table) = value.as_table() else {
        return Ok(());
    };
    if table.contains_key("styles") {
        return Err(anyhow!(
            "`styles` is not a supported config key; customize CSS through a local theme with `extends`"
        ));
    }
    Ok(())
}

fn resolve_asset_dir(value: Option<PathBuf>) -> Result<Option<PathBuf>> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let path = expand_home(raw);
    if path.as_os_str().is_empty() {
        return Err(anyhow!("asset-dir must not be empty"));
    }
    if path.is_absolute() {
        return Err(anyhow!(
            "asset-dir must be a relative path: {}",
            path.display()
        ));
    }
    if path.to_string_lossy().contains('\\') {
        return Err(anyhow!(
            "asset-dir must not contain '\\': {}",
            path.display()
        ));
    }
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir | std::path::Component::Prefix(_)
        )
    }) {
        return Err(anyhow!(
            "asset-dir must not include parent or drive path segments: {}",
            path.display()
        ));
    }

    let path = normalize_path(&path);
    if path.as_os_str().is_empty() {
        return Err(anyhow!("asset-dir must include at least one path segment"));
    }
    Ok(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testutil::{env_lock, EnvVarGuard};

    #[test]
    fn missing_config_uses_current_defaults() {
        let _env_lock = env_lock();
        let _env = EnvVarGuard::unset(PYTHON_EXECUTABLE_ENV_VAR);
        let dir = tempfile::tempdir().unwrap();
        let config = CalepinConfig::load(dir.path(), None).unwrap();

        assert_eq!(config.executables.typst, PathBuf::from("typst"));
        assert_eq!(config.executables.python, PathBuf::from("python3"));
        assert_eq!(config.executables.rscript, PathBuf::from("Rscript"));
    }

    #[test]
    fn missing_config_uses_python_env_default() {
        let _env_lock = env_lock();
        let _env = EnvVarGuard::set(PYTHON_EXECUTABLE_ENV_VAR, "env-python");
        let dir = tempfile::tempdir().unwrap();
        let config = CalepinConfig::load(dir.path(), None).unwrap();

        assert_eq!(config.executables.python, PathBuf::from("env-python"));
    }

    #[test]
    fn missing_config_uses_project_venv_python() {
        let _env_lock = env_lock();
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
        let _env_lock = env_lock();
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

        let config =
            CalepinConfig::load(dir.path(), Some(&calepin_dir.join("config.toml"))).unwrap();

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
        assert_eq!(config.asset_dir, None);
    }

    #[test]
    fn config_rejects_runtime_dir_key() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("calepin.toml"),
            "runtime-dir = \"_calepin\"",
        )
        .unwrap();

        let err = CalepinConfig::load(dir.path(), Some(&dir.path().join("calepin.toml")))
            .unwrap_err()
            .to_string();

        assert!(err.contains("runtime-dir"), "{err}");
        assert!(err.contains("asset-dir"), "{err}");
    }

    #[test]
    fn config_accepts_asset_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("calepin.toml"), "asset-dir = \"_calepin\"").unwrap();

        let config =
            CalepinConfig::load(dir.path(), Some(&dir.path().join("calepin.toml"))).unwrap();

        assert_eq!(config.asset_dir, Some(PathBuf::from("_calepin")));
    }

    #[test]
    fn config_rejects_runtime_dir_underscore_key() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("calepin.toml"),
            "runtime_dir = \"_calepin\"",
        )
        .unwrap();

        let err = CalepinConfig::load(dir.path(), Some(&dir.path().join("calepin.toml")))
            .unwrap_err()
            .to_string();

        assert!(err.contains("runtime_dir"), "{err}");
        assert!(err.contains("asset-dir"), "{err}");
    }

    #[test]
    fn config_rejects_asset_dir_underscore_key() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("calepin.toml"), "asset_dir = \"_calepin\"").unwrap();

        let err = CalepinConfig::load(dir.path(), Some(&dir.path().join("calepin.toml")))
            .unwrap_err()
            .to_string();

        assert!(err.contains("asset_dir"), "{err}");
        assert!(err.contains("asset-dir"), "{err}");
    }

    #[test]
    fn config_asset_dir_validation_errors_use_public_key() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("calepin.toml"),
            r#"asset-dir = "../_calepin""#,
        )
        .unwrap();

        let err = CalepinConfig::load(dir.path(), Some(&dir.path().join("calepin.toml")))
            .unwrap_err()
            .to_string();

        assert!(err.contains("asset-dir"), "{err}");
        assert!(!err.contains("runtime-dir"), "{err}");
    }

    #[test]
    fn config_expands_home_in_executable_paths() {
        let _env_lock = env_lock();
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        std::fs::create_dir(&home).unwrap();
        let _home = EnvVarGuard::set("HOME", home.to_str().unwrap());
        let calepin_dir = dir.path().join(".calepin");
        std::fs::create_dir(&calepin_dir).unwrap();
        std::fs::write(
            calepin_dir.join("config.toml"),
            r#"[executables]
typst = "~/Downloads/typst"
"#,
        )
        .unwrap();

        let config =
            CalepinConfig::load(dir.path(), Some(&calepin_dir.join("config.toml"))).unwrap();

        assert_eq!(config.executables.typst, home.join("Downloads/typst"));
    }

    #[test]
    fn explicit_relative_config_path_resolves_from_current_directory() {
        let _env_lock = env_lock();
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        let docs = project.join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(
            project.join("project.toml"),
            r#"[executables]
typst = "typst-dev"
"#,
        )
        .unwrap();
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(&project).unwrap();

        let config = CalepinConfig::load(&docs, Some(Path::new("project.toml")));

        std::env::set_current_dir(previous).unwrap();
        let config = config.unwrap();

        assert_eq!(config.executables.typst, PathBuf::from("typst-dev"));
    }

    #[test]
    fn config_rejects_removed_styles_key() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("calepin.toml"),
            r#"
theme = "academic"
styles = ["styles/site.css"]
"#,
        )
        .unwrap();

        let err = CalepinConfig::load(dir.path(), Some(&dir.path().join("calepin.toml")))
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("`styles` is not a supported config key"),
            "{err}"
        );
        assert!(err.contains("local theme"), "{err}");
    }

    #[test]
    fn config_theme_resolves_relative_to_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join("project");
        let theme_dir = config_dir.join("themes/local");
        std::fs::create_dir_all(theme_dir.join("layouts")).unwrap();
        std::fs::write(theme_dir.join("layouts/notebook.html"), "{{ body }}").unwrap();
        std::fs::write(config_dir.join("calepin.toml"), r#"theme = "themes/local""#).unwrap();

        let config =
            CalepinConfig::load(dir.path(), Some(&config_dir.join("calepin.toml"))).unwrap();

        assert_eq!(
            config.theme_selection().unwrap(),
            Some(crate::theme::ThemeSelection::Dir(theme_dir))
        );
    }

    #[test]
    fn config_theme_typst_selects_raw_typst_output() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("calepin.toml"), r#"theme = "typst""#).unwrap();

        let config =
            CalepinConfig::load(dir.path(), Some(&dir.path().join("calepin.toml"))).unwrap();

        assert_eq!(
            config.theme_selection().unwrap(),
            Some(crate::theme::ThemeSelection::Typst)
        );
    }

    #[test]
    fn config_theme_rejects_boolean_values() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("calepin.toml"), "theme = false").unwrap();

        let err = CalepinConfig::load(dir.path(), Some(&dir.path().join("calepin.toml")))
            .unwrap_err()
            .to_string();

        assert!(err.contains("failed to parse"), "{err}");
    }

    #[test]
    fn config_path_must_be_toml() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "#let setup = (:)\n").unwrap();

        let err = CalepinConfig::load(dir.path(), Some(&input))
            .unwrap_err()
            .to_string();

        assert!(err.contains("config file must be a .toml file"), "{err}");
        assert!(err.contains("paper.typ"), "{err}");
    }

    #[test]
    fn revealjs_config_key_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("calepin.toml"), "[revealjs]\nhash = false\n").unwrap();

        let err = CalepinConfig::load(dir.path(), Some(&dir.path().join("calepin.toml")))
            .unwrap_err()
            .to_string();

        assert!(err.contains("unknown field `revealjs`"), "{err}");
    }
}
