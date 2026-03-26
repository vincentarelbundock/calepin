//! Figure module: handles `#fig-` prefixed divs and `Element::Figure`.
//!
//! Consolidates caption extraction, variable enrichment, image variant
//! selection, and template rendering for all figure types.
//! Registered as a TransformElementChildren module matching `id_prefix = "fig-"`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::types::Element;

// ---------------------------------------------------------------------------
// Figure div rendering (TransformElementChildren entry point)
// ---------------------------------------------------------------------------

/// Render a figure div: extract caption from children, build template
/// vars, apply the `figure_div` template.
pub fn render(
    id: &Option<String>,
    attrs: &HashMap<String, String>,
    children: &[Element],
    format: &str,
    render_element: &dyn Fn(&Element) -> String,
    defaults: &crate::config::Metadata,
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
    let mut vars = HashMap::new();
    vars.insert("base".to_string(), format.to_string());
    vars.insert("writer".to_string(), format.to_string());
    vars.insert("children".to_string(), children_rendered);
    vars.insert("label".to_string(), id_val.to_string());
    vars.insert("id".to_string(), id_val.to_string());

    // Copy div attrs into vars
    for (k, val) in attrs {
        vars.insert(k.clone(), val.clone());
    }

    // Render caption markdown to target format
    if !caption_text.is_empty() {
        let rendered_caption = crate::render::convert::render_inline(&caption_text, format);
        vars.insert("caption".to_string(), rendered_caption);
    }

    // Figure wrapper vars (alignment, fig_env, fig_pos, short_caption, cap_location, link)
    let fig_attrs = figure_attrs_from_div(attrs);
    build_figure_wrapper_vars(&mut vars, &fig_attrs, format, None, defaults);

    let tpl = crate::render::elements::resolve_builtin_partial("figure_div", format).unwrap_or("");
    crate::render::template::apply_template(tpl, &vars)
}

fn render_children(children: &[Element], render_element: &dyn Fn(&Element) -> String) -> String {
    children.iter()
        .map(render_element)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

// ---------------------------------------------------------------------------
// Element::Figure var building (called from ElementRenderer::build_template_output)
// ---------------------------------------------------------------------------

pub struct BuildFigureVars {
    default_cap_location: Option<String>,
    /// Preferred image formats for variant selection, in priority order.
    fig_formats: Vec<String>,
}

impl BuildFigureVars {
    pub fn new(
        ext: &str,
        target: Option<&crate::config::Target>,
        default_cap_location: Option<String>,
    ) -> Self {
        let fig_formats = target
            .map(|t| t.fig_formats.clone())
            .filter(|f| !f.is_empty())
            .unwrap_or_else(|| default_fig_formats(ext));
        Self { default_cap_location, fig_formats }
    }
}

impl crate::render::filter::BuildElementVars for BuildFigureVars {
    fn apply(&self, element: &Element, format: &str, vars: &mut HashMap<String, String>, defaults: &crate::config::Metadata) {
        if let Element::Figure { path, alt, caption, label, number, attrs } = element {
            build_figure_element_vars(
                vars, path, alt, caption.as_deref(), label,
                number.as_deref(), attrs, format,
                self.default_cap_location.as_deref(),
                defaults,
                &self.fig_formats,
            );
        }
    }
}

fn build_figure_element_vars(
    vars: &mut HashMap<String, String>,
    path: &Path,
    alt: &str,
    caption: Option<&str>,
    label: &str,
    number: Option<&str>,
    attrs: &crate::types::FigureAttrs,
    format: &str,
    default_cap_location: Option<&str>,
    defaults: &crate::config::Metadata,
    fig_formats: &[String],
) {
    vars.insert("alt".to_string(), alt.to_string());
    vars.insert("caption".to_string(), caption.unwrap_or("").to_string());
    vars.insert("label".to_string(), label.to_string());
    vars.insert("number".to_string(), number.unwrap_or("").to_string());

    let resolved_path = select_image_variant_with_prefs(path, fig_formats);
    let display_path = resolved_path.to_string_lossy().to_string();
    vars.insert("path".to_string(), display_path.clone());

    // Image components for template
    let embed = defaults.embed_resources.unwrap_or(true);
    if format == "html" && embed {
        if let Ok((mime, data)) = crate::util::base64_encode_image(&resolved_path) {
            vars.insert("src".to_string(), format!("data:{};base64,{}", mime, data));
        } else {
            vars.insert("src".to_string(), crate::util::escape_html(&display_path));
        }
    } else if format == "html" {
        vars.insert("src".to_string(), crate::util::escape_html(&display_path));
    } else {
        let rel = relative_figure_path(&resolved_path);
        vars.insert("src".to_string(), rel);
    }

    vars.insert("width_attr".to_string(), format_width(attrs, format));
    vars.insert("height_attr".to_string(), format_height(attrs));

    build_figure_wrapper_vars(vars, attrs, format, default_cap_location, defaults);
}

// ---------------------------------------------------------------------------
// Caption extraction
// ---------------------------------------------------------------------------

/// Separate the caption from children in a figure div.
/// The caption is the last Text element.
pub fn separate_figure_caption(children: &[Element]) -> (Vec<Element>, String) {
    let mut content = children.to_vec();
    let mut caption = String::new();
    if let Some(last_idx) = content.iter().rposition(|e| matches!(e, Element::Text { .. })) {
        if let Element::Text { content: ref text } = content[last_idx] {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                caption = trimmed.to_string();
                content.remove(last_idx);
            }
        }
    }
    (content, caption)
}

