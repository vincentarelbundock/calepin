//! Per-element transforms: enrich template vars or produce final output.
//!
//! These run during the render stage, dispatched per-element by the div
//! pipeline (render/div.rs). Each transform handles a specific element type
//! (callouts, theorems, code blocks, figures).

pub mod code;
pub mod figure;
pub mod theorem;

use std::collections::HashMap;

use crate::types::Element;

pub use theorem::TheoremFilter;

/// Result of applying an element transform.
pub enum FilterResult {
    /// Transform produced final rendered output.
    #[allow(dead_code)]
    Rendered(String),
    /// Transform enriched the vars map. Proceed with template.
    Continue,
    /// Transform does not handle this element.
    Pass,
}

/// Uniform trait for per-element transforms.
pub trait Filter {
    fn apply(
        &self,
        element: &Element,
        format: &str,
        vars: &mut HashMap<String, String>,
        defaults: &crate::project::Defaults,
    ) -> FilterResult;
}
