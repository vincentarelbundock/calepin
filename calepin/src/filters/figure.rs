// Figure filter: fills template variables for Figure elements.
//
// - FigureFilter::apply()  — Dispatch to build_figure_vars for Figure elements.
// - build_figure_vars()    — Populate all figure template vars (image tag, dimensions,
//                            alignment, caption location, link wrapping).
// - render_image()         — Format-specific image tag (base64 HTML, \includegraphics,
//                            image(), ![alt](path)).
// - resolve_path()         — Find preferred image format variant for the output format.
// - format_width/height/align() — Dimension and alignment formatting per format.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;

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

    let resolved_path = resolve_path(path, format);
    let display_path = resolved_path.to_string_lossy().to_string();
    vars.insert("path".to_string(), display_path.clone());

    let image_tag = render_image(&resolved_path, alt, attrs, format);
    vars.insert("image".to_string(), image_tag);

    vars.insert("width_attr".to_string(), format_width(attrs, format));
    vars.insert("height_attr".to_string(), format_height(attrs));

    let align = attrs.fig_align.as_deref().unwrap_or("center");
    vars.insert("align_style".to_string(), format_align(align, format));
    vars.insert("align".to_string(), align.to_string());

    let fig_env = attrs.fig_env.as_deref().unwrap_or("figure");
    vars.insert("fig_env".to_string(), fig_env.to_string());
    vars.insert("fig_begin".to_string(), format!("\\begin{{{}}}", fig_env));
    vars.insert("fig_end".to_string(), format!("\\end{{{}}}", fig_env));
    vars.insert("fig_pos".to_string(), match attrs.fig_pos.as_deref() {
        Some(pos) => format!("[{}]", pos),
        None => String::new(),
    });
    let short_caption = match attrs.fig_scap.as_deref() {
        Some(sc) => format!("[{}]", sc),
        None => String::new(),
    };
    vars.insert("short_caption".to_string(), short_caption.clone());
    let caption_text = caption.unwrap_or("");
    vars.insert("caption_cmd".to_string(),
        if caption_text.is_empty() {
            String::new()
        } else {
            format!("\\caption{}{{{}}}", short_caption, caption_text)
        }
    );

    let cap_loc = attrs.cap_location.as_deref()
        .or(default_cap_location)
        .unwrap_or("bottom");
    vars.insert("cap_location".to_string(), cap_loc.to_string());

    if let Some(ref link) = attrs.link {
        let img = vars.get("image").cloned().unwrap_or_default();
        match format {
            "html" => vars.insert("image".to_string(), format!("<a href=\"{}\">{}</a>", crate::util::escape_html(link), img)),
            "latex" => vars.insert("image".to_string(), format!("\\href{{{}}}{{{}}}", link, img)),
            "typst" => vars.insert("image".to_string(), format!("#link(\"{}\")[{}]", link, img)),
            _ => vars.insert("image".to_string(), format!("[{}]({})", img, link)),
        };
    }
}

// ---------------------------------------------------------------------------
// Image helpers
// ---------------------------------------------------------------------------

fn render_image(path: &Path, alt: &str, attrs: &crate::types::FigureAttrs, format: &str) -> String {
    let safe_alt = crate::util::escape_html(alt);
    match format {
        "html" => {
            let mut html_attrs = String::new();
            let mut styles: Vec<String> = Vec::new();
            if let Some(ref w) = attrs.width {
                if w.parse::<u32>().is_ok() {
                    html_attrs.push_str(&format!(" width=\"{}\"", crate::util::escape_html(w)));
                } else {
                    styles.push(format!("width:{}", w));
                    styles.push(format!("max-width:{}", w));
                }
            }
            if let Some(ref h) = attrs.height {
                styles.push(format!("height:{}", h));
            }
            if !styles.is_empty() {
                html_attrs.push_str(&format!(" style=\"{}\"", styles.join(";")));
            }
            render_base64_image(path, &safe_alt)
                .map(|tag| {
                    if html_attrs.is_empty() { tag }
                    else { tag.replace("<img ", &format!("<img{} ", html_attrs)) }
                })
                .unwrap_or_else(|_| format!("<img src=\"{}\" alt=\"{}\"{}/>", crate::util::escape_html(&path.display().to_string()), safe_alt, html_attrs))
        }
        "latex" => {
            let width_opt = if attrs.width.is_some() {
                format_width(attrs, format)
            } else {
                "width=0.60\\textwidth".to_string()
            };
            format!("\\includegraphics[{}]{{{}}}", width_opt, path.display())
        }
        "typst" => {
            let mut args = vec![format!("\"{}\"", path.display())];
            args.push(format!("width: {}", match &attrs.width {
                Some(w) => typst_length(w),
                None => "60%".to_string(),
            }));
            if let Some(ref h) = attrs.height {
                args.push(format!("height: {}", typst_length(h)));
            }
            format!("image({})", args.join(", "))
        }
        "markdown" => format!("![{}]({})", alt, path.display()),
        _ => String::new(),
    }
}

fn render_base64_image(path: &Path, alt: &str) -> anyhow::Result<String> {
    let data = std::fs::read(path)
        .with_context(|| format!("Failed to read plot file: {}", path.display()))?;
    let encoded = BASE64.encode(&data);
    let mime = match path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    };
    // alt is already HTML-escaped by the caller
    Ok(format!("<img src=\"data:{};base64,{}\" alt=\"{}\" />", mime, encoded, alt))
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

pub fn typst_length(s: &str) -> String {
    if s.ends_with('%') || s.ends_with("pt") || s.ends_with("in")
        || s.ends_with("cm") || s.ends_with("mm") || s.ends_with("em")
    {
        s.to_string()
    } else if s.parse::<f64>().is_ok() {
        format!("{}pt", s)
    } else {
        s.to_string()
    }
}

pub fn resolve_path(path: &Path, format: &str) -> PathBuf {
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
                    if candidate != path {
                        cwarn!("image: {} → {}", path.display(), candidate.display());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typst_length() {
        assert_eq!(typst_length("80%"), "80%");
        assert_eq!(typst_length("300"), "300pt");
        assert_eq!(typst_length("4in"), "4in");
        assert_eq!(typst_length("2cm"), "2cm");
    }
}
