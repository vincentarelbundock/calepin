//! TransformPage: inject syntax highlighting variables into page template.
//!
//! For HTML: injects CSS into the `css` and `syntax_css` template vars.
//! For LaTeX: injects `\definecolor` commands into the `colors` template var.

use std::collections::HashMap;

use crate::modules::transform_page::TransformPage;
use crate::modules::highlight::ColorScope;
use crate::render::elements::ElementRenderer;
use crate::config::Metadata;

pub struct InjectHighlightVars;

impl TransformPage for InjectHighlightVars {
    fn transform(&self, vars: &mut HashMap<String, String>, renderer: &ElementRenderer, _meta: &Metadata) {
        let format = vars.get("base").map(|s| s.as_str()).unwrap_or("");

        match format {
            "html" => {
                let syntax_css = renderer.syntax_css_with_scope(ColorScope::Both);
                if !syntax_css.is_empty() {
                    let css = vars.entry("css".to_string()).or_default();
                    css.push('\n');
                    css.push_str(&syntax_css);
                    vars.insert("syntax_css".to_string(), syntax_css);
                }
            }
            "latex" => {
                let colors = renderer.latex_color_definitions();
                if !colors.is_empty() {
                    vars.insert("colors".to_string(), colors);
                }
            }
            _ => {}
        }
    }
}
