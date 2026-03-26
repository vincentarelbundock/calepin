//! Append the rendered footnote section to the document.

use crate::render::elements::ElementRenderer;
use crate::modules::transform_document::TransformDocument;

pub struct AppendFootnotes;

impl TransformDocument for AppendFootnotes {
    fn transform(&self, document: &str, engine: &str, renderer: &ElementRenderer) -> String {
        if engine != "html" {
            return document.to_string();
        }
        let footnotes = renderer.render_footnote_section();
        if footnotes.is_empty() {
            document.to_string()
        } else {
            // Insert before </main> or </body> or append at end
            if let Some(pos) = document.find("</main>") {
                format!("{}{}\n{}", &document[..pos], footnotes, &document[pos..])
            } else if let Some(pos) = document.find("</body>") {
                format!("{}{}\n{}", &document[..pos], footnotes, &document[pos..])
            } else {
                format!("{}{}", document, footnotes)
            }
        }
    }
}
