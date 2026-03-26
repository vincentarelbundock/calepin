//! TransformBody trait: transforms the full rendered body string.
//!
//! Implementations are registered in the module registry and selected
//! by name in the Target's `body_transforms` list.

use crate::render::elements::ElementRenderer;
use crate::project::Target;

pub trait TransformBody: Send + Sync {
    fn transform(&self, body: &str, renderer: &ElementRenderer, target: &Target) -> String;
}
