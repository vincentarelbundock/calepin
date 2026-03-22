use crate::render::elements::ElementRenderer;
use crate::render::template::{self, build_latex_vars};
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
        _renderer: &ElementRenderer,
    ) -> Option<String> {
        let vars = build_latex_vars(meta, body);
        let tpl = template::latex_template();
        Some(template::render_page_template(&tpl, &vars, "latex"))
    }
}
