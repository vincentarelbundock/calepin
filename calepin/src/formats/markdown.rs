use crate::formats::OutputRenderer;
use crate::types::Metadata;
use crate::render::elements::ElementRenderer;

pub struct MarkdownRenderer;

impl OutputRenderer for MarkdownRenderer {
    fn format(&self) -> &str { "markdown" }
    fn extension(&self) -> &str { "md" }

    fn assemble_page(
        &self,
        _body: &str,
        _meta: &Metadata,
        _renderer: &ElementRenderer,
    ) -> Option<String> {
        None
    }
}
