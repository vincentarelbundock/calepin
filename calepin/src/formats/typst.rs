use crate::render::elements::ElementRenderer;
use crate::formats::OutputRenderer;
use crate::types::Metadata;

pub struct TypstRenderer;

impl OutputRenderer for TypstRenderer {
    fn format(&self) -> &str { "typst" }
    fn extension(&self) -> &str { "typ" }

    fn assemble_page(
        &self,
        body: &str,
        meta: &Metadata,
        renderer: &ElementRenderer,
    ) -> Option<String> {
        Some(crate::render::template::assemble_page(
            body, meta, "typst", &[], renderer.preamble(), |_| {},
        ))
    }
}
