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
        let body = self.postprocess(body, renderer);
        let mut vars = crate::render::template::build_template_vars(meta, &body, "latex");
        let preamble_content = crate::render::template::deduplicate_preamble(renderer.preamble());
        if !preamble_content.is_empty() {
            let entry = vars.entry("preamble".to_string()).or_default();
            if !entry.is_empty() { entry.push('\n'); }
            entry.push_str(&preamble_content);
        }
        let tpl = crate::render::template::load_page_template("page", "latex");
        Some(crate::render::template::render_page_template(&tpl, &vars, "latex"))
    }
}
