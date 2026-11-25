use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::cli::HealthArgs;
use crate::config::CalepinConfig;
use crate::typst::version::{typst_version, version_is_too_old, REQUIRED_TYPST_VERSION};
use crate::utils::process::{validate_executable, validate_python_interpreter};
use crate::utils::tools::{self, Tool};

mod links;
mod quality;
mod source;

use links::link_check;
use quality::{image_check, slug_check};
use source::{collect_typst_files, mask_raw_spans};

/// One Typst source file, read and raw-span-masked once so the quality
/// checks (links, images, slugs) do not each re-walk and re-read it.
pub(crate) struct QualitySource {
    pub(crate) path: PathBuf,
    pub(crate) masked: String,
}

fn collect_quality_sources(root: &Path, depth: Option<usize>) -> Result<Vec<QualitySource>> {
    collect_typst_files(root, depth)?
        .into_iter()
        .map(|path| {
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let masked = mask_raw_spans(&source);
            Ok(QualitySource { path, masked })
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: HealthStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<String>,
}

impl HealthCheck {
    fn new(name: &str, status: HealthStatus, message: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            status,
            path: None,
            message: message.into(),
            hint: None,
            details: Vec::new(),
        }
    }

    fn ok(name: &str, message: impl Into<String>) -> Self {
        Self::new(name, HealthStatus::Ok, message)
    }

    fn warn(name: &str, message: impl Into<String>) -> Self {
        Self::new(name, HealthStatus::Warning, message)
    }

    fn error(name: &str, message: impl Into<String>) -> Self {
        Self::new(name, HealthStatus::Error, message)
    }

    fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    fn with_optional_hint(mut self, hint: Option<String>) -> Self {
        self.hint = hint;
        self
    }

    fn with_details(mut self, details: Vec<String>) -> Self {
        self.details = details;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HealthReport {
    pub root: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<String>,
    pub checks: Vec<HealthCheck>,
}

impl HealthReport {
    fn has_errors(&self) -> bool {
        self.checks
            .iter()
            .any(|check| check.status == HealthStatus::Error)
    }

    fn has_warnings(&self) -> bool {
        self.checks
            .iter()
            .any(|check| check.status == HealthStatus::Warning)
    }

    fn counts(&self) -> (usize, usize, usize) {
        self.checks.iter().fold((0, 0, 0), |mut counts, check| {
            match check.status {
                HealthStatus::Ok => counts.0 += 1,
                HealthStatus::Warning => counts.1 += 1,
                HealthStatus::Error => counts.2 += 1,
            }
            counts
        })
    }
}

pub fn handle_health(args: HealthArgs) -> Result<()> {
    let report = build_report(
        args.config.as_deref(),
        args.depth,
        args.check_external_links,
    )?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_text_report(&report);
    }

    if report.has_errors() || (args.strict && report.has_warnings()) {
        std::process::exit(1);
    }

    Ok(())
}

pub fn build_report(
    config_path: Option<&Path>,
    check_links_depth: Option<usize>,
    check_external_links: bool,
) -> Result<HealthReport> {
    let root = std::env::current_dir()?;
    let config = CalepinConfig::load(&root, config_path)?;
    let mut checks = vec![
        typst_check(
            "typst",
            &config.executables.typst,
            Some(&tools::TYPST),
            true,
            "render Typst documents",
        ),
        python_check(
            "python",
            &config.executables.python,
            Some(&tools::PYTHON),
            true,
            "execute Python chunks",
        ),
        tool_check(
            "Rscript",
            &config.executables.rscript,
            Some(&tools::RSCRIPT),
            false,
            "execute R chunks",
        ),
        tool_check(
            "mmdc",
            &config.executables.mmdc,
            Some(&tools::MMDC),
            false,
            "render Mermaid diagrams",
        ),
        tool_check(
            "dot",
            &config.executables.dot,
            Some(&tools::DOT),
            false,
            "render Graphviz DOT diagrams",
        ),
        tool_check(
            "d2",
            &config.executables.d2,
            Some(&tools::D2),
            false,
            "render D2 diagrams",
        ),
        tool_check(
            "tectonic",
            &config.executables.tectonic,
            Some(&tools::TECTONIC),
            false,
            "render TikZ diagrams",
        ),
        tool_check(
            "dvisvgm",
            &config.executables.dvisvgm,
            Some(&tools::DVISVGM),
            false,
            "convert TikZ output to SVG",
        ),
        tool_check(
            "pdf2svg",
            &config.executables.pdf2svg,
            Some(&tools::PDF2SVG),
            false,
            "convert PDFs to SVG",
        ),
    ];

    let python_available = checks
        .iter()
        .find(|check| check.name == "python")
        .is_some_and(|check| check.status == HealthStatus::Ok);
    let jupyter_client = jupyter_client_check(&config.executables.python, python_available);
    let jupyter_client_available = jupyter_client.status == HealthStatus::Ok;
    checks.push(jupyter_client);
    checks.push(jupyter_kernels_check(
        &config.executables.python,
        jupyter_client_available,
    ));
    match collect_quality_sources(&root, check_links_depth) {
        Ok(sources) => {
            checks.push(link_check(&root, &sources, check_external_links));
            checks.push(image_check(&root, &sources));
            checks.push(slug_check(&root, &sources));
        }
        Err(error) => {
            let message = format!("failed to collect Typst files: {error}");
            for name in ["links", "images", "slugs"] {
                checks.push(
                    HealthCheck::warn(name, message.clone()).with_path(root.display().to_string()),
                );
            }
        }
    }

    Ok(HealthReport {
        root: root.display().to_string(),
        config: config_path.map(display_config_path).transpose()?,
        checks,
    })
}

fn tool_check(
    name: &str,
    path: &Path,
    tool: Option<&Tool>,
    required: bool,
    action: &str,
) -> HealthCheck {
    checked_tool(name, path, tool, required, action, validate_executable)
}

fn python_check(
    name: &str,
    path: &Path,
    tool: Option<&Tool>,
    required: bool,
    action: &str,
) -> HealthCheck {
    checked_tool(
        name,
        path,
        tool,
        required,
        action,
        validate_python_interpreter,
    )
}

fn checked_tool(
    name: &str,
    path: &Path,
    tool: Option<&Tool>,
    required: bool,
    action: &str,
    validate: fn(&Path, &str, Option<&Tool>) -> Result<()>,
) -> HealthCheck {
    match validate(path, action, tool) {
        Ok(()) => HealthCheck::ok(name, "found").with_path(path.display().to_string()),
        Err(error) => HealthCheck::new(
            name,
            if required {
                HealthStatus::Error
            } else {
                HealthStatus::Warning
            },
            error.to_string(),
        )
        .with_path(path.display().to_string())
        .with_optional_hint(tool.map(|tool| tool.install_hint.to_string())),
    }
}

fn typst_check(
    name: &str,
    path: &Path,
    tool: Option<&Tool>,
    required: bool,
    action: &str,
) -> HealthCheck {
    let mut check = tool_check(name, path, tool, required, action);
    if check.status != HealthStatus::Ok {
        return check;
    }

    match typst_version(path) {
        Ok(version) if version_is_too_old(&version) => HealthCheck::error(
            name,
            format!(
                "Typst {version} is too old; Calepin requires Typst {REQUIRED_TYPST_VERSION} or newer"
            ),
        )
        .with_path(path.display().to_string())
        .with_hint(tools::TYPST.install_hint),
        Ok(version) => {
            check.message = format!("found Typst {version}");
            check
        }
        Err(error) => HealthCheck::error(name, error.to_string())
            .with_path(path.display().to_string())
            .with_hint(tools::TYPST.install_hint),
    }
}

fn jupyter_client_check(python: &Path, python_available: bool) -> HealthCheck {
    if !python_available {
        return HealthCheck::warn("jupyter_client", "skipped because Python is not available")
            .with_path(python.display().to_string())
            .with_hint(tools::JUPYTER_CLIENT.install_hint);
    }

    match Command::new(python)
        .args(["-c", "import jupyter_client"])
        .output()
    {
        Ok(output) if output.status.success() => {
            HealthCheck::ok("jupyter_client", "available in configured Python")
                .with_path(python.display().to_string())
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            HealthCheck::warn(
                "jupyter_client",
                if stderr.is_empty() {
                    "not importable from configured Python".to_string()
                } else {
                    format!("not importable from configured Python: {stderr}")
                },
            )
            .with_path(python.display().to_string())
            .with_hint(tools::JUPYTER_CLIENT.install_hint)
        }
        Err(error) => HealthCheck::warn(
            "jupyter_client",
            format!("failed to check jupyter_client: {error}"),
        )
        .with_path(python.display().to_string())
        .with_hint(tools::JUPYTER_CLIENT.install_hint),
    }
}

#[derive(Debug, Deserialize)]
struct JupyterKernelspecList {
    kernelspecs: BTreeMap<String, JupyterKernelspecEntry>,
}

#[derive(Debug, Deserialize)]
struct JupyterKernelspecEntry {
    resource_dir: String,
    spec: JupyterKernelspec,
}

#[derive(Debug, Deserialize)]
struct JupyterKernelspec {
    #[serde(default)]
    argv: Vec<String>,
}

/// Lists the kernels the Jupyter bridge can actually launch.
///
/// The bridge resolves kernels with `KernelSpecManager` inside the configured
/// Python, so this asks the same interpreter the same question. Shelling out
/// to whatever `jupyter` is on PATH instead can describe a different
/// environment: a Homebrew `jupyter` sees kernels installed into its own
/// prefix that the configured Python never will, and the reverse.
/// `get_all_specs()` returns the same shape as `jupyter kernelspec list
/// --json`'s `kernelspecs` object.
const LIST_KERNELSPECS_SCRIPT: &str = "import json\n\
from jupyter_client.kernelspec import KernelSpecManager\n\
print(json.dumps({'kernelspecs': KernelSpecManager().get_all_specs()}))\n";

fn jupyter_kernels_check(python: &Path, jupyter_client_available: bool) -> HealthCheck {
    let path = python.display().to_string();
    if !jupyter_client_available {
        return HealthCheck::warn(
            "jupyter kernels",
            "skipped because jupyter_client is not available in configured Python",
        )
        .with_path(path)
        .with_hint(tools::JUPYTER_CLIENT.install_hint);
    }

    match Command::new(python)
        .args(["-c", LIST_KERNELSPECS_SCRIPT])
        .output()
    {
        Ok(output) if output.status.success() => jupyter_kernels_json_check(&output.stdout, &path),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            HealthCheck::warn(
                "jupyter kernels",
                if stderr.is_empty() {
                    "listing kernelspecs from configured Python failed".to_string()
                } else {
                    format!("listing kernelspecs from configured Python failed: {stderr}")
                },
            )
            .with_path(path)
        }
        Err(error) => HealthCheck::warn(
            "jupyter kernels",
            format!("failed to list kernelspecs from configured Python: {error}"),
        )
        .with_path(path),
    }
}

fn jupyter_kernels_json_check(stdout: &[u8], path: &str) -> HealthCheck {
    let parsed: JupyterKernelspecList = match serde_json::from_slice(stdout) {
        Ok(parsed) => parsed,
        Err(error) => {
            return HealthCheck::warn(
                "jupyter kernels",
                format!("failed to parse kernelspec list from configured Python: {error}"),
            )
            .with_path(path);
        }
    };

    let mut details = Vec::new();
    let mut warnings = 0usize;

    for (name, entry) in parsed.kernelspecs {
        match entry.spec.argv.first() {
            Some(program) => {
                let program_path = Path::new(program);
                if validate_executable(program_path, "launch Jupyter kernel", None).is_ok() {
                    details.push(format!("{name} {} -> {program}", entry.resource_dir));
                } else {
                    warnings += 1;
                    details.push(format!(
                        "{name} {} -> missing launch executable: {program}",
                        entry.resource_dir
                    ));
                }
            }
            None => {
                warnings += 1;
                details.push(format!(
                    "{name} {} -> missing argv in kernel.json",
                    entry.resource_dir
                ));
            }
        }
    }

    let total = details.len();
    let status = if total == 0 || warnings > 0 {
        HealthStatus::Warning
    } else {
        HealthStatus::Ok
    };
    let message = if total == 0 {
        "no registered kernels found".to_string()
    } else if warnings > 0 {
        format!("{total} registered kernel(s), {warnings} launch warning(s)")
    } else {
        format!("{total} registered kernel(s)")
    };

    HealthCheck::new("jupyter kernels", status, message)
        .with_path(path)
        .with_optional_hint(
            (warnings > 0)
                .then(|| "fix the kernelspec argv executable or reinstall the kernel".to_string()),
        )
        .with_details(details)
}

fn display_config_path(path: &Path) -> Result<String> {
    let path = if path.is_absolute() {
        PathBuf::from(path)
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(path.display().to_string())
}

fn print_text_report(report: &HealthReport) {
    eprintln!("Calepin health");
    eprintln!();
    eprintln!("Root: {}", report.root);
    if let Some(config) = &report.config {
        eprintln!("Config: {config}");
    }
    eprintln!();

    for check in &report.checks {
        let status = match check.status {
            HealthStatus::Ok => "OK",
            HealthStatus::Warning => "WARN",
            HealthStatus::Error => "ERROR",
        };
        let path = check.path.as_deref().unwrap_or("");
        eprintln!(
            "{status:<5} {:<16} {:<28} {}",
            check.name, path, check.message
        );
        if let Some(hint) = &check.hint {
            eprintln!("      {:<16} hint: {hint}", "");
        }
        for detail in &check.details {
            eprintln!("      {:<16} {detail}", "");
        }
    }

    let (ok, warnings, errors) = report.counts();
    eprintln!();
    eprintln!("{ok} ok, {warnings} warning(s), {errors} error(s)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernelspec_json_warns_on_missing_launch_executable() {
        let report = jupyter_kernels_json_check(
            br#"{
              "kernelspecs": {
                "ark": {
                  "resource_dir": "/tmp/kernels/ark",
                  "spec": { "argv": ["/definitely/missing/ark", "{connection_file}"] }
                }
              }
            }"#,
            "python3",
        );

        assert_eq!(report.status, HealthStatus::Warning);
        assert!(report.message.contains("1 launch warning"));
        assert!(report.details[0].contains("missing launch executable"));
    }

    /// A stand-in for the configured Python that ignores its arguments and
    /// prints `stdout`, so a test can tell which interpreter a check asked.
    #[cfg(unix)]
    fn stub_python(dir: &Path, stdout: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("python-stub");
        std::fs::write(&path, format!("#!/bin/sh\ncat <<'JSON'\n{stdout}\nJSON\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[cfg(unix)]
    #[test]
    fn kernels_check_lists_the_kernels_the_configured_python_sees() {
        // The bridge launches kernels through the configured Python, so the
        // health report must describe that interpreter's kernels, not those
        // of whatever `jupyter` happens to be on PATH.
        let dir = tempfile::tempdir().unwrap();
        let python = stub_python(
            dir.path(),
            r#"{"kernelspecs": {"julia-1.12": {"resource_dir": "/kernels/julia-1.12",
               "spec": {"argv": ["/bin/sh", "{connection_file}"]}}}}"#,
        );

        let check = jupyter_kernels_check(&python, true);

        assert_eq!(check.status, HealthStatus::Ok, "{check:?}");
        assert_eq!(
            check.path.as_deref(),
            Some(python.display().to_string().as_str())
        );
        assert_eq!(check.details.len(), 1, "{check:?}");
        assert!(check.details[0].starts_with("julia-1.12 "), "{check:?}");
    }

    #[test]
    fn kernels_check_is_skipped_when_jupyter_client_is_missing() {
        let check = jupyter_kernels_check(Path::new("definitely-missing-python"), false);

        assert_eq!(check.status, HealthStatus::Warning);
        assert!(check.message.contains("jupyter_client"), "{check:?}");
    }

    #[test]
    fn tool_check_marks_missing_required_tools_as_errors() {
        let check = tool_check(
            "missing",
            Path::new("definitely-missing-calepin-tool"),
            None,
            true,
            "test health",
        );

        assert_eq!(check.status, HealthStatus::Error);
    }

    #[test]
    fn tool_check_marks_missing_optional_tools_as_warnings() {
        let check = tool_check(
            "missing",
            Path::new("definitely-missing-calepin-tool"),
            None,
            false,
            "test health",
        );

        assert_eq!(check.status, HealthStatus::Warning);
    }

    #[test]
    fn report_counts_statuses() {
        let report = HealthReport {
            root: "/tmp/project".to_string(),
            config: None,
            checks: vec![
                HealthCheck::ok("ok", ""),
                HealthCheck::warn("warn", ""),
                HealthCheck::error("err", ""),
            ],
        };

        assert_eq!(report.counts(), (1, 1, 1));
        assert!(report.has_warnings());
        assert!(report.has_errors());
    }
}
