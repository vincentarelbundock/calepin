use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::cli::HealthArgs;
use crate::config::CalepinConfig;
use crate::typst::version::{typst_version, version_is_too_old, REQUIRED_TYPST_VERSION};
use crate::utils::process::validate_executable;
use crate::utils::tools::{self, Tool};

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
    let report = build_report(args.config.as_deref())?;

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

pub fn build_report(config_path: Option<&Path>) -> Result<HealthReport> {
    let root = std::env::current_dir()?;
    let config = CalepinConfig::load(&root, config_path)?;
    let mut checks = Vec::new();

    checks.push(typst_check(
        "typst",
        &config.executables.typst,
        Some(&tools::TYPST),
        true,
        "render Typst documents",
    ));
    checks.push(tool_check(
        "python",
        &config.executables.python,
        Some(&tools::PYTHON),
        true,
        "execute Python chunks",
    ));
    checks.push(tool_check(
        "Rscript",
        &config.executables.rscript,
        Some(&tools::RSCRIPT),
        false,
        "execute R chunks",
    ));
    checks.push(tool_check(
        "mmdc",
        &config.executables.mmdc,
        Some(&tools::MMDC),
        false,
        "render Mermaid diagrams",
    ));
    checks.push(tool_check(
        "dot",
        &config.executables.dot,
        Some(&tools::DOT),
        false,
        "render Graphviz DOT diagrams",
    ));
    checks.push(tool_check(
        "d2",
        &config.executables.d2,
        Some(&tools::D2),
        false,
        "render D2 diagrams",
    ));
    checks.push(tool_check(
        "tectonic",
        &config.executables.tectonic,
        Some(&tools::TECTONIC),
        false,
        "render TikZ diagrams",
    ));
    checks.push(tool_check(
        "dvisvgm",
        &config.executables.dvisvgm,
        Some(&tools::DVISVGM),
        false,
        "convert TikZ output to SVG",
    ));
    checks.push(tool_check(
        "pdf2svg",
        &config.executables.pdf2svg,
        Some(&tools::PDF2SVG),
        false,
        "convert PDFs to SVG",
    ));

    let python_available = checks
        .iter()
        .find(|check| check.name == "python")
        .is_some_and(|check| check.status == HealthStatus::Ok);
    checks.push(jupyter_client_check(
        &config.executables.python,
        python_available,
    ));
    checks.push(jupyter_kernels_check());

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
    match validate_executable(path, action, tool) {
        Ok(()) => HealthCheck {
            name: name.to_string(),
            status: HealthStatus::Ok,
            path: Some(path.display().to_string()),
            message: "found".to_string(),
            hint: None,
            details: Vec::new(),
        },
        Err(error) => HealthCheck {
            name: name.to_string(),
            status: if required {
                HealthStatus::Error
            } else {
                HealthStatus::Warning
            },
            path: Some(path.display().to_string()),
            message: error.to_string(),
            hint: tool.map(|tool| tool.install_hint.to_string()),
            details: Vec::new(),
        },
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
        Ok(version) if version_is_too_old(&version) => HealthCheck {
            name: name.to_string(),
            status: HealthStatus::Error,
            path: Some(path.display().to_string()),
            message: format!(
                "Typst {version} is too old; Calepin requires Typst {REQUIRED_TYPST_VERSION} or newer"
            ),
            hint: Some(tools::TYPST.install_hint.to_string()),
            details: Vec::new(),
        },
        Ok(version) => {
            check.message = format!("found Typst {version}");
            check
        }
        Err(error) => HealthCheck {
            name: name.to_string(),
            status: HealthStatus::Error,
            path: Some(path.display().to_string()),
            message: error.to_string(),
            hint: Some(tools::TYPST.install_hint.to_string()),
            details: Vec::new(),
        },
    }
}

