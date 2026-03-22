use crate::render::elements::ElementRenderer;
use crate::formats::OutputRenderer;
use crate::render::template;
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
        let vars = template::build_template_vars(meta, body, "typst");
        let tpl = template::load_page_template("page", "typst");
        Some(template::render_page_template(&tpl, &vars, "typst"))
    }
}
