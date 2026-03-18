use extism_pdk::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct PostprocessInput {
    body: String,
    format: String,
    #[allow(dead_code)]
    title: String,
    #[allow(dead_code)]
    css: String,
}

const LIGHTBOX_CSS: &str = r#"<style>
.lightbox { cursor: zoom-in; }
.lightbox-overlay {
  position: fixed; top: 0; left: 0; width: 100%; height: 100%;
  background: rgba(0,0,0,0.85); display: flex; align-items: center; justify-content: center;
  z-index: 9999; cursor: zoom-out;
}
.lightbox-overlay img { max-width: 95%; max-height: 95%; object-fit: contain; }
</style>"#;

const LIGHTBOX_JS: &str = r#"<script>
document.querySelectorAll('.lightbox').forEach(function(a) {
  a.addEventListener('click', function(e) {
    e.preventDefault();
    var overlay = document.createElement('div');
    overlay.className = 'lightbox-overlay';
    var img = document.createElement('img');
    img.src = a.href || a.querySelector('img').src;
    overlay.appendChild(img);
    overlay.addEventListener('click', function() { overlay.remove(); });
    document.body.appendChild(overlay);
  });
});
</script>"#;

/// Postprocessor: wrap figure images in lightbox links and inject CSS/JS.
///
/// Finds `<img>` tags inside `.figure` divs and wraps them in
/// `<a class="lightbox" href="src">...</a>`.
#[plugin_fn]
pub fn postprocess(Json(input): Json<PostprocessInput>) -> FnResult<String> {
    if input.format != "html" {
        return Ok(input.body);
    }

    let body = &input.body;
    let mut result = String::with_capacity(body.len() + 1024);
    let mut has_lightbox = false;

    let bytes = body.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Look for <img inside a .figure div
        if i + 4 < len && &bytes[i..i + 4] == b"<img" {
            // Check if we're inside a .figure div by looking back for it
            let context = if i > 500 { &body[i - 500..i] } else { &body[..i] };
            let in_figure = context.rfind("<div class=\"figure\"").is_some()
                && !context.rfind("</div>").map_or(false, |close_pos| {
                    close_pos > context.rfind("<div class=\"figure\"").unwrap_or(0)
                });

            if in_figure {
                // Find the end of the <img> tag
                if let Some(end_offset) = body[i..].find("/>") {
                    let img_tag = &body[i..i + end_offset + 2];
                    // Extract src attribute
                    if let Some(src) = extract_attr(img_tag, "src") {
                        has_lightbox = true;
                        result.push_str(&format!(
                            "<a href=\"{}\" class=\"lightbox\">{}</a>",
                            src, img_tag
                        ));
                        i += end_offset + 2;
                        continue;
                    }
                }
                // Also handle <img ...> (not self-closing)
                if let Some(end_offset) = body[i..].find('>') {
                    let img_tag = &body[i..i + end_offset + 1];
                    if let Some(src) = extract_attr(img_tag, "src") {
                        has_lightbox = true;
                        result.push_str(&format!(
                            "<a href=\"{}\" class=\"lightbox\">{}</a>",
                            src, img_tag
                        ));
                        i += end_offset + 1;
                        continue;
                    }
                }
            }
        }

        result.push(bytes[i] as char);
        i += 1;
    }

    // Only inject CSS/JS if we actually wrapped any images
    if has_lightbox {
        result.push_str(LIGHTBOX_CSS);
        result.push_str(LIGHTBOX_JS);
    }

    Ok(result)
}

/// Extract an attribute value from an HTML tag string.
fn extract_attr<'a>(tag: &'a str, attr: &str) -> Option<&'a str> {
    let pattern = format!("{}=\"", attr);
    let start = tag.find(&pattern)? + pattern.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}
