//! TransformDocument trait: post-assembly full document mutation.
//!
//! Runs after the page template has been applied. Used for operations
//! like base64 image embedding and syntax CSS/color injection.

use crate::render::elements::ElementRenderer;

pub trait TransformDocument: Send + Sync {
    fn transform(&self, document: &str, engine: &str, renderer: &ElementRenderer) -> String;
}
