//! Shared utility functions (non-path).

use std::path::Path;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("It's a Test!"), "its-a-test");
    }
}
