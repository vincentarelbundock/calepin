//! Figure module: handles `#fig-` prefixed divs and `Element::Figure`.
//!
//! Consolidates caption extraction, variable enrichment, image variant
//! selection, and template rendering for all figure types.
//! Registered as a TransformElementChildren module matching `id_prefix = "fig-"`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::types::Element;
use crate::render::template::TemplateVars;

// ---------------------------------------------------------------------------
// Figure div rendering (TransformElementChildren entry point)
// ---------------------------------------------------------------------------

/// Render a figure div: extract caption from children, build template
/// vars, apply the `figure` template.
pub fn render(
    id: &Option<String>,
    attrs: &HashMap<String, String>,
    children: &[Element],
    writer: &str,
    render_element: &dyn Fn(&Element) -> String,
    _defaults: &crate::config::Metadata,
    module_ids: &std::cell::RefCell<HashMap<String, String>>,
) -> String {
    let id_val = match id.as_deref() {
        Some(id) => id,
        None => return render_children(children, render_element),
    };

    // Register ID for cross-referencing
    {
        let ids = module_ids.borrow();
        let count = ids.keys().filter(|k| k.starts_with("fig-")).count();
        drop(ids);
        module_ids.borrow_mut().insert(id_val.to_string(), (count + 1).to_string());
    }

    // Extract caption from last Text child (unless fig_cap is already set)
    let (content, caption_text) = if attrs.contains_key("fig_cap") {
        (children.to_vec(), attrs.get("fig_cap").cloned().unwrap_or_default())
    } else {
        separate_figure_caption(children)
    };

    // Render children
    let children_rendered: String = content.iter()
        .map(render_element)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    // Build template vars
    let mut vars = crate::render::template::TemplateVars::with_writer(writer);
    vars.clp.insert("content".to_string(), minijinja::Value::from(children_rendered));
    vars.cfg.insert("id".to_string(), minijinja::Value::from(id_val.to_string()));

    // Copy div attrs into vars (user-authored -> config)
    for (k, val) in attrs {
        vars.cfg.insert(k.clone(), minijinja::Value::from(val.clone()));
    }

    // Render caption through the full element pipeline (processes spans, markdown)
    if !caption_text.is_empty() {
        let caption_el = Element::Text { content: caption_text };
        let rendered_caption = render_element(&caption_el);
        // Strip wrapping <p>...</p> tags to get inline content
        let rendered_caption = rendered_caption.trim();
        let rendered_caption = rendered_caption
            .strip_prefix("<p>").unwrap_or(rendered_caption);
        let rendered_caption = rendered_caption
            .strip_suffix("</p>").unwrap_or(rendered_caption);
        vars.cfg.insert("caption".to_string(), minijinja::Value::from(rendered_caption.to_string()));
    }

    let tpl = crate::render::elements::resolve_element_template("figure", writer).unwrap_or_default();
    crate::render::template::apply_template(&tpl, &vars)
}

fn render_children(children: &[Element], render_element: &dyn Fn(&Element) -> String) -> String {
    super::render_children(children, render_element)
}

// ---------------------------------------------------------------------------
// Element::Figure var building (called from ElementRenderer::build_template_output)
// ---------------------------------------------------------------------------

pub struct BuildFigureVars {
    /// Preferred image formats for variant selection, in priority order.
    fig_formats: Vec<String>,
}

impl BuildFigureVars {
    pub fn new(
        writer: &str,
        target: Option<&crate::config::Target>,
    ) -> Self {
        let fig_formats = target
            .map(|t| t.fig_formats.clone())
            .filter(|f| !f.is_empty())
            .unwrap_or_else(|| default_fig_formats(writer));
        Self { fig_formats }
    }
}

impl crate::render::vars::BuildElementVars for BuildFigureVars {
    fn apply(&self, element: &Element, writer: &str, vars: &mut crate::render::template::TemplateVars, defaults: &crate::config::Metadata) {
        if let Element::Figure { path, alt, caption, label, number, attrs, options } = element {
            build_figure_element_vars(
                vars, path, alt, caption.as_deref(), label,
                number.as_deref(), attrs, options, writer,
                defaults,
                &self.fig_formats,
            );
        }
    }
}

