use base64::Engine;
use extism_pdk::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize)]
struct FilterContext {
    context: String,
    content: String,
    classes: Vec<String>,
    format: String,
    attrs: HashMap<String, String>,
}

#[derive(Serialize)]
enum FilterResult {
    Rendered(String),
    Pass,
}

/// Built-in anonymous imgur Client-IDs (same as xfun::upload_imgur).
const BUILTIN_IMGUR_KEYS: &[&str] = &[
    "019b80d1dc7a38b",
    "0db7f928e7df36b",
    "24190e94c632a10",
];

fn get_client_id() -> String {
    // Try plugin config first
    if let Some(id) = config::get("client_id").ok().flatten() {
        if !id.is_empty() {
            return id;
        }
    }
    BUILTIN_IMGUR_KEYS[0].to_string()
}

/// Upload image data to imgur, return the public URL.
fn upload_imgur(image_data: &[u8], client_id: &str) -> Result<String, Error> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(image_data);

    // URL-encode the base64 for form body
    let encoded_safe = encoded
        .replace('+', "%2B")
        .replace('/', "%2F")
        .replace('=', "%3D");
    let body = format!("image={}&type=base64", encoded_safe);

    let req = HttpRequest::new("https://api.imgur.com/3/image")
        .with_method("POST")
        .with_header("Authorization", &format!("Client-ID {}", client_id))
        .with_header("Content-Type", "application/x-www-form-urlencoded");

    let resp = http::request::<Vec<u8>>(&req, Some(body.into_bytes()))?;
    let resp_body = resp.body();
    let body_str = String::from_utf8_lossy(&resp_body);
    let json: serde_json::Value = serde_json::from_str(&body_str)?;

    json["data"]["link"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| {
            let err = json["data"]["error"]
                .as_str()
                .unwrap_or("unknown error");
            Error::msg(format!("Imgur upload failed: {}", err))
        })
}

/// Span filter: `[path/to/image.png]{.imgur alt="description"}`
///
/// Reads the local image file, uploads it to imgur, and returns
/// format-appropriate image markup with the public URL.
#[plugin_fn]
pub fn filter(Json(ctx): Json<FilterContext>) -> FnResult<Json<FilterResult>> {
    if ctx.context != "span" || !ctx.classes.iter().any(|c| c == "imgur") {
        return Ok(Json(FilterResult::Pass));
    }

    let path = ctx.content.trim();
    let alt = ctx.attrs.get("alt").map(|s| s.as_str()).unwrap_or("");

    // Read the image file (requires WASI filesystem access)
    let image_data = match std::fs::read(path) {
        Ok(data) => data,
        Err(e) => {
            return Ok(Json(FilterResult::Rendered(format!(
                "<!-- imgur: failed to read {}: {} -->",
                path, e
            ))));
        }
    };

    let client_id = get_client_id();

    match upload_imgur(&image_data, &client_id) {
        Ok(url) => {
            let output = match ctx.format.as_str() {
                "html" => format!("<img src=\"{}\" alt=\"{}\" />", url, alt),
                "tex" => {
                    // LaTeX can't use remote URLs directly; include a comment with the URL
                    format!("\\includegraphics{{{}}} % imgur: {}", path, url)
                }
                "typ" => format!("#image(\"{}\")", url),
                _ => format!("![{}]({})", alt, url),
            };
            Ok(Json(FilterResult::Rendered(output)))
        }
        Err(e) => Ok(Json(FilterResult::Rendered(format!(
            "<!-- imgur upload failed: {} -->",
            e
        )))),
    }
}
