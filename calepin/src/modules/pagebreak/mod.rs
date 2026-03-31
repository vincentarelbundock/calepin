//! Pagebreak span module: `[]{.pagebreak}` -> format-specific page break.

pub fn render(format: &str) -> String {
    match crate::render::elements::resolve_element_template("pagebreak", format) {
        Some(t) => crate::render::template::apply_template(&t, &crate::render::template::TemplateVars::new()),
        None => "\u{0C}".to_string(),
    }
}
