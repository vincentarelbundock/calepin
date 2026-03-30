//! The `calepin preview` command: live-preview documents and collections.

use anyhow::Result;
use crate::cli::PreviewArgs;

pub fn handle_preview(args: PreviewArgs) -> Result<()> {
    // `calepin preview kill` -- kill all running preview servers
    if args.input.as_os_str() == "kill" {
        return kill_preview_servers();
    }

    use crate::paths::ProjectKind;

    match ProjectKind::discover(&args.input)? {
        ProjectKind::Collection { config, project_dir, .. } => {
            let meta = crate::config::load_project_metadata(&config)?;
            let output = crate::paths::output_dir(&project_dir, meta.output.as_deref());

            // Read target from the input .qmd front matter or CLI -t
            let target_name = args.format.clone()
                .or_else(|| read_target_from_qmd(&args.input))
                .unwrap_or_else(|| "html".to_string());
            let target = crate::config::resolve_target(&target_name, &meta.targets)?;

            // Non-HTML targets: one-shot build and open
            if target.writer != "html" {
                crate::collection::build_collection(Some(config.as_path()), &output, true, false, Some(&target_name), false, true)?;
                let pdf = output.join("book.pdf");
                if pdf.exists() {
                    eprintln!("Opening {}", pdf.display());
                    let _ = open::that(&pdf);
                }
                return Ok(());
            }

            crate::preview::run_collection(&config, &args)
        }
        ProjectKind::Document { qmd, .. } => {
            let ctx = crate::resolve_context(&qmd, args.format.as_deref())?;
            crate::preview::run(&qmd, &args, &ctx.target_name, &ctx.target)
        }
    }
}

/// Read the `target` field from a .qmd file's TOML front matter.
fn read_target_from_qmd(path: &std::path::Path) -> Option<String> {
    if !path.extension().and_then(|e| e.to_str()).is_some_and(|e| e == "qmd") {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    let (fm, _) = crate::config::split_frontmatter(&text).ok()?;
    fm.target
}

/// Kill all running calepin preview servers (macOS/Linux).
fn kill_preview_servers() -> Result<()> {
    let output = std::process::Command::new("pgrep")
        .args(["-f", "calepin preview"])
        .output()?;

    let own_pid = std::process::id();
    let pids: Vec<u32> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .filter(|&pid| pid != own_pid)
        .collect();

    if pids.is_empty() {
        eprintln!("No running preview servers found.");
        return Ok(());
    }

    let mut killed = 0;
    for pid in &pids {
        let result = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status();
        match result {
            Ok(s) if s.success() => {
                eprintln!("Killed preview server (PID {})", pid);
                killed += 1;
            }
            _ => eprintln!("Failed to kill PID {}", pid),
        }
    }

    if killed > 0 {
        eprintln!("Stopped {} preview server(s).", killed);
    }
    Ok(())
}