fn build_figure_element_vars(
    vars: &mut crate::render::template::TemplateVars,
    path: &Path,
    alt: &str,
    caption: Option<&str>,
    label: &str,
    number: Option<&str>,
    attrs: &crate::types::FigureAttrs,
    options: &HashMap<String, String>,
    writer: &str,
    defaults: &crate::config::Metadata,
    fig_formats: &[String],
) {
    // User-authored -> config
    vars.cfg.insert("caption".to_string(), minijinja::Value::from(caption.unwrap_or("").to_string()));
    vars.cfg.insert("id".to_string(), minijinja::Value::from(label.to_string()));
    // Engine-computed -> calepin
    vars.clp.insert("number".to_string(), minijinja::Value::from(number.unwrap_or("").to_string()));

    // Resolve image path
    let resolved_path = select_image_variant_with_prefs(path, fig_formats);
    let display_path = resolved_path.to_string_lossy().to_string();

    let src = if writer == "html" {
        let embed = defaults.standalone.unwrap_or(true);
        if embed {
            if let Ok((mime, data)) = crate::util::base64_encode_image(&resolved_path) {
                format!("data:{};base64,{}", mime, data)
            } else {
                crate::util::escape_html(&display_path)
            }
        } else {
            crate::util::escape_html(&display_path)
        }
    } else {
        relative_figure_path(&resolved_path)
    };

    let width_attr = format_width(attrs, writer);
    let height_attr = format_height(attrs);
    let link = attrs.link.as_deref().unwrap_or("");

    // Render image tag into clp.content so the unified figure template handles it
    let children = render_image_tag(&src, alt, &width_attr, &height_attr, link, writer);
    vars.clp.insert("content".to_string(), minijinja::Value::from(children));

    // Pass raw chunk options through to templates as cfg.*
    for (k, v) in options {
        vars.cfg.insert(k.clone(), minijinja::Value::from(v.clone()));
    }
}

/// Render a format-specific image tag string.
fn render_image_tag(src: &str, alt: &str, width: &str, height: &str, link: &str, writer: &str) -> String {
    let img = match writer {
        "html" => {
            let mut s = format!("<img src=\"{}\" alt=\"{}\"", src, alt);
            if !width.is_empty() {
                s.push_str(&format!(" style=\"width:{};max-width:{}\"", width, width));
            } else if !height.is_empty() {
                s.push_str(&format!(" style=\"height:{}\"", height));
            }
            s.push_str("/>");
            s
        }
        "latex" => {
            let w = if width.is_empty() { "width=0.70\\textwidth".to_string() } else { width.to_string() };
            format!("\\includegraphics[{}]{{{}}}", w, src)
        }
        "typst" => {
            let mut s = format!("image(\"{}\", width: {}", src, if width.is_empty() { "70%" } else { width });
            if !height.is_empty() {
                s.push_str(&format!(", height: {}", height));
            }
            s.push(')');
            s
        }
        _ => {
            // markdown
            format!("![{}]({})", alt, src)
        }
    };

    // Wrap in link if present
    if link.is_empty() {
        return img;
    }
    match writer {
        "html" => format!("<a href=\"{}\">{}</a>", link, img),
        "latex" => format!("\\href{{{}}}{{{}}}", link, img),
        "typst" => format!("#link(\"{}\")[{}]", link, img),
        _ => format!("[{}]({})", img, link),
    }
}

// ---------------------------------------------------------------------------
// Caption extraction
// ---------------------------------------------------------------------------

/// Separate the caption from children in a figure div.
/// The caption is the last paragraph of the last Text element.
pub fn separate_figure_caption(children: &[Element]) -> (Vec<Element>, String) {
    separate_caption(children, |text| {
        // Split on double newline to find the last paragraph
        if let Some(pos) = text.rfind("\n\n") {
            let remaining = text[..pos].trim().to_string();
            let caption = text[pos+2..].trim().to_string();
            if remaining.is_empty() {
                (caption, None)
            } else {
                (caption, Some(remaining))
            }
        } else {
            (text.to_string(), None)
        }
    })
}

