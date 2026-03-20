//! Shared utility functions.

use std::path::{Path, PathBuf};

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

/// Resolve a file by checking project then user directories.
/// Returns the first path that exists, or None.
///
/// Resolution order:
///   1. `_calepin/{dir}/{filename}` (project)
///   2. `~/.config/calepin/{dir}/{filename}` (user)
pub fn resolve_path(dir: &str, filename: &str) -> Option<PathBuf> {
    let project = Path::new("_calepin").join(dir).join(filename);
    if project.exists() {
        return Some(project);
    }

    if let Ok(home) = std::env::var("HOME") {
        let user = Path::new(&home)
            .join(".config/calepin")
            .join(dir)
            .join(filename);
        if user.exists() {
            return Some(user);
        }
    }

    None
}

/// Run a subprocess with JSON on stdin, return stdout on success.
/// Used by external filters and shortcodes.
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

/// Find the first file matching a glob pattern in `_calepin/{dir}/` then `~/.config/calepin/{dir}/`.
/// Returns the alphabetically first match across both directories.
pub fn resolve_first_match(dir: &str, extension: &str) -> Option<PathBuf> {
    let dirs: Vec<PathBuf> = {
        let mut v = vec![Path::new("_calepin").join(dir)];
        if let Ok(home) = std::env::var("HOME") {
            v.push(Path::new(&home).join(".config/calepin").join(dir));
        }
        v
    };
    for d in &dirs {
        if let Ok(entries) = std::fs::read_dir(d) {
            let mut matches: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(extension))
                .collect();
            matches.sort();
            if let Some(first) = matches.into_iter().next() {
                return Some(first);
            }
        }
    }
    None
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
