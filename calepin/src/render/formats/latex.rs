use crate::render::elements::ElementRenderer;
use crate::formats::OutputRenderer;
use crate::types::Metadata;

pub struct LatexRenderer;

impl OutputRenderer for LatexRenderer {
    fn format(&self) -> &str { "latex" }
    fn extension(&self) -> &str { "tex" }

    fn postprocess(&self, body: &str, renderer: &ElementRenderer) -> String {
        let color_defs = renderer.latex_color_definitions();
        if color_defs.is_empty() {
            body.to_string()
        } else {
            format!("{}\n{}", color_defs, body)
        }
    }

    fn apply_template(
        &self,
        body: &str,
        meta: &Metadata,
        renderer: &ElementRenderer,
    ) -> Option<String> {
        // Note: postprocess() is already called by the render pipeline before
        // apply_template, so color definitions are already prepended to body.
        Some(crate::render::template::assemble_page(
            body, meta, "latex", &[], renderer.preamble(), |_| {},
        ))
    }
}
