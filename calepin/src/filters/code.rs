// Code filter: fills template variables for code elements.
//
// - CodeFilter::apply()       — Populate vars (code, lang, label, highlighted) for
//                                CodeSource, CodeOutput, CodeWarning/Message/Error.
// - escape_code_for_format()  — Typst-specific escaping for code strings.

use std::collections::HashMap;

use super::{Filter, FilterResult};
use crate::types::Element;
use crate::filters::highlighting::Highlighter;

pub struct CodeFilter<'a> {
    highlighter: &'a Highlighter,
}

impl<'a> CodeFilter<'a> {
    pub fn new(highlighter: &'a Highlighter) -> Self {
        Self { highlighter }
    }
}

impl<'a> Filter for CodeFilter<'a> {
    fn apply(&self, element: &Element, format: &str, vars: &mut HashMap<String, String>) -> FilterResult {
        match element {
            Element::CodeSource { code, lang, label, filename } => {
                let escaped = escape_code_for_format(code, format);
                let highlighted = self.highlighter.highlight(code, lang, format);
                if !filename.is_empty() {
                    let inner = match format {
                        "html" => format!(
                            "<pre><code class=\"language-{} code\">{}</code></pre>",
                            lang, highlighted
                        ),
                        "latex" => format!(
                            "\\begin{{srccode}}\n\\begin{{Verbatim}}[commandchars=\\\\\\{{\\}}]\n{}\n\\end{{Verbatim}}\n\\end{{srccode}}",
                            highlighted
                        ),
                        "typst" => format!(
                            "#srcbox[#raw(\"{}\", block: true, lang: \"{}\")]",
                            escaped, lang
                        ),
                        _ => format!("``` {}\n{}\n```", lang, code),
                    };
                    return FilterResult::Rendered(render_filename_wrapper(&inner, filename, format));
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

/// Wrap a rendered code block with a filename header.
fn render_filename_wrapper(inner: &str, filename: &str, format: &str) -> String {
    match format {
        "html" => format!(
            "<div class=\"code-with-filename\">\n<div class=\"code-filename\">{}</div>\n{}\n</div>",
            filename, inner
        ),
        "latex" => format!(
            "\\begin{{codefilename}}\n\\codefilenameheader{{{}}}\n{}\n\\end{{codefilename}}",
            filename, inner
        ),
        "typst" => format!(
            "#block(stroke: 0.5pt + luma(180), radius: 3pt, clip: true)[\n#block(width: 100%, fill: luma(240), inset: (x: 8pt, y: 4pt))[#text(size: 0.85em)[{}]]\n{}\n]",
            filename, inner
        ),
        _ => format!("**{}**\n\n{}", filename, inner),
    }
}
