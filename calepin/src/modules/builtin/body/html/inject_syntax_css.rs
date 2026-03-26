//! Inject syntax highlighting CSS into the page template variables.
//!
//! This transform appends syntax highlighting CSS to the body as a `<style>`
//! block. For standalone documents this is picked up by the page template;
//! for collection pages it is inlined before the body.

use crate::render::elements::ElementRenderer;
use crate::render::highlighting::ColorScope;
use crate::project::Target;
use crate::modules::builtin::body::TransformBody;

pub struct InjectSyntaxCssHtml;

impl TransformBody for InjectSyntaxCssHtml {

    fn transform(&self, body: &str, _renderer: &ElementRenderer, _target: &Target) -> String {
        // The CSS is injected during page assembly (assemble_page) rather than
        // as a body transform, because it needs to go into the page template's
        // `css` variable, not into the body string. This module is a marker:
        // FormatPipeline checks whether "syntax_css" is in the transform list
        // during assemble_page to decide whether to inject the CSS.
        body.to_string()
    }
}

/// Generate syntax highlighting CSS for the given scope.
/// Called by FormatPipeline::assemble_page when "syntax_css" is in body_transforms.
pub fn generate(renderer: &ElementRenderer, scope: ColorScope) -> String {
    renderer.syntax_css_with_scope(scope)
}
