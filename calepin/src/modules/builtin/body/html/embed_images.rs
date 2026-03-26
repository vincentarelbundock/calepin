//! Replace `<img src="path">` with base64 data URIs for standalone HTML.

use regex::Regex;
use std::sync::LazyLock;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;

use crate::render::elements::ElementRenderer;
use crate::project::Target;
use crate::modules::builtin::body::TransformBody;

pub struct EmbedImagesHtml;

impl TransformBody for EmbedImagesHtml {

    fn transform(&self, body: &str, _renderer: &ElementRenderer, _target: &Target) -> String {
        embed_images_base64(body)
    }
}

static IMG_SRC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<img\s([^>]*?)src="([^"]+)"([^>]*)>"#).unwrap());

/// Replace all `<img src="path">` with base64 data URIs.
/// Skips images that are already data URIs or remote URLs.
pub fn embed_images_base64(html: &str) -> String {
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
