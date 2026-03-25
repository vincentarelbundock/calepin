use crate::render::elements::ElementRenderer;
use crate::formats::OutputRenderer;
use crate::metadata::Metadata;

pub struct LatexRenderer;

impl OutputRenderer for LatexRenderer {
    fn format(&self) -> &str { "latex" }
    fn extension(&self) -> &str { "tex" }

    fn transform_body(&self, body: &str, renderer: &ElementRenderer) -> String {
        let color_defs = renderer.latex_color_definitions();
        if color_defs.is_empty() {
            body.to_string()
        } else {
            format!("{}\n{}", color_defs, body)
        }
    }

    fn assemble_page(
        &self,
        body: &str,
        meta: &Metadata,
        renderer: &ElementRenderer,
    ) -> Option<String> {
        // Note: transform_body() is called by the pipeline before
        // assemble_page, so color definitions are already prepended to body.
        Some(crate::render::template::assemble_page(
            body, meta, "latex", &[], renderer.preamble(), renderer.target.as_ref(), |_| {},
        ))
    }
}
