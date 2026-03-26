//! TransformPage: inject syntax highlighting CSS into page template variables.

use std::collections::HashMap;

use crate::modules::transform_page::TransformPage;
use crate::modules::highlight::ColorScope;
use crate::render::elements::ElementRenderer;
use crate::config::Metadata;

pub struct InjectSyntaxCss;

impl TransformPage for InjectSyntaxCss {
    fn transform(&self, vars: &mut HashMap<String, String>, renderer: &ElementRenderer, _meta: &Metadata) {
        let syntax_css = renderer.syntax_css_with_scope(ColorScope::Both);
        if !syntax_css.is_empty() {
            let css = vars.entry("css".to_string()).or_default();
            css.push('\n');
            css.push_str(&syntax_css);
            // Also set as a standalone var for partials that use it separately
            vars.insert("syntax_css".to_string(), syntax_css);
        }
    }
}
