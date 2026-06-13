use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
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
    checks.push(link_check(&root));

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct LinkOccurrence {
    source: PathBuf,
    line: usize,
    target: String,
}

const LINK_CHECK_SKIP_DIRS: &[&str] = &[".calepin", ".git", "target", "node_modules", ".venv"];

fn link_check(root: &Path) -> HealthCheck {
    match check_links(root) {
        Ok(LinkSummary {
            files,
            links,
            broken,
        }) => {
            let status = if broken.is_empty() {
                HealthStatus::Ok
            } else {
                HealthStatus::Error
            };
            let message = if broken.is_empty() {
                format!("checked {links} literal link(s) in {files} Typst file(s)")
            } else {
                format!(
                    "{} broken link(s) among {links} literal link(s) in {files} Typst file(s)",
                    broken.len()
                )
            };
            HealthCheck {
                name: "links".to_string(),
                status,
                path: Some(root.display().to_string()),
                message,
                hint: (!broken.is_empty()).then(|| {
                    "fix missing local link targets or rebuild generated linked outputs".to_string()
                }),
                details: broken,
            }
        }
        Err(error) => HealthCheck {
            name: "links".to_string(),
            status: HealthStatus::Warning,
            path: Some(root.display().to_string()),
            message: format!("failed to check links: {error}"),
            hint: None,
            details: Vec::new(),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LinkSummary {
    files: usize,
    links: usize,
    broken: Vec<String>,
}

fn check_links(root: &Path) -> Result<LinkSummary> {
    let typ_files = collect_typst_files(root)?;
    let mut links = 0usize;
    let mut broken = Vec::new();

    for file in &typ_files {
        let source = fs::read_to_string(file)
            .with_context(|| format!("failed to read {}", file.display()))?;
        for link in extract_literal_links(file, &source) {
            links += 1;
            if let Some(message) = validate_local_link(root, &link) {
                broken.push(message);
            }
        }
    }

    Ok(LinkSummary {
        files: typ_files.len(),
        links,
        broken,
    })
}

fn collect_typst_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect_typst_files_in(root, root, &mut out)?;
    out.sort();
    Ok(out)
}

fn collect_typst_files_in(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        if rel.components().any(|component| {
            component
                .as_os_str()
                .to_str()
                .is_some_and(|name| LINK_CHECK_SKIP_DIRS.contains(&name))
        }) {
            continue;
        }
        if path.is_dir() {
            collect_typst_files_in(root, &path, out)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("typ") {
            out.push(path);
        }
    }
    Ok(())
}

fn extract_literal_links(source_path: &Path, source: &str) -> Vec<LinkOccurrence> {
    let mut links = Vec::new();
    let mut offset = 0usize;
    while let Some(relative) = source[offset..].find("#link") {
        let start = offset + relative;
        let after_name = start + "#link".len();
        if source[after_name..]
            .chars()
            .next()
            .is_some_and(is_identifier_char)
        {
            offset = after_name;
            continue;
        }
        let open = skip_ws(source, after_name);
        if source[open..].chars().next() != Some('(') {
            offset = after_name;
            continue;
        }
        let value_start = skip_ws(source, open + 1);
        if source[value_start..].chars().next() != Some('"') {
            offset = value_start;
            continue;
        }
        match parse_string_literal(source, value_start) {
            Some((target, end)) => {
                links.push(LinkOccurrence {
                    source: source_path.to_path_buf(),
                    line: line_number(source, start),
                    target,
                });
                offset = end;
            }
            None => {
                offset = value_start + 1;
            }
        }
    }
    links
}

fn validate_local_link(root: &Path, link: &LinkOccurrence) -> Option<String> {
    let target = link.target.trim();
    if target.is_empty() || is_external_or_special_link(target) {
        return None;
    }
    let path_part = target
        .split_once('#')
        .map(|(path, _)| path)
        .unwrap_or(target)
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or_else(|| {
            target
                .split_once('#')
                .map(|(path, _)| path)
                .unwrap_or(target)
        })
        .trim();
    if path_part.is_empty() {
        return None;
    }

    let base = if path_part.starts_with('/') {
        root.to_path_buf()
    } else {
        link.source.parent().unwrap_or(root).to_path_buf()
    };
    let candidate = normalize_path(&base.join(path_part.trim_start_matches('/')));
    if !candidate.starts_with(root) {
        return Some(format!(
            "{}:{} `{}` escapes the project root",
            display_rel(root, &link.source),
            link.line,
            link.target
        ));
    }
    if link_target_exists(&candidate) {
        return None;
    }

    Some(format!(
        "{}:{} missing local link target `{}`",
        display_rel(root, &link.source),
        link.line,
        link.target
    ))
}

fn link_target_exists(candidate: &Path) -> bool {
    if candidate.exists() {
        return true;
    }
    if candidate
        .extension()
        .and_then(|extension| extension.to_str())
        == Some("html")
    {
        let mut typ_source = candidate.to_path_buf();
        typ_source.set_extension("typ");
        return typ_source.is_file();
    }
    if candidate.extension().is_none() {
        return candidate.join("index.typ").is_file() || candidate.join("index.html").is_file();
    }
    false
}

fn is_external_or_special_link(target: &str) -> bool {
    target.starts_with('#')
        || target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("//")
        || target.starts_with("mailto:")
        || target.starts_with("tel:")
        || target.starts_with("data:")
}

fn parse_string_literal(source: &str, quote: usize) -> Option<(String, usize)> {
    let mut out = String::new();
    let mut escaped = false;
    let mut index = quote + 1;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        if escaped {
            out.push(match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some((out, index + ch.len_utf8()));
        } else {
            out.push(ch);
        }
        index += ch.len_utf8();
    }
    None
}

fn skip_ws(value: &str, mut index: usize) -> usize {
    while index < value.len() {
        let Some(ch) = value[index..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'
}

fn line_number(source: &str, offset: usize) -> usize {
    source[..offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

fn normalize_path(path: &Path) -> PathBuf {
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

fn display_rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
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
    fn extracts_literal_typst_links() {
        let links = extract_literal_links(
            Path::new("doc.typ"),
            r#"
#link("guide.html")[Guide]
#link(dynamic)[Dynamic]
#link(
  "../assets/logo.svg"
)[Logo]
"#,
        );

        assert_eq!(
            links
                .iter()
                .map(|link| (link.line, link.target.as_str()))
                .collect::<Vec<_>>(),
            vec![(2, "guide.html"), (4, "../assets/logo.svg")]
        );
    }

    #[test]
    fn link_check_accepts_html_target_with_typst_source() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("index.typ"), r#"#link("guide.html")[Guide]"#).unwrap();
        fs::write(root.join("guide.typ"), "= Guide\n").unwrap();

        let summary = check_links(root).unwrap();

        assert_eq!(summary.links, 1);
        assert!(summary.broken.is_empty());
    }

    #[test]
    fn link_check_reports_missing_local_targets() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("index.typ"), r#"#link("missing.html")[Missing]"#).unwrap();

        let summary = check_links(root).unwrap();

        assert_eq!(summary.links, 1);
        assert_eq!(summary.broken.len(), 1);
        assert!(summary.broken[0].contains("index.typ:1"));
        assert!(summary.broken[0].contains("missing.html"));
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