fn jupyter_client_check(python: &Path, python_available: bool) -> HealthCheck {
    if !python_available {
        return HealthCheck {
            name: "jupyter_client".to_string(),
            status: HealthStatus::Warning,
            path: Some(python.display().to_string()),
            message: "skipped because Python is not available".to_string(),
            hint: Some(tools::JUPYTER_CLIENT.install_hint.to_string()),
            details: Vec::new(),
        };
    }

    match Command::new(python)
        .args(["-c", "import jupyter_client"])
        .output()
    {
        Ok(output) if output.status.success() => HealthCheck {
            name: "jupyter_client".to_string(),
            status: HealthStatus::Ok,
            path: Some(python.display().to_string()),
            message: "available in configured Python".to_string(),
            hint: None,
            details: Vec::new(),
        },
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            HealthCheck {
                name: "jupyter_client".to_string(),
                status: HealthStatus::Warning,
                path: Some(python.display().to_string()),
                message: if stderr.is_empty() {
                    "not importable from configured Python".to_string()
                } else {
                    format!("not importable from configured Python: {stderr}")
                },
                hint: Some(tools::JUPYTER_CLIENT.install_hint.to_string()),
                details: Vec::new(),
            }
        }
        Err(error) => HealthCheck {
            name: "jupyter_client".to_string(),
            status: HealthStatus::Warning,
            path: Some(python.display().to_string()),
            message: format!("failed to check jupyter_client: {error}"),
            hint: Some(tools::JUPYTER_CLIENT.install_hint.to_string()),
            details: Vec::new(),
        },
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

fn jupyter_kernels_check() -> HealthCheck {
    let jupyter = Path::new("jupyter");
    if let Err(error) = validate_executable(jupyter, "list registered Jupyter kernels", None) {
        return HealthCheck {
            name: "jupyter kernels".to_string(),
            status: HealthStatus::Warning,
            path: Some("jupyter".to_string()),
            message: error.to_string(),
            hint: Some("install Jupyter or ensure the `jupyter` command is on PATH".to_string()),
            details: Vec::new(),
        };
    }

    match Command::new(jupyter)
        .args(["kernelspec", "list", "--json"])
        .output()
    {
        Ok(output) if output.status.success() => jupyter_kernels_json_check(&output.stdout),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            HealthCheck {
                name: "jupyter kernels".to_string(),
                status: HealthStatus::Warning,
                path: Some("jupyter".to_string()),
                message: if stderr.is_empty() {
                    "`jupyter kernelspec list --json` failed".to_string()
                } else {
                    format!("`jupyter kernelspec list --json` failed: {stderr}")
                },
                hint: Some("run `jupyter kernelspec list --json` for details".to_string()),
                details: Vec::new(),
            }
        }
        Err(error) => HealthCheck {
            name: "jupyter kernels".to_string(),
            status: HealthStatus::Warning,
            path: Some("jupyter".to_string()),
            message: format!("failed to run `jupyter kernelspec list --json`: {error}"),
            hint: Some("install Jupyter or ensure the `jupyter` command is on PATH".to_string()),
            details: Vec::new(),
        },
    }
}

fn jupyter_kernels_json_check(stdout: &[u8]) -> HealthCheck {
    let parsed: JupyterKernelspecList = match serde_json::from_slice(stdout) {
        Ok(parsed) => parsed,
        Err(error) => {
            return HealthCheck {
                name: "jupyter kernels".to_string(),
                status: HealthStatus::Warning,
                path: Some("jupyter".to_string()),
                message: format!("failed to parse `jupyter kernelspec list --json`: {error}"),
                hint: Some("run `jupyter kernelspec list --json` for details".to_string()),
                details: Vec::new(),
            }
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

    HealthCheck {
        name: "jupyter kernels".to_string(),
        status,
        path: Some("jupyter".to_string()),
        message,
        hint: (warnings > 0)
            .then(|| "fix the kernelspec argv executable or reinstall the kernel".to_string()),
        details,
    }
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
            &check.name, path, &check.message
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
        );

        assert_eq!(report.status, HealthStatus::Warning);
        assert!(report.message.contains("1 launch warning"));
        assert!(report.details[0].contains("missing launch executable"));
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
                HealthCheck {
                    name: "ok".to_string(),
                    status: HealthStatus::Ok,
                    path: None,
                    message: String::new(),
                    hint: None,
                    details: Vec::new(),
                },
                HealthCheck {
                    name: "warn".to_string(),
                    status: HealthStatus::Warning,
                    path: None,
                    message: String::new(),
                    hint: None,
                    details: Vec::new(),
                },
                HealthCheck {
                    name: "err".to_string(),
                    status: HealthStatus::Error,
                    path: None,
                    message: String::new(),
                    hint: None,
                    details: Vec::new(),
                },
            ],
        };

        assert_eq!(report.counts(), (1, 1, 1));
        assert!(report.has_warnings());
        assert!(report.has_errors());
    }
}
