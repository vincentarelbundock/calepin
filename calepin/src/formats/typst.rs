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
        renderer: &ElementRenderer,
    ) -> Option<String> {
        let mut vars = template::build_typst_vars(meta, body);
        vars.insert("preamble".to_string(), renderer.get_template("preamble"));
        let tpl = template::typst_template();
        Some(template::apply_template(&tpl, &vars))
    }
}
