//! Per-element var builders: enrich template vars for code and figure elements.
//!
//! These run during element rendering in `ElementRenderer::render_templated()`,
//! not through the module registry. They handle code highlighting and figure
//! variable building for the element templates.

pub mod code;
pub mod figure;
pub mod theorem;

use std::collections::HashMap;

use crate::types::Element;

/// Populates template variables for a specific element type.
/// Each builder handles the element types it knows about and ignores the rest.
pub trait BuildElementVars {
    fn apply(
        &self,
        element: &Element,
        format: &str,
        vars: &mut HashMap<String, String>,
        defaults: &crate::config::Metadata,
    );
}
