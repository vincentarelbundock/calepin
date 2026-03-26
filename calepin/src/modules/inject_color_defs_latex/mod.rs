//! Prepend LaTeX color definitions for syntax highlighting.

use crate::render::elements::ElementRenderer;
use crate::project::Target;
use crate::modules::transform_body::TransformBody;

pub struct InjectColorDefsLatex;

impl TransformBody for InjectColorDefsLatex {

    fn transform(&self, body: &str, renderer: &ElementRenderer, _target: &Target) -> String {
        let color_defs = renderer.latex_color_definitions();
        if color_defs.is_empty() {
            body.to_string()
        } else {
            format!("{}\n{}", color_defs, body)
        }
    }
}
