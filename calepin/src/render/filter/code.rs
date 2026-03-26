// Code filter: fills template variables for code elements.
//
// - CodeFilter::apply()       — Populate vars (code, lang, label, highlighted) for
//                                CodeSource, CodeOutput, CodeWarning/Message/Error.
// - escape_code_for_format()  — Typst-specific escaping for code strings.

use std::collections::HashMap;

use super::{Filter, FilterResult};
use crate::types::Element;
use crate::modules::highlight::Highlighter;

pub struct CodeFilter<'a> {
    highlighter: &'a Highlighter,
}

impl<'a> CodeFilter<'a> {
    pub fn new(highlighter: &'a Highlighter) -> Self {
        Self { highlighter }
    }
}

impl<'a> Filter for CodeFilter<'a> {
    fn apply(&self, element: &Element, format: &str, vars: &mut HashMap<String, String>, _defaults: &crate::config::Metadata) -> FilterResult {
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
                FilterResult::Continue
            }
            Element::CodeOutput { text } => {
                vars.insert("output".to_string(), escape_code_for_format(text, format));
                FilterResult::Continue
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
                FilterResult::Continue
            }
            _ => FilterResult::Pass,
        }
    }
}

fn escape_code_for_format(s: &str, format: &str) -> String {
    match format {
        "typst" => s.replace('\\', "\\\\").replace('"', "\\\""),
        _ => s.to_string(),
    }
}

