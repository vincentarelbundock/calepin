//! Body transform modules.
//!
//! Each module is a named transformation applied to the rendered body string
//! between element rendering and cross-reference resolution. Targets declare
//! which transforms to apply (and in what order) via `body_transforms`.
//!
//! Transforms are organized by target format: `html/`, `latex/`, etc.
//! They are registered as plugins in the plugin registry and resolved by name.

pub mod html;
pub mod latex;

use crate::render::elements::ElementRenderer;
use crate::project::Target;

/// A named body transform module. Implementations are registered in the
/// plugin registry and selected by name in the Target's `body_transforms` list.
pub trait TransformBody: Send + Sync {
    fn transform(&self, body: &str, renderer: &ElementRenderer, target: &Target) -> String;
}
