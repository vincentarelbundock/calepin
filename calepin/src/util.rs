//! Shared utility functions (non-path).

use std::path::Path;

use anyhow::Context;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;

/// HTML-escape the minimal set of characters for safe embedding.
pub fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Convert heading text to a URL-friendly slug.
pub fn slugify(text: &str) -> String {
    let mut slug = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if ch == ' ' || ch == '-' || ch == '_' {
            if !slug.ends_with('-') {
                slug.push('-');
            }
        }
    }
    slug.trim_matches('-').to_string()
}

/// Run a subprocess with JSON on stdin, return stdout on success.
/// Used by external filters and plugin functions.
pub fn run_json_process(path: &Path, input: &serde_json::Value) -> Option<String> {
    use std::process::{Command, Stdio};
    match Command::new(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(mut child) => {
            if let Some(mut stdin) = child.stdin.take() {
                if let Err(e) = serde_json::to_writer(&mut stdin, input) {
                    eprintln!("Warning: subprocess {:?}: failed to write stdin: {}", path, e);
                }
                drop(stdin);
            }
            match child.wait_with_output() {
                Ok(output) if output.status.success() => {
                    Some(String::from_utf8_lossy(&output.stdout).to_string())
                }
                Ok(output) => {
                    eprintln!("Warning: subprocess {:?} exited with status {}", path, output.status);
                    None
                }
                Err(e) => {
                    eprintln!("Warning: subprocess {:?} failed: {}", path, e);
                    None
                }
            }
        }
        Err(e) => {
            eprintln!("Warning: failed to run subprocess {:?}: {}", path, e);
            None
        }
    }
}

/// Read an image file and return `(mime_type, base64_data)`.
pub fn base64_encode_image(path: &Path) -> anyhow::Result<(String, String)> {
    let data = std::fs::read(path)
        .with_context(|| format!("Failed to read image file: {}", path.display()))?;
    let encoded = BASE64.encode(&data);
    let mime = match path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    };
    Ok((mime.to_string(), encoded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("It's a Test!"), "its-a-test");
    }
}
