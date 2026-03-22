mod reload;
mod server;
mod watcher;

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::{Context, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::cli::PreviewArgs;

pub fn run(input: &Path, args: &PreviewArgs) -> Result<()> {
    // Canonicalize for the file watcher (needs absolute paths for event matching),
    // but keep the original relative path for rendering (so figure paths stay relative).
    let input_abs = input.canonicalize()
        .with_context(|| format!("Input file not found: {}", input.display()))?;
    let input = input;

    // Determine the effective format
    let format = resolve_preview_format(args, input)?;

    match format.as_str() {
        "latex" | "typst" => run_pdf_preview(input, &input_abs, args, &format),
        _ => run_html_preview(input, &input_abs, args),
    }
}

/// Preview for HTML: serve with live-reload.
fn run_html_preview(input: &Path, input_abs: &Path, args: &PreviewArgs) -> Result<()> {
    let serve_dir = input_abs.parent().unwrap().to_path_buf();

    let version = Arc::new(AtomicU64::new(1));

    let mp = MultiProgress::new();

    let style = ProgressStyle::default_spinner()
        .template("{spinner:.cyan} {msg}")
        .unwrap();

    let status = mp.add(ProgressBar::new_spinner());
    status.set_style(style.clone());

    let spinner = mp.add(ProgressBar::new_spinner());
    spinner.set_style(style);
    spinner.enable_steady_tick(Duration::from_millis(80));

    // Initial render
    spinner.set_message("rendering...");
    let html = render_html(input, &args.overrides)?;
    let html = reload::inject_reload_script(&html, version.load(Ordering::Relaxed));

    let content = Arc::new(RwLock::new(html));

    // Start HTTP server in background
    let (_server, actual_port) = server::start(
        args.port,
        Arc::clone(&content),
        Arc::clone(&version),
        serve_dir,
    )?;

    let url = format!("http://localhost:{}", actual_port);

    if !args.quiet {
        mp.println(format!("→ preview at {}", url)).ok();
    }

    // Open browser
    let _ = open::that(&url);

    // Set up Ctrl+C handler
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    ctrlc::set_handler(move || {
        stop_clone.store(true, Ordering::Relaxed);
    }).context("Failed to set Ctrl+C handler")?;

    // Show watching status
    status.set_message(format!("built at {}", local_time_str()));
    spinner.set_message("watching for changes... (Ctrl+C to stop)");

    // Watch for changes and re-render
    let overrides = args.overrides.clone();
    let stop_clone = Arc::clone(&stop);
    let quiet = args.quiet;
    watcher::watch(input_abs, stop_clone, || {
        spinner.set_message("rebuilding...");
        let start = std::time::Instant::now();
        match render_html(input, &overrides) {
            Ok(html) => {
                let elapsed = start.elapsed();
                let v = version.fetch_add(1, Ordering::Relaxed) + 1;
                let html = reload::inject_reload_script(&html, v);
                *content.write().unwrap() = html;
                status.set_message(format!("rebuilt at {} ({:.1}s)", local_time_str(), elapsed.as_secs_f64()));
                spinner.set_message("watching for changes... (Ctrl+C to stop)");
            }
            Err(e) => {
                if !quiet {
                    mp.println(format!("\x1b[33mWarning:\x1b[0m rebuild failed: {}", e)).ok();
                }
                spinner.set_message("watching for changes... (Ctrl+C to stop)");
            }
        }
    })?;

    // Clean exit
    status.finish_and_clear();
    spinner.finish_with_message("stopped.");

    Ok(())
}

/// Preview for LaTeX/Typst: render, compile to PDF, serve in browser with reload.
fn run_pdf_preview(input: &Path, input_abs: &Path, args: &PreviewArgs, format: &str) -> Result<()> {
    let serve_dir = input_abs.parent().unwrap().to_path_buf();

    let version = Arc::new(AtomicU64::new(1));

    let mp = MultiProgress::new();

    let style = ProgressStyle::default_spinner()
        .template("{spinner:.cyan} {msg}")
        .unwrap();

    let status = mp.add(ProgressBar::new_spinner());
    status.set_style(style.clone());

    let spinner = mp.add(ProgressBar::new_spinner());
    spinner.set_style(style);
    spinner.enable_steady_tick(Duration::from_millis(80));

    // Initial render + compile
    spinner.set_message("rendering...");
    let pdf_path = render_and_compile(input, format, &args.overrides, args.quiet)?;

    // Build the PDF viewer HTML wrapper
    let pdf_filename = pdf_path.file_name().unwrap().to_string_lossy().to_string();
    let html = pdf_viewer_html(&pdf_filename, version.load(Ordering::Relaxed));
    let content = Arc::new(RwLock::new(html));

    // Start HTTP server in background
    let (_server, actual_port) = server::start(
        args.port,
        Arc::clone(&content),
        Arc::clone(&version),
        serve_dir,
    )?;

    let url = format!("http://localhost:{}", actual_port);

    if !args.quiet {
        mp.println(format!("→ preview at {}", url)).ok();
    }

    // Open browser
    let _ = open::that(&url);

    // Set up Ctrl+C handler
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    ctrlc::set_handler(move || {
        stop_clone.store(true, Ordering::Relaxed);
    }).context("Failed to set Ctrl+C handler")?;

    // Show watching status
    status.set_message(format!("built at {}", local_time_str()));
    spinner.set_message("watching for changes... (Ctrl+C to stop)");

    // Watch for changes and re-render + recompile
    let overrides = args.overrides.clone();
    let format = format.to_string();
    let quiet = args.quiet;
    watcher::watch(input_abs, Arc::clone(&stop), || {
        spinner.set_message("rebuilding...");
        let start = std::time::Instant::now();
        match render_and_compile(input, &format, &overrides, quiet) {
            Ok(_) => {
                let elapsed = start.elapsed();
                let v = version.fetch_add(1, Ordering::Relaxed) + 1;
                *content.write().unwrap() = pdf_viewer_html(&pdf_filename, v);
                status.set_message(format!("rebuilt at {} ({:.1}s)", local_time_str(), elapsed.as_secs_f64()));
                spinner.set_message("watching for changes... (Ctrl+C to stop)");
            }
            Err(e) => {
                if !quiet {
                    mp.println(format!("\x1b[33mWarning:\x1b[0m rebuild failed: {}", e)).ok();
                }
                spinner.set_message("watching for changes... (Ctrl+C to stop)");
            }
        }
    })?;

    // Clean exit
    status.finish_and_clear();
    spinner.finish_with_message("stopped.");

    Ok(())
}

/// Generate an HTML page that embeds a PDF with live-reload.
fn pdf_viewer_html(pdf_filename: &str, version: u64) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>{filename}</title>
<style>
  html, body {{ margin: 0; padding: 0; height: 100%; overflow: hidden; }}
  iframe {{ width: 100%; height: 100%; border: none; }}
</style>
</head>
<body data-version="{version}">
<iframe src="{filename}?v={version}"></iframe>
<script>
(function() {{
  var lastVersion = "{version}";
  setInterval(function() {{
    fetch('/__version').then(function(r) {{ return r.text(); }}).then(function(v) {{
      if (v !== lastVersion) {{ location.reload(); }}
    }}).catch(function() {{}});
  }}, 500);
}})();
</script>
</body>
</html>"#,
        filename = pdf_filename,
        version = version
    )
}

