//! Per-element filters: enrich template vars for code and figure elements.
//!
//! These run during element rendering in `ElementRenderer::render_templated()`,
//! not through the plugin registry. They handle code highlighting and figure
//! variable building for the element templates.

pub mod code;
pub mod figure;
pub mod theorem;

use std::collections::HashMap;

use crate::types::Element;

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
        defaults: &crate::config::Metadata,
    ) -> FilterResult;
}
