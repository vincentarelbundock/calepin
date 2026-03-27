// Per-element var builders: enrich template vars before rendering.
//
// These run during element rendering in `ElementRenderer::render_templated()`,
// not through the module registry.

use std::collections::HashMap;

use crate::types::Element;
use crate::modules::Highlighter;
use crate::utils::escape::escape_code_for_format;

/// Populates template variables for a specific element type.
/// Each builder handles the element types it knows about and ignores the rest.
pub trait BuildElementVars {
    fn apply(
        &self,
        element: &Element,
        format: &str,
        vars: &mut HashMap<String, String>,
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
    fn apply(&self, element: &Element, format: &str, vars: &mut HashMap<String, String>, _defaults: &crate::config::Metadata) {
        match element {
            Element::CodeSource { code, lang, label, filename, .. } => {
                let escaped = escape_code_for_format(code, format);
                let highlighted = self.highlighter.highlight(code, lang, format);
                if !filename.is_empty() {
                    vars.insert("filename".to_string(), filename.clone());
                }
                vars.insert("code".to_string(), escaped);
                vars.insert("lang".to_string(), lang.clone());
                vars.insert("label".to_string(), label.clone());
                vars.insert("highlighted".to_string(), highlighted);
            }
            Element::CodeOutput { text } => {
                vars.insert("output".to_string(), escape_code_for_format(text, format));
            }
            Element::CodeWarning { text }
            | Element::CodeMessage { text }
            | Element::CodeError { text } => {
                vars.insert("text".to_string(), escape_code_for_format(text, format));
                let cls = match element {
                    Element::CodeWarning { .. } => "warning",
                    Element::CodeMessage { .. } => "message",
                    Element::CodeError { .. } => "error",
                    _ => unreachable!(),
                };
                vars.insert("diagnostic_class".to_string(), cls.to_string());
            }
            _ => {}
        }
    }
}
