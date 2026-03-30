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
        match super::external::run_script(&self.script_path, &self.module_dir, document, writer) {
            Ok(output) => output,
            Err(e) => {
                cwarn!("module script failed: {}: {}", self.script_path.display(), e);
                document.to_string()
            }
        }
    }
}
