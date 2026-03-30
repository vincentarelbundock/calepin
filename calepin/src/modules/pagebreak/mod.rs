//! Pagebreak span module: `[]{.pagebreak}` -> format-specific page break.

pub fn render(format: &str) -> String {
    let tpl = crate::render::elements::resolve_builtin_template("pagebreak", format);
    match tpl {
        Some(t) => crate::render::template::apply_template(t, &crate::render::template::TemplateVars::new()),
        None => "\u{0C}".to_string(),
    }
}