/// Render to LaTeX/Typst, write the file, compile to PDF. Returns the PDF path.
/// Always compiles quietly — the spinner shows status instead.
fn render_and_compile(input: &Path, format: &str, overrides: &[String], _quiet: bool) -> Result<std::path::PathBuf> {
    let (output_path, content, renderer) = crate::render_file(input, None, Some(format), overrides, None, None)?;
    renderer.write_output(&content, &output_path)?;
    crate::compile::compile_to_pdf(&output_path, true)?;
    Ok(output_path.with_extension("pdf"))
}

/// Detect the format for preview from CLI flags or YAML front matter.
fn resolve_preview_format(args: &PreviewArgs, input: &Path) -> Result<String> {
    if let Some(ref fmt) = args.target {
        return Ok(fmt.clone());
    }
    // Check YAML front matter
    let text = fs::read_to_string(input)
        .with_context(|| format!("Failed to read {}", input.display()))?;
    let (metadata, _) = crate::parse::yaml::split_yaml(&text)?;
    Ok(metadata.target.unwrap_or_else(|| "html".to_string()))
}

fn local_time_str() -> String {
    String::from_utf8(
        std::process::Command::new("date")
            .arg("+%H:%M:%S")
            .output()
            .map(|o| o.stdout)
            .unwrap_or_default()
    ).unwrap_or_default().trim().to_string()
}

fn render_html(input: &Path, overrides: &[String]) -> Result<String> {
    let (_path, html, _renderer) = crate::render_file(input, None, Some("html"), overrides, None, None)?;
    Ok(html)
}

