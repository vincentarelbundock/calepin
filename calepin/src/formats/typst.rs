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
        renderer: &ElementRenderer,
    ) -> Option<String> {
        let mut vars = crate::render::template::build_template_vars(meta, body, "typst");
        let preamble_content = crate::render::template::deduplicate_preamble(renderer.preamble());
        if !preamble_content.is_empty() {
            let entry = vars.entry("preamble".to_string()).or_default();
            if !entry.is_empty() { entry.push('\n'); }
            entry.push_str(&preamble_content);
        }
        let tpl = crate::render::template::load_page_template("page", "typst");
        Some(crate::render::template::render_page_template(&tpl, &vars, "typst"))
    }
}
