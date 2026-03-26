//! TransformDocument: inject syntax highlighting into the assembled document.
//!
//! For HTML: injects a <style> block with syntax CSS before </head>.
//! For LaTeX: injects \definecolor commands before \begin{document}.

use crate::modules::transform_document::TransformDocument;
use crate::modules::highlight::ColorScope;
use crate::render::elements::ElementRenderer;

pub struct InjectHighlightMarkup;

impl TransformDocument for InjectHighlightMarkup {
    fn transform(&self, document: &str, engine: &str, renderer: &ElementRenderer) -> String {
        match engine {
            "html" => {
                let css = renderer.syntax_css_with_scope(ColorScope::Both);
                if css.is_empty() {
                    return document.to_string();
                }
                let style_tag = format!("<style>\n{}</style>", css);
                if let Some(pos) = document.find("</head>") {
                    format!("{}{}\n{}", &document[..pos], style_tag, &document[pos..])
                } else {
                    format!("{}\n{}", style_tag, document)
                }
            }
            "latex" => {
                let colors = renderer.latex_color_definitions();
                if colors.is_empty() {
                    return document.to_string();
                }
                if let Some(pos) = document.find("\\begin{document}") {
                    format!("{}{}\n{}", &document[..pos], colors, &document[pos..])
                } else {
                    format!("{}\n{}", colors, document)
                }
            }
            _ => document.to_string(),
        }
    }
}
