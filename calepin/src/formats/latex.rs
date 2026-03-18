use crate::render::elements::ElementRenderer;
use crate::render::template::{self, build_latex_vars};
use crate::formats::OutputRenderer;
use crate::types::Metadata;

pub struct LatexRenderer;

impl OutputRenderer for LatexRenderer {
    fn format(&self) -> &str { "latex" }
    fn extension(&self) -> &str { "tex" }

    fn default_fig_ext(&self) -> &str {
        "pdf"
    }

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
        let mut vars = build_latex_vars(meta, body);
        let preamble = renderer.get_template("preamble");
        if !preamble.is_empty() {
            let header = vars.entry("header-includes".to_string()).or_default();
            if !header.is_empty() {
                header.push('\n');
            }
            header.push_str(&preamble);
        }
        let tpl = template::latex_template();
        Some(template::apply_template(&tpl, &vars))
    }
}
