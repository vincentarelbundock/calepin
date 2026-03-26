//! TransformPage trait: page template variable injection.
//!
//! Runs during page assembly, before the page template is rendered.
//! Used for injecting CSS, math includes, or other template variables.

use std::collections::HashMap;
use crate::render::elements::ElementRenderer;
use crate::metadata::Metadata;

pub trait TransformPage: Send + Sync {
    fn transform(&self, vars: &mut HashMap<String, String>, renderer: &ElementRenderer, meta: &Metadata);
}
