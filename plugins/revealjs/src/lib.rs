use extism_pdk::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct PostprocessInput {
    body: String,
    #[allow(dead_code)]
    format: String,
    title: String,
    css: String,
}

/// Split rendered HTML body into reveal.js slides.
///
/// The first slide is a title slide from metadata.
/// Subsequent slides are separated by `<h2>` headings.
#[plugin_fn]
pub fn postprocess(Json(input): Json<PostprocessInput>) -> FnResult<String> {
    let body = &input.body;

    // Split by <h2> tags (keeping the tag with each section)
    let mut slides: Vec<String> = Vec::new();
    let mut current = String::new();
    let bytes = body.as_bytes();

    let mut i = 0;
    while i < bytes.len() {
        if i + 3 < bytes.len()
            && bytes[i] == b'<'
            && bytes[i + 1] == b'h'
            && bytes[i + 2] == b'2'
            && (bytes[i + 3] == b' ' || bytes[i + 3] == b'>')
        {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                slides.push(trimmed);
            }
            current = String::new();
        }
        current.push(bytes[i] as char);
        i += 1;
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        slides.push(trimmed);
    }

    // Build sections: title slide first, then content slides
    let mut sections = Vec::new();

    // Title slide
    if !input.title.is_empty() {
        sections.push(format!("<section>\n<h1>{}</h1>\n</section>", input.title));
    }

    // Content slides
    for slide in &slides {
        sections.push(format!("<section>\n{}\n</section>", slide));
    }

    let result = format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/reveal.js@5/dist/reveal.css">
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/reveal.js@5/dist/theme/white.css">
<style>
.reveal pre {{ font-size: 0.55em; }}
.reveal code {{ font-size: 0.9em; }}
.reveal img {{ max-height: 500px; }}
{css}
</style>
<script>
MathJax = {{
  tex: {{ inlineMath: [['$','$'], ['\\(','\\)']], displayMath: [['$$','$$'], ['\\[','\\]']] }},
  options: {{ ignoreHtmlClass: 'nodollar' }},
  svg: {{ fontCache: 'global' }}
}};
</script>
<script id="MathJax-script" async src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-svg.js"></script>
</head>
<body>
<div class="reveal">
<div class="slides">
{slides}
</div>
</div>
<script src="https://cdn.jsdelivr.net/npm/reveal.js@5/dist/reveal.js"></script>
<script>
Reveal.initialize({{
  hash: true,
  slideNumber: true,
  transition: 'slide'
}});
</script>
</body>
</html>"#,
        title = input.title,
        css = input.css,
        slides = sections.join("\n\n"),
    );

    Ok(result)
}
