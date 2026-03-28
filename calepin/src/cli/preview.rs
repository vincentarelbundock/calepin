//! The `calepin preview` command: live-preview documents and collections.

use std::path::PathBuf;
use anyhow::Result;
use crate::cli::PreviewArgs;

pub fn handle_preview(args: PreviewArgs) -> Result<()> {
    // `calepin preview kill` -- kill all running preview servers
    if args.input.as_os_str() == "kill" {
        return kill_preview_servers();
    }

    // Directory: check for project config inside, otherwise serve statically
    if args.input.is_dir() {
        if let Some(config) = crate::cli::find_project_config(&args.input) {
            let args = PreviewArgs { input: config, ..args };
            return handle_preview(args);
        }
        eprintln!("Serving static files from: {}", args.input.display());
        return crate::collection::serve(&args.input, args.port);
    }
    // Project manifest: build, serve with live-reload, and watch for changes
    if crate::cli::is_collection_config(&args.input) {
        // For non-HTML targets, do a one-shot build and open the output.
        let is_html = {
            let target_name = args.format.as_deref().unwrap_or("html");
            let meta = crate::config::load_project_metadata(&args.input)?;
            let target = crate::config::resolve_target(target_name, &meta.targets)?;
            target.writer == "html"
        };
        if !is_html {
            let output = PathBuf::from(crate::paths::DEFAULT_OUTPUT_DIR);
            crate::collection::build_collection(Some(args.input.as_path()), &output, true, false, args.format.as_deref(), false, true)?;
            let pdf = output.join("book.pdf");
            if pdf.exists() {
                eprintln!("Opening {}", pdf.display());
                let _ = open::that(&pdf);
            }
            return Ok(());
        }

        return crate::preview::run_collection(&args.input, &args);
    }
    // Resolve target using the same path as render
    let ctx = crate::resolve_context(&args.input, args.format.as_deref())?;
    crate::preview::run(&args.input, &args, &ctx.target_name, &ctx.target)
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
