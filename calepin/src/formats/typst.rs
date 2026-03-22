use crate::render::elements::ElementRenderer;
use crate::formats::OutputRenderer;
use crate::types::Metadata;

pub struct TypstRenderer;

impl OutputRenderer for TypstRenderer {
    fn format(&self) -> &str { "typst" }
    fn extension(&self) -> &str { "typ" }

    fn apply_template(
        &self,
        body: &str,
        meta: &Metadata,
        _renderer: &ElementRenderer,
    ) -> Option<String> {
        super::apply_page_template(body, meta, "typst")
    }
}
