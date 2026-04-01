// Tailwind CSS build-time compilation.
//
// When the `tailwindcss` CLI is available on PATH, this module runs it after
// all HTML files and assets are written. It generates a compiled CSS file
// (`assets/calepin-tw.css`) containing only the utility classes actually used
// in the rendered output. When the CLI is not available, the templates fall
// back to the Tailwind browser script (CDN).

use std::path::Path;

use anyhow::Result;

/// Check whether the `tailwindcss` CLI is available on PATH.
pub fn is_available() -> bool {
    crate::utils::tools::is_available(&crate::utils::tools::TAILWINDCSS)
}

/// Run the Tailwind CSS CLI to compile a CSS file from the rendered HTML output.
///
/// Writes a temporary input CSS file containing `@import "tailwindcss"` plus the
/// active color scheme's `@theme` block and color tokens, then invokes the CLI
/// to scan all HTML files in the output directory and produce `assets/calepin-tw.css`.
pub fn compile(
    output_dir: &Path,
    target_name: &str,
    cfg: &std::collections::HashMap<String, crate::value::Value>,
    quiet: bool,
) -> Result<()> {
    let colors = crate::config::extension::resolve_active_colors(cfg, target_name);
    let combined_css = colors
        .as_ref()
        .map(|c| c.generate_combined_css())
        .unwrap_or_default();

    // Build the Tailwind input CSS
    let input_css = format!(
        "@import \"tailwindcss\";\n\n{}\n",
        combined_css,
    );

    // Write temporary input file in the output directory
    let input_path = output_dir.join("assets").join("_tailwind-input.css");
    std::fs::write(&input_path, &input_css)?;

    let output_path = output_dir.join("assets").join("calepin-tw.css");

    if !quiet {
        eprintln!("  \x1b[36mtailwind:\x1b[0m compiling CSS...");
    }

    let status = std::process::Command::new("tailwindcss")
        .arg("-i")
        .arg(&input_path)
        .arg("-o")
        .arg(&output_path)
        .arg("--content")
        .arg(format!("{}/**/*.html", output_dir.display()))
        .arg("--minify")
        .stdout(if quiet { std::process::Stdio::null() } else { std::process::Stdio::inherit() })
        .stderr(if quiet { std::process::Stdio::null() } else { std::process::Stdio::inherit() })
        .status();

    // Clean up the temporary input file
    let _ = std::fs::remove_file(&input_path);

    match status {
        Ok(s) if s.success() => {
            if !quiet {
                eprintln!("  \x1b[36mtailwind:\x1b[0m done");
            }
            Ok(())
        }
        Ok(s) => {
            eprintln!("  \x1b[33mwarning:\x1b[0m tailwindcss exited with {}", s);
            Ok(())
        }
        Err(e) => {
            eprintln!("  \x1b[33mwarning:\x1b[0m failed to run tailwindcss: {}", e);
            Ok(())
        }
    }
}
