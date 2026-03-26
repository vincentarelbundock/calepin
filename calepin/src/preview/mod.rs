mod reload;
pub(crate) mod server;
mod watcher;

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::{Context, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::cli::PreviewArgs;

/// Preview a collection project: build, serve with live-reload, and watch for changes.
pub fn run_collection(
    config_path: &Path,
    args: &PreviewArgs,
) -> Result<()> {
    let config_abs = config_path.canonicalize()
        .with_context(|| format!("Config file not found: {}", config_path.display()))?;
    let cwd = std::env::current_dir()?;
    let base_dir = crate::paths::resolve_project_root(&config_abs, &cwd);

    // Read output dir from config (defaults to "output")
    let meta = crate::config::load_project_metadata(&config_abs)?;
    let output_name = meta.output.as_deref().unwrap_or("output");
    let output = base_dir.join(output_name);

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

    // Initial build — pause spinner so per-file progress prints cleanly
    spinner.finish_and_clear();
    crate::collection::build_collection(
        Some(config_path),
        &std::path::PathBuf::from(output_name),
        true,
        false,
        args.format.as_deref(),
    )?;
    spinner.reset();
    spinner.enable_steady_tick(Duration::from_millis(80));

    // Start collection server (serves from disk with live-reload)
    let (server_handle, actual_port) = server::start_collection(
        args.port,
        Arc::clone(&version),
        output.clone(),
    )?;

    let url = format!("http://localhost:{}", actual_port);
    if !args.quiet {
        mp.println(format!("-> preview at {}", url)).ok();
    }
    let _ = open::that(&url);

    // Ctrl+C handler: signal stop and unblock the server so the port is released.
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    let server_for_ctrlc = server_handle;
    ctrlc::set_handler(move || {
        stop_clone.store(true, Ordering::Relaxed);
        server_for_ctrlc.shutdown();
    }).context("Failed to set Ctrl+C handler")?;

    status.set_message(format!("built at {}", format_local_time()));
    spinner.set_message("watching for changes... (Ctrl+C to stop)");

    let watch_dir = base_dir.clone();

    let config_path = config_path.to_path_buf();
    let target = args.format.clone();
    let quiet = args.quiet;
    watcher::watch_dir(&watch_dir, Arc::clone(&stop), Some(output.as_path()), |changed_paths| {
        let names: Vec<_> = changed_paths.iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .collect();
        spinner.set_message(format!("rebuilding {}...", names.join(", ")));
        let start = std::time::Instant::now();
        let result = crate::collection::rebuild_documents(
            Some(config_path.as_path()),
            target.as_deref(),
            changed_paths,
        );
        match result {
            Ok(()) => {
                version.fetch_add(1, Ordering::Relaxed);
                let elapsed = start.elapsed();
                status.set_message(format!("rebuilt {} at {} ({:.1}s)", names.join(", "), format_local_time(), elapsed.as_secs_f64()));
            }
            Err(e) => {
                if !quiet {
                    mp.println(format!("\x1b[33mWarning:\x1b[0m rebuild failed: {}", e)).ok();
                }
            }
        }
        spinner.set_message("watching for changes... (Ctrl+C to stop)");
    })?;

    status.finish_and_clear();
    spinner.finish_with_message("stopped.");
    Ok(())
}

pub fn run(
    input: &Path,
    args: &PreviewArgs,
    target_name: &str,
    target: &crate::config::Target,
) -> Result<()> {
    let input_abs = input.canonicalize()
        .with_context(|| format!("Input file not found: {}", input.display()))?;

    match target.engine.as_str() {
        "latex" | "typst" => run_preview(input, &input_abs, args, PreviewMode::Pdf(target_name)),
        _ => run_preview(input, &input_abs, args, PreviewMode::Html),
    }
}

// ---------------------------------------------------------------------------
// Preview mode
// ---------------------------------------------------------------------------

enum PreviewMode<'a> {
    Html,
    Pdf(&'a str),
}

/// Shared preview loop: initial render, serve, watch, rebuild.
fn run_preview(input: &Path, input_abs: &Path, args: &PreviewArgs, mode: PreviewMode) -> Result<()> {
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
    let (html, rebuild_state) = match mode {
        PreviewMode::Html => {
            let html = render_file_html(input, &args.overrides)?;
            let html = reload::inject_reload_script(&html, version.load(Ordering::Relaxed));
            (html, RebuildState::Html)
        }
        PreviewMode::Pdf(target_name) => {
            let pdf_path = render_and_compile(input, target_name, &args.overrides)?;
            let pdf_filename = pdf_path.file_name().unwrap().to_string_lossy().to_string();
            let html = build_pdf_viewer_html(&pdf_filename, version.load(Ordering::Relaxed));
            (html, RebuildState::Pdf {
                target_name: target_name.to_string(),
                pdf_filename,
            })
        }
    };
    let content = Arc::new(RwLock::new(html));

    // Start HTTP server
    let (server_handle, actual_port) = server::start(
        args.port,
        Arc::clone(&content),
        Arc::clone(&version),
        serve_dir,
    )?;

    let url = format!("http://localhost:{}", actual_port);
    if !args.quiet {
        mp.println(format!("-> preview at {}", url)).ok();
    }
    let _ = open::that(&url);

    // Ctrl+C handler: signal stop and unblock the server so the port is released.
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);
    let server_for_ctrlc = server_handle;
    ctrlc::set_handler(move || {
        stop_clone.store(true, Ordering::Relaxed);
        server_for_ctrlc.shutdown();
    }).context("Failed to set Ctrl+C handler")?;

    status.set_message(format!("built at {}", format_local_time()));
    spinner.set_message("watching for changes... (Ctrl+C to stop)");

    // Watch and rebuild
    let overrides = args.overrides.clone();
    let quiet = args.quiet;
    watcher::watch(input_abs, Arc::clone(&stop), || {
        spinner.set_message("rebuilding...");
        let start = std::time::Instant::now();
        let result = match &rebuild_state {
            RebuildState::Html => {
                render_file_html(input, &overrides).map(|html| {
                    let v = version.fetch_add(1, Ordering::Relaxed) + 1;
                    *content.write().unwrap() = reload::inject_reload_script(&html, v);
                })
            }
            RebuildState::Pdf { target_name, pdf_filename } => {
                render_and_compile(input, target_name, &overrides).map(|_| {
                    let v = version.fetch_add(1, Ordering::Relaxed) + 1;
                    *content.write().unwrap() = build_pdf_viewer_html(pdf_filename, v);
                })
            }
        };
        match result {
            Ok(()) => {
                let elapsed = start.elapsed();
                status.set_message(format!("rebuilt at {} ({:.1}s)", format_local_time(), elapsed.as_secs_f64()));
            }
            Err(e) => {
                if !quiet {
                    mp.println(format!("\x1b[33mWarning:\x1b[0m rebuild failed: {}", e)).ok();
                }
            }
        }
        spinner.set_message("watching for changes... (Ctrl+C to stop)");
    })?;

    status.finish_and_clear();
    spinner.finish_with_message("stopped.");
    Ok(())
}