/// Extract a caption from the last Text child element.
/// `split_fn` receives the text content and returns (caption, optional remaining text).
/// If remaining text is None, the element is removed entirely.
pub fn separate_caption(
    children: &[Element],
    split_fn: impl Fn(&str) -> (String, Option<String>),
) -> (Vec<Element>, String) {
    let mut content = children.to_vec();
    let mut caption = String::new();
    if let Some(last_idx) = content.iter().rposition(|e| matches!(e, Element::Text { .. })) {
        if let Element::Text { content: ref text } = content[last_idx] {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let (cap, remaining) = split_fn(trimmed);
                if !cap.is_empty() {
                    caption = cap;
                    match remaining {
                        Some(rest) => content[last_idx] = Element::Text { content: rest },
                        None => { content.remove(last_idx); }
                    }
                }
            }
        }
    }
    (content, caption)
}

// ---------------------------------------------------------------------------
// Shared figure var building
// ---------------------------------------------------------------------------

/// Build a `FigureAttrs` from div attribute key-value pairs.
// ---------------------------------------------------------------------------
// Image helpers
// ---------------------------------------------------------------------------

/// Make a figure path relative to the output file's directory.
fn relative_figure_path(path: &Path) -> String {
    let s = path.display().to_string();
    if let Some(idx) = s.find("_calepin/files") {
        s[idx..].to_string()
    } else {
        s
    }
}

pub fn format_width(attrs: &crate::types::FigureAttrs, writer: &str) -> String {
    use crate::render::elements::resolve_element_template;
    use crate::render::template::apply_template;

    let width = match attrs.width.as_deref() {
        Some(w) => w,
        None => return String::new(),
    };

    if let Some(tpl) = resolve_element_template("format_width", writer) {
        let mut vars = TemplateVars::new();
        vars.cfg.insert("width".to_string(), minijinja::Value::from(width.to_string()));
        if width.ends_with('%') {
            let pct: f64 = width.trim_end_matches('%').parse().unwrap_or(100.0);
            vars.cfg.insert("width_pct".to_string(), minijinja::Value::from("true".to_string()));
            vars.cfg.insert("width_frac".to_string(), minijinja::Value::from(format!("{:.2}", pct / 100.0)));
        }
        apply_template(&tpl, &vars)
    } else {
        width.to_string()
    }
}

pub fn format_height(attrs: &crate::types::FigureAttrs) -> String {
    attrs.height.clone().unwrap_or_default()
}


/// Find the preferred image format variant for the output format.
pub fn select_image_variant(path: &Path, writer: &str) -> PathBuf {
    let preferred = default_fig_formats(writer);
    select_image_variant_with_prefs(path, &preferred)
}

/// Engine-appropriate default image format preferences.
fn default_fig_formats(writer: &str) -> Vec<String> {
    match writer {
        "latex" => vec!["pdf", "eps", "svg", "png", "jpg"],
        "typst" => vec!["svg", "png", "jpg"],
        "html" => vec!["svg", "png", "jpg", "webp", "gif"],
        _ => vec!["svg", "png", "jpg"],
    }.into_iter().map(String::from).collect()
}

/// Find the preferred image format variant using an explicit preference list.
pub fn select_image_variant_with_prefs(path: &Path, preferred: &[String]) -> PathBuf {
    let preferred: Vec<&str> = preferred.iter().map(|s| s.as_str()).collect();

    if let Some(stem) = path.file_stem() {
        if let Some(parent) = path.parent() {
            for ext in preferred {
                let candidate = parent.join(format!("{}.{}", stem.to_string_lossy(), ext));
                if candidate.exists() {
                    return candidate;
                }
            }
        }
    }

    path.to_path_buf()
}
