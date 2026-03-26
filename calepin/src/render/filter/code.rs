// Code filter: fills template variables for code elements.
//
// - BuildCodeVars::apply()       — Populate vars (code, lang, label, highlighted) for
//                                CodeSource, CodeOutput, CodeWarning/Message/Error.
// - escape_code_for_format()  — Typst-specific escaping for code strings.

use std::collections::HashMap;

use super::BuildElementVars;
use crate::types::Element;
use crate::modules::highlight::Highlighter;

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

fn escape_code_for_format(s: &str, format: &str) -> String {
    match format {
        "typst" => s.replace('\\', "\\\\").replace('"', "\\\""),
        _ => s.to_string(),
    }
}