// ---------------------------------------------------------------------------
// Shared figure var building
// ---------------------------------------------------------------------------

/// Build a `FigureAttrs` from div attribute key-value pairs.
pub fn figure_attrs_from_div(attrs: &HashMap<String, String>) -> crate::types::FigureAttrs {
    crate::types::FigureAttrs {
        width: attrs.get("fig_width").cloned(),
        height: attrs.get("fig_height").cloned(),
        fig_align: attrs.get("fig_align").cloned(),
        fig_scap: attrs.get("fig_scap").cloned(),
        fig_env: attrs.get("fig_env").cloned(),
        fig_pos: attrs.get("fig_pos").cloned(),
        cap_location: attrs.get("fig_cap_location").or_else(|| attrs.get("cap_location")).cloned(),
        link: attrs.get("fig_link").or_else(|| attrs.get("link")).cloned(),
    }
}

/// Populate figure-wrapper template vars shared by both `Element::Figure` and
/// figure divs: alignment, fig_env, fig_pos, short_caption, cap_location, link.
pub fn build_figure_wrapper_vars(
    vars: &mut HashMap<String, String>,
    attrs: &crate::types::FigureAttrs,
    format: &str,
    default_cap_location: Option<&str>,
    defaults: &crate::config::Metadata,
) {
    let default_align = defaults.figure.as_ref().and_then(|f| f.alignment.as_deref()).unwrap_or("center");
    let align = attrs.fig_align.as_deref().unwrap_or(default_align);
    vars.insert("align_style".to_string(), format_align(align, format));
    vars.insert("align".to_string(), align.to_string());

    if let Some(ref env) = attrs.fig_env {
        vars.insert("fig_env".to_string(), env.clone());
    }
    vars.insert("fig_pos".to_string(), match attrs.fig_pos.as_deref() {
        Some(pos) => format!("[{}]", pos),
        None => String::new(),
    });
    let short_caption = match attrs.fig_scap.as_deref() {
        Some(sc) => format!("[{}]", sc),
        None => String::new(),
    };
    vars.insert("short_caption".to_string(), short_caption);

    if let Some(loc) = attrs.cap_location.as_deref().or(default_cap_location) {
        vars.insert("cap_location".to_string(), loc.to_string());
    }

    if let Some(ref link) = attrs.link {
        vars.insert("link".to_string(), link.clone());
    }
}

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

pub fn format_width(attrs: &crate::types::FigureAttrs, format: &str) -> String {
    use crate::render::elements::resolve_element_partial;
    use crate::render::template::apply_template;

    let width = match attrs.width.as_deref() {
        Some(w) => w,
        None => return String::new(),
    };

    if let Some(tpl) = resolve_element_partial("format_width", format) {
        let mut vars = HashMap::new();
        vars.insert("width".to_string(), width.to_string());
        if width.ends_with('%') {
            let pct: f64 = width.trim_end_matches('%').parse().unwrap_or(100.0);
            vars.insert("width_pct".to_string(), "true".to_string());
            vars.insert("width_frac".to_string(), format!("{:.2}", pct / 100.0));
        }
        apply_template(&tpl, &vars)
    } else {
        width.to_string()
    }
}

pub fn format_height(attrs: &crate::types::FigureAttrs) -> String {
    attrs.height.clone().unwrap_or_default()
}

pub fn format_align(align: &str, format: &str) -> String {
    use crate::render::elements::resolve_element_partial;
    use crate::render::template::apply_template;

    if let Some(tpl) = resolve_element_partial("align_style", format) {
        let mut vars = HashMap::new();
        vars.insert("align".to_string(), align.to_string());
        apply_template(&tpl, &vars)
    } else {
        String::new()
    }
}

/// Find the preferred image format variant for the output format.
pub fn select_image_variant(path: &Path, format: &str) -> PathBuf {
    let preferred = default_fig_formats(format);
    select_image_variant_with_prefs(path, &preferred)
}

/// Engine-appropriate default image format preferences.
fn default_fig_formats(format: &str) -> Vec<String> {
    match format {
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
