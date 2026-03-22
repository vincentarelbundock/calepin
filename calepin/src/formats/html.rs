use regex::Regex;
use std::sync::LazyLock;

use crate::render::elements::ElementRenderer;
use crate::render::template;
use crate::formats::OutputRenderer;
use crate::types::Metadata;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;

pub struct HtmlRenderer;

impl OutputRenderer for HtmlRenderer {
    fn format(&self) -> &str { "html" }
    fn extension(&self) -> &str { "html" }

    fn postprocess(&self, body: &str, _renderer: &ElementRenderer) -> String {
        // Heading IDs, section numbering, and footnote consolidation are
        // handled structurally in the AST walker (render/html_ast.rs).
        body.to_string()
    }

    fn apply_template(
        &self,
        body: &str,
        meta: &Metadata,
        renderer: &ElementRenderer,
    ) -> Option<String> {
        // Append combined footnote section at end of body
        let footnotes = renderer.render_footnote_section();
        let full_body = if footnotes.is_empty() {
            body.to_string()
        } else {
            format!("{}{}", body, footnotes)
        };
        let walk_meta = renderer.walk_metadata();
        let mut vars = template::build_template_vars_with_headings(meta, &full_body, "html", &walk_meta.headings);
        let syntax_css = renderer.syntax_css_with_scope(
            crate::filters::highlighting::ColorScope::Both,
        );
        let css = vars.entry("css".to_string()).or_default();
        css.push_str(&format!("\n<style>\n{}</style>", syntax_css));
        let tpl = template::load_page_template("page", "html");
        let html = template::render_page_template(&tpl, &vars, "html");
        Some(embed_images_base64(&html))
    }
}

// ---------------------------------------------------------------------------
// Image embedding
// ---------------------------------------------------------------------------

static IMG_SRC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<img\s([^>]*?)src="([^"]+)"([^>]*)>"#).unwrap());

/// Replace all `<img src="path">` with base64 data URIs.
/// Skips images that are already data URIs or remote URLs.
fn embed_images_base64(html: &str) -> String {
    IMG_SRC_RE.replace_all(html, |caps: &regex::Captures| {
        let before = &caps[1];
        let src = &caps[2];
        let after = &caps[3];

        // Skip data URIs and remote URLs
        if src.starts_with("data:") || src.starts_with("http://") || src.starts_with("https://") {
            return caps[0].to_string();
        }

        let path = std::path::Path::new(src);
        match std::fs::read(path) {
            Ok(data) => {
                let mime = match path.extension().and_then(|e| e.to_str()) {
                    Some("png") => "image/png",
                    Some("jpg") | Some("jpeg") => "image/jpeg",
                    Some("svg") => "image/svg+xml",
                    Some("gif") => "image/gif",
                    Some("webp") => "image/webp",
                    _ => "application/octet-stream",
                };
                let encoded = BASE64.encode(&data);
                format!("<img {}src=\"data:{};base64,{}\"{}/>", before, mime, encoded, after)
            }
            Err(_) => caps[0].to_string(),
        }
    }).to_string()
}
