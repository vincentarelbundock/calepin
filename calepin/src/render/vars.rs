// Per-element var builders: enrich template vars before rendering.
//
// These run during element rendering in `ElementRenderer::render_templated()`,
// not through the module registry.

use crate::types::Element;
use crate::modules::Highlighter;
use crate::utils::escape::escape_code_for_format;
use crate::render::template::TemplateVars;

/// Populates template variables for a specific element type.
/// Each builder handles the element types it knows about and ignores the rest.
pub trait BuildElementVars {
    fn apply(
        &self,
        element: &Element,
        format: &str,
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
    fn apply(&self, element: &Element, format: &str, vars: &mut TemplateVars, _defaults: &crate::config::Metadata) {
        match element {
            Element::CodeSource { code, lang, label, filename, .. } => {
                let escaped = escape_code_for_format(code, format);
                let highlighted = self.highlighter.highlight(code, lang, format);
                if !filename.is_empty() {
                    vars.cfg.insert("filename".to_string(), minijinja::Value::from(filename.clone()));
                }
                vars.clp.insert("code".to_string(), minijinja::Value::from(escaped));
                vars.cfg.insert("lang".to_string(), minijinja::Value::from(lang.clone()));
                vars.cfg.insert("label".to_string(), minijinja::Value::from(label.clone()));
                vars.clp.insert("highlighted".to_string(), minijinja::Value::from(highlighted));
            }
            Element::CodeOutput { text } => {
                vars.clp.insert("output".to_string(), minijinja::Value::from(escape_code_for_format(text, format)));
            }
            Element::CodeWarning { text }
            | Element::CodeMessage { text }
            | Element::CodeError { text } => {
                vars.clp.insert("text".to_string(), minijinja::Value::from(escape_code_for_format(text, format)));
                let cls = match element {
                    Element::CodeWarning { .. } => "warning",
                    Element::CodeMessage { .. } => "message",
                    Element::CodeError { .. } => "error",
                    _ => unreachable!(),
                };
                vars.clp.insert("diagnostic_class".to_string(), minijinja::Value::from(cls.to_string()));
            }
            _ => {}
        }
    }
}
