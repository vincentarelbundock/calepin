//! TransformDocument trait: post-assembly full document mutation.
//!
//! Runs after the page template has been applied. Used for operations
//! like base64 image embedding that need the complete HTML document.

pub trait TransformDocument: Send + Sync {
    fn transform(&self, document: &str) -> String;
}
