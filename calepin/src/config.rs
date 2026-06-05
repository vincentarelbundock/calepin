use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::utils::tools;

pub const CONFIG_RELATIVE_PATH: &str = ".calepin/config.toml";
pub const PYTHON_EXECUTABLE_ENV_VAR: &str = "CALEPIN_PYTHON";

pub const DEFAULT_THEMES_DIR: &str = "themes";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DiagramOutput {
    #[default]
    File,
    Stdout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomDiagramBackend {
    pub command: PathBuf,
    pub input_ext: String,
    pub args: Vec<String>,
    pub output: DiagramOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalepinConfig {
    pub executables: ExecutablePaths,
    pub themes_dir: PathBuf,
    pub custom_diagrams: IndexMap<String, CustomDiagramBackend>,
}

impl CalepinConfig {
    pub fn load(root: &Path) -> Result<Self> {
        let path = config_path(root);
        if !path.exists() {
            return Ok(Self::default_for_root(root));
        }

        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let raw: RawCalepinConfig = toml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        Ok(Self {
            executables: ExecutablePaths::from_raw(root, raw.executables),
            themes_dir: resolve_themes_dir(root, raw.themes_dir),
            custom_diagrams: custom_diagrams_from_raw(root, raw.diagram),
        })
    }

    fn default_for_root(root: &Path) -> Self {
        Self {
            executables: ExecutablePaths::from_raw(root, RawExecutablePaths::default()),
            themes_dir: resolve_themes_dir(root, None),
            custom_diagrams: IndexMap::new(),
        }
    }
}

/// The themes directory always resolves relative to the project root unless
/// absolute. Unlike executable names, a bare `themes` is a directory, not a
/// `PATH` lookup, so it is joined to the root rather than left untouched.
fn resolve_themes_dir(root: &Path, value: Option<PathBuf>) -> PathBuf {
    match value {
        Some(path) if path.is_absolute() => path,
        Some(path) => root.join(path),
        None => root.join(DEFAULT_THEMES_DIR),
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

    fn from_raw(root: &Path, raw: RawExecutablePaths) -> Self {
        let defaults = Self::defaults();
        // The VS Code extension uses this as a process-local default for the
        // selected interpreter; an explicit project config still takes priority.
        let env_python = std::env::var_os(PYTHON_EXECUTABLE_ENV_VAR)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Self {
            typst: tool_path(root, raw.typst, defaults.typst),
            rscript: tool_path(root, raw.rscript, defaults.rscript),
            python: tool_path(root, raw.python.or(env_python), defaults.python),
            mmdc: tool_path(root, raw.mmdc, defaults.mmdc),
            dot: tool_path(root, raw.dot, defaults.dot),
            tectonic: tool_path(root, raw.tectonic, defaults.tectonic),
            dvisvgm: tool_path(root, raw.dvisvgm, defaults.dvisvgm),
            pdf2svg: tool_path(root, raw.pdf2svg, defaults.pdf2svg),
            d2: tool_path(root, raw.d2, defaults.d2),
            chrome: raw.chrome.map(|path| resolve_tool_path(root, path)),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawCalepinConfig {
    executables: RawExecutablePaths,
    themes_dir: Option<PathBuf>,
    diagram: IndexMap<String, RawCustomDiagramBackend>,
}

#[derive(Debug, Deserialize)]
struct RawCustomDiagramBackend {
    command: PathBuf,
    #[serde(default = "default_input_ext")]
    input_ext: String,
    #[serde(default = "default_args")]
    args: Vec<String>,
    #[serde(default)]
    output: RawDiagramOutput,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawDiagramOutput {
    #[default]
    File,
    Stdout,
}

fn default_input_ext() -> String {
    "txt".to_string()
}

fn default_args() -> Vec<String> {
    vec!["{input}".to_string()]
}

fn custom_diagrams_from_raw(
    root: &Path,
    raw: IndexMap<String, RawCustomDiagramBackend>,
) -> IndexMap<String, CustomDiagramBackend> {
    raw.into_iter()
        .map(|(name, r)| {
            let backend = CustomDiagramBackend {
                command: resolve_tool_path(root, r.command),
                input_ext: r.input_ext,
                args: r.args,
                output: match r.output {
                    RawDiagramOutput::File => DiagramOutput::File,
                    RawDiagramOutput::Stdout => DiagramOutput::Stdout,
                },
            };
            (name, backend)
        })
        .collect()
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

pub fn config_path(root: &Path) -> PathBuf {
    root.join(CONFIG_RELATIVE_PATH)
}

fn tool_path(root: &Path, value: Option<PathBuf>, default: PathBuf) -> PathBuf {
    value
        .map(|path| resolve_tool_path(root, path))
        .unwrap_or(default)
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
        let config = CalepinConfig::load(dir.path()).unwrap();

        assert_eq!(config.executables.typst, PathBuf::from("typst"));
        assert_eq!(config.executables.python, PathBuf::from("python3"));
        assert_eq!(config.executables.rscript, PathBuf::from("Rscript"));
    }

    #[test]
    fn missing_config_uses_default_themes_dir() {
        let _env_lock = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let config = CalepinConfig::load(dir.path()).unwrap();

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

        let config = CalepinConfig::load(dir.path()).unwrap();

        assert_eq!(config.themes_dir, dir.path().join("site/themes"));
    }

    #[test]
    fn missing_config_uses_python_env_default() {
        let _env_lock = ENV_LOCK.lock().unwrap();
        let _env = EnvVarGuard::set(PYTHON_EXECUTABLE_ENV_VAR, "env-python");
        let dir = tempfile::tempdir().unwrap();
        let config = CalepinConfig::load(dir.path()).unwrap();

        assert_eq!(config.executables.python, PathBuf::from("env-python"));
    }

    #[test]
    fn custom_diagram_parsed_from_config() {
        let _env_lock = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let calepin_dir = dir.path().join(".calepin");
        std::fs::create_dir(&calepin_dir).unwrap();
        std::fs::write(
            calepin_dir.join("config.toml"),
            r#"[diagram.plantuml]
command = "plantuml"
input_ext = "puml"
args = ["-tsvg", "-pipe", "{input}"]
output = "stdout"
"#,
        )
        .unwrap();

        let config = CalepinConfig::load(dir.path()).unwrap();

        let backend = config.custom_diagrams.get("plantuml").unwrap();
        assert_eq!(backend.command, PathBuf::from("plantuml"));
        assert_eq!(backend.input_ext, "puml");
        assert_eq!(backend.args, vec!["-tsvg", "-pipe", "{input}"]);
        assert!(matches!(backend.output, DiagramOutput::Stdout));
    }

    #[test]
    fn custom_diagram_defaults() {
        let _env_lock = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let calepin_dir = dir.path().join(".calepin");
        std::fs::create_dir(&calepin_dir).unwrap();
        std::fs::write(
            calepin_dir.join("config.toml"),
            "[diagram.mydiag]\ncommand = \"mydiag\"\n",
        )
        .unwrap();

        let config = CalepinConfig::load(dir.path()).unwrap();

        let backend = config.custom_diagrams.get("mydiag").unwrap();
        assert_eq!(backend.input_ext, "txt");
        assert_eq!(backend.args, vec!["{input}"]);
        assert!(matches!(backend.output, DiagramOutput::File));
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

        let config = CalepinConfig::load(dir.path()).unwrap();

        assert_eq!(config.executables.typst, PathBuf::from("typst-dev"));
        assert_eq!(
            config.executables.python,
            dir.path().join(".venv/bin/python")
        );
        assert_eq!(
            config.executables.chrome,
            Some(dir.path().join("tools/chrome"))
        );
    }
}