/// State needed to rebuild on file change, captured once at startup.
enum RebuildState {
    Html,
    Pdf { target_name: String, pdf_filename: String },
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

fn render_file_html(input: &Path, overrides: &[String]) -> Result<String> {
    let (_path, html, _renderer) = crate::pipeline::render_file(input, None, Some("html"), overrides, None, None, None, None)?;
    Ok(html)
}

/// Render to LaTeX/Typst, write the file, compile if the target defines it.
/// Returns the final output path (PDF if compiled, rendered file otherwise).
fn render_and_compile(input: &Path, target_name: &str, overrides: &[String]) -> Result<std::path::PathBuf> {
    let target = crate::config::resolve_target(target_name, &std::collections::HashMap::new())?;
    let (output_path, content, renderer) = crate::pipeline::render_file(
        input, None, Some(target_name), overrides, Some(&target), None, None, None,
    )?;
    renderer.write_output(&content, &output_path)?;

    let needs_compile = target.compile.is_some()
        || crate::paths::engine_to_ext(&target.engine) != target.output_extension();
    if needs_compile {
        let cmd = target.compile.as_deref().unwrap_or("");
        let ext = target.output_extension();
        crate::cli::render::run_compile_step(&output_path, cmd, ext, true)?;
        Ok(output_path.with_extension(ext))
    } else {
        Ok(output_path)
    }
}

/// Generate an HTML page that embeds a PDF with live-reload.
fn build_pdf_viewer_html(pdf_filename: &str, version: u64) -> String {
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

fn format_local_time() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

