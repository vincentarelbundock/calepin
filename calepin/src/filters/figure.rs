// Figure filter: fills template variables for Figure elements.
//
// - FigureFilter::apply()  — Dispatch to build_figure_vars for Figure elements.
// - build_figure_vars()    — Populate all figure template vars (image tag, dimensions,
//                            alignment, caption location, link wrapping).
// - build_figure_vars()    — Populate figure template vars (path, dimensions,
//                            alignment, caption, link) for template construction.
// - select_image_variant() — Find preferred image format variant for the output format.
// - format_width/height/align() — Dimension and alignment formatting per format.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::{Filter, FilterResult};
use crate::types::Element;

pub struct FigureFilter {
    pub default_cap_location: Option<String>,
}

impl FigureFilter {
    pub fn new(default_cap_location: Option<String>) -> Self {
        Self { default_cap_location }
    }
}

impl Filter for FigureFilter {
    fn apply(&self, element: &Element, format: &str, vars: &mut HashMap<String, String>) -> FilterResult {
        if let Element::Figure { path, alt, caption, label, number, attrs } = element {
            build_figure_vars(
                vars, path, alt, caption.as_deref(), label,
                number.as_deref(), attrs, format,
                self.default_cap_location.as_deref(),
            );
            FilterResult::Continue
        } else {
            FilterResult::Pass
        }
    }
}

fn build_figure_vars(
    vars: &mut HashMap<String, String>,
    path: &Path,
    alt: &str,
    caption: Option<&str>,
    label: &str,
    number: Option<&str>,
    attrs: &crate::types::FigureAttrs,
    format: &str,
    default_cap_location: Option<&str>,
) {
    vars.insert("alt".to_string(), alt.to_string());
    vars.insert("caption".to_string(), caption.unwrap_or("").to_string());
    vars.insert("label".to_string(), label.to_string());
    vars.insert("number".to_string(), number.unwrap_or("").to_string());

    let resolved_path = select_image_variant(path, format);
    let display_path = resolved_path.to_string_lossy().to_string();
    vars.insert("path".to_string(), display_path.clone());

    // Image components for template
    let embed = crate::project::get_defaults().embed_resources.unwrap_or(true);
    if format == "html" && embed {
        if let Ok((mime, data)) = crate::util::base64_encode_image(&resolved_path) {
            vars.insert("src".to_string(), format!("data:{};base64,{}", mime, data));
        } else {
            vars.insert("src".to_string(), crate::util::escape_html(&display_path));
        }
    } else if format == "html" {
        vars.insert("src".to_string(), crate::util::escape_html(&display_path));
    } else {
        // For LaTeX/Typst, use relative figure path
        let rel = relative_figure_path(&resolved_path);
        vars.insert("src".to_string(), rel);
    }

    vars.insert("width_attr".to_string(), format_width(attrs, format));
    vars.insert("height_attr".to_string(), format_height(attrs));

    let defs = crate::project::get_defaults();
    let default_align = defs.figure.as_ref().and_then(|f| f.alignment.as_deref()).unwrap_or("center");
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
    vars.insert("short_caption".to_string(), short_caption.clone());

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
/// Strips everything before the `_calepin/files` component.
fn relative_figure_path(path: &Path) -> String {
    let s = path.display().to_string();
    if let Some(idx) = s.find("_calepin/files") {
        s[idx..].to_string()
    } else {
        s
    }
}

pub fn format_width(attrs: &crate::types::FigureAttrs, format: &str) -> String {
    match format {
        "latex" => match attrs.width.as_deref() {
            Some(w) if w.ends_with('%') => {
                let pct: f64 = w.trim_end_matches('%').parse().unwrap_or(100.0);
                format!("width={:.2}\\textwidth", pct / 100.0)
            }
            Some(w) if w.parse::<u32>().is_ok() => format!("width={}px", w),
            Some(w) => format!("width={}", w),
            None => String::new(),
        },
        _ => attrs.width.clone().unwrap_or_default(),
    }
}

pub fn format_height(attrs: &crate::types::FigureAttrs) -> String {
    attrs.height.clone().unwrap_or_default()
}

pub fn format_align(align: &str, format: &str) -> String {
    match format {
        "html" => match align {
            "left" => "text-align:left".to_string(),
            "right" => "text-align:right".to_string(),
            _ => "text-align:center".to_string(),
        },
        "latex" => match align {
            "left" => "\\raggedright".to_string(),
            "right" => "\\raggedleft".to_string(),
            _ => "\\centering".to_string(),
        },
        "typst" => match align {
            "left" => "left".to_string(),
            "right" => "right".to_string(),
            _ => "center".to_string(),
        },
        _ => String::new(),
    }
}

/// Find the preferred image format variant for the output format.
/// For example, prefers PDF/EPS for LaTeX, SVG/PNG for HTML.
pub fn select_image_variant(path: &Path, format: &str) -> PathBuf {
    let preferred: &[&str] = match format {
        "latex" => &["pdf", "eps", "svg", "png", "jpg"],
        "typst" => &["svg", "png", "jpg"],
        "html" => &["svg", "png", "jpg", "webp", "gif"],
        _ => &["svg", "png", "jpg"],
    };

    if let Some(stem) = path.file_stem() {
        if let Some(parent) = path.parent() {
            for ext in preferred {
                let candidate = parent.join(format!("{}.{}", stem.to_string_lossy(), ext));
                if candidate.exists() {
                    // For LaTeX, convert SVG to PDF on the fly
                    if format == "latex" && *ext == "svg" {
                        match super::svg::convert_svg_to_pdf(&candidate) {
                            Ok(pdf_path) => return pdf_path,
                            Err(e) => {
                                cwarn!("SVG→PDF conversion failed for {}: {}", candidate.display(), e);
                                return candidate;
                            }
                        }
                    }
                    return candidate;
                }
            }
        }
    }

    // If the path itself is an SVG and we're targeting LaTeX, convert it
    if format == "latex" && path.extension().is_some_and(|e| e == "svg") && path.exists() {
        match super::svg::convert_svg_to_pdf(path) {
            Ok(pdf_path) => return pdf_path,
            Err(e) => {
                cwarn!("SVG→PDF conversion failed for {}: {}", path.display(), e);
            }
        }
    }

    path.to_path_buf()
}

