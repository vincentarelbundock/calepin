//! TransformDocument trait: post-assembly full document mutation.
//!
//! Runs after the page template has been applied. Used for operations
//! like base64 image embedding, syntax CSS/color injection, and
//! user-provided scripts via stdin/stdout.

use std::path::PathBuf;

use crate::render::elements::ElementRenderer;

pub trait TransformDocument: Send + Sync {
    fn transform(&self, document: &str, writer: &str, renderer: &ElementRenderer) -> String;
}

/// A document transform backed by an external script.
/// Sends the document on stdin, reads the transformed document from stdout.
pub struct ScriptTransformDocument {
    pub script_path: PathBuf,
    pub module_dir: PathBuf,
}

impl TransformDocument for ScriptTransformDocument {
    fn transform(&self, document: &str, writer: &str, _renderer: &ElementRenderer) -> String {
        use std::process::{Command, Stdio};
        use std::io::Write;

        let result = Command::new("sh")
            .arg("-c")
            .arg(self.script_path.to_string_lossy().as_ref())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(&self.module_dir)
            .env("CALEPIN_FORMAT", writer)
            .env("CALEPIN_ROOT", crate::paths::get_project_root().to_string_lossy().as_ref())
            .spawn()
            .and_then(|mut child| {
                if let Some(ref mut stdin) = child.stdin {
                    stdin.write_all(document.as_bytes()).ok();
                }
                drop(child.stdin.take());
                child.wait_with_output()
            });

        match result {
            Ok(output) if output.status.success() => {
                String::from_utf8(output.stdout).unwrap_or_else(|_| document.to_string())
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                cwarn!("module script failed: {}: {}", self.script_path.display(), stderr.trim());
                document.to_string()
            }
            Err(e) => {
                cwarn!("failed to run module script {}: {}", self.script_path.display(), e);
                document.to_string()
            }
        }
    }
}
