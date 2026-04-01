// Per-element var builders: enrich template vars before rendering.
//
// These run during element rendering in `ElementRenderer::render_templated()`,
// not through the module registry.

use crate::types::Element;
use crate::modules::Highlighter;
use crate::utils::util::escape_code_for_format;
use crate::render::template::TemplateVars;

/// Populates template variables for a specific element type.
/// Each builder handles the element types it knows about and ignores the rest.
pub trait BuildElementVars {
    fn apply(
        &self,
        element: &Element,
        writer: &str,
        vars: &mut TemplateVars,
        defaults: &crate::config::Metadata,
    );
}

pub struct BuildCodeVars<'a> {
    highlighter: &'a Highlighter,
}

impl<'a> BuildCodeVars<'a> {
    pub fn new(highlighter: &'a Highlighter) -> Self {
        Self { highlighter }
    }
}

impl<'a> BuildElementVars for BuildCodeVars<'a> {
    fn apply(&self, element: &Element, writer: &str, vars: &mut TemplateVars, _defaults: &crate::config::Metadata) {
        match element {
            Element::CodeSource { code, lang, label, filename, .. } => {
                let escaped = escape_code_for_format(code, writer);
                let highlighted = self.highlighter.highlight(code, lang, writer);
                if !filename.is_empty() {
                    vars.cfg.insert("filename".to_string(), minijinja::Value::from(filename.clone()));
                }
                vars.clp.insert("content".to_string(), minijinja::Value::from(
                    if writer == "html" || writer == "latex" { highlighted } else { escaped }
                ));
                vars.cfg.insert("lang".to_string(), minijinja::Value::from(lang.clone()));
                vars.cfg.insert("label".to_string(), minijinja::Value::from(label.clone()));
            }
            Element::CodeOutput { text } => {
                vars.clp.insert("content".to_string(), minijinja::Value::from(escape_code_for_format(text, writer)));
            }
            Element::CodeWarning { text }
            | Element::CodeMessage { text }
            | Element::CodeError { text } => {
                vars.clp.insert("content".to_string(), minijinja::Value::from(escape_code_for_format(text, writer)));
                let cls = match element {
                    Element::CodeWarning { .. } => "warning",
                    Element::CodeMessage { .. } => "message",
                    Element::CodeError { .. } => "error",
                    _ => unreachable!(),
                };
                vars.cfg.insert("type".to_string(), minijinja::Value::from(cls.to_string()));
            }
            _ => {}
        }
    }
}
