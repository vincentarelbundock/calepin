//! Video span module: `[]{.video url="..."}` -> video embed.
//!
//! Supports YouTube and Vimeo URL auto-detection and embed URL normalization.

use std::collections::HashMap;

pub fn render(
    kv: &HashMap<String, String>,
    format: &str,
    defaults: &crate::config::Metadata,
) -> String {
    let url = match kv.get("url") {
        Some(u) => u.as_str(),
        None => {
            cwarn!("[]{{.video}} requires a url attribute");
            return String::new();
        }
    };

    let vdefs = defaults.video.as_ref();
    let default_width = vdefs.and_then(|v| v.width.clone()).unwrap_or_else(|| "100%".to_string());
    let default_height = vdefs.and_then(|v| v.height.clone()).unwrap_or_else(|| "400".to_string());
    let default_title = vdefs.and_then(|v| v.title.clone()).unwrap_or_else(|| "Video".to_string());

    let width = kv.get("width").map(|s| s.as_str()).unwrap_or(&default_width);
    let height = kv.get("height").map(|s| s.as_str()).unwrap_or(&default_height);
    let title = kv.get("title").map(|s| s.as_str()).unwrap_or(&default_title);

    // YouTube/Vimeo URL normalization
    let embed_url = if url.contains("youtube.com/watch") || url.contains("youtu.be") {
        let id = url
            .split("v=").nth(1).map(|s| s.split('&').next().unwrap_or(s))
            .or_else(|| url.split("youtu.be/").nth(1).map(|s| s.split('?').next().unwrap_or(s)))
            .unwrap_or(url);
        format!("https://www.youtube.com/embed/{}", id)
    } else if url.contains("vimeo.com/") {
        let id = url.rsplit('/').next().unwrap_or(url);
        format!("https://player.vimeo.com/video/{}", id)
    } else {
        url.to_string()
    };

    let is_embed = embed_url.contains("youtube.com/embed") || embed_url.contains("player.vimeo.com");
    let mut vars = HashMap::new();
    vars.insert("src".to_string(), url.to_string());
    vars.insert("url".to_string(), embed_url);
    vars.insert("width".to_string(), width.to_string());
    vars.insert("height".to_string(), height.to_string());
    vars.insert("title".to_string(), title.to_string());
    vars.insert("is_embed".to_string(), is_embed.to_string());

    let fallback = format!("[{}]({})", title, url);
    match crate::render::elements::resolve_builtin_partial("video", format) {
        Some(tpl) => crate::render::template::apply_template(tpl, &vars),
        None => fallback,
    }
}
