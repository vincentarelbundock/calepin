use extism_pdk::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct PostprocessInput {
    body: String,
    #[allow(dead_code)]
    format: String,
    title: String,
    #[allow(dead_code)]
    css: String,
}

/// Convert rendered Markdown body into Slidev format.
///
/// Slidev uses `---` to separate slides. Each `## ` heading starts a new slide.
/// A title slide is prepended with Slidev YAML frontmatter.
#[plugin_fn]
pub fn postprocess(Json(input): Json<PostprocessInput>) -> FnResult<String> {
    let mut slides: Vec<String> = Vec::new();

    // Title slide with Slidev frontmatter
    slides.push(format!(
        "---\ntheme: default\ntitle: {title}\n---\n\n# {title}",
        title = input.title,
    ));

    // Split body by ## headings
    let mut current = String::new();
    for line in input.body.lines() {
        if line.starts_with("## ") {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                slides.push(trimmed);
            }
            current = format!("{}\n", line);
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        slides.push(trimmed);
    }

    Ok(slides.join("\n\n---\n\n"))
}
