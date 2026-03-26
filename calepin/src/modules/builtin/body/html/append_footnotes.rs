//! Append the rendered footnote section to the body.

use crate::render::elements::ElementRenderer;
use crate::project::Target;
use crate::modules::builtin::body::TransformBody;

pub struct AppendFootnotesHtml;

impl TransformBody for AppendFootnotesHtml {

    fn transform(&self, body: &str, renderer: &ElementRenderer, _target: &Target) -> String {
        let footnotes = renderer.render_footnote_section();
        if footnotes.is_empty() {
            body.to_string()
        } else {
            format!("{}{}", body, footnotes)
        }
    }
}
