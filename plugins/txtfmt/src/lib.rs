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

// ---------------------------------------------------------------------------
// Color dictionary (generated at build time from color_dict.csv, binary search)
// ---------------------------------------------------------------------------

include!(concat!(env!("OUT_DIR"), "/colors.rs"));

/// Resolve a color value to a hex string.
/// Accepts "#hex" (passthrough) or a color name (looked up via binary search).
fn resolve_color(color: &str) -> Option<String> {
    if color.starts_with('#') {
        return Some(color.to_string());
    }
    let normalized: String = color
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase();
    COLORS
        .binary_search_by_key(&&*normalized, |(name, _)| name)
        .ok()
        .map(|i| COLORS[i].1.to_string())
}

#[plugin_fn]
pub fn filter(Json(ctx): Json<FilterContext>) -> FnResult<Json<FilterResult>> {
    if ctx.context != "span" || !ctx.classes.iter().any(|c| c == "txtfmt") {
        return Ok(Json(FilterResult::Pass));
    }

    let content = &ctx.content;
    let em = ctx.attrs.get("em");
    let color = ctx.attrs.get("color").and_then(|c| resolve_color(c));
    let smallcaps = ctx.attrs.get("smallcaps").is_some_and(|v| v == "true");
    let underline = ctx.attrs.get("underline").is_some_and(|v| v == "true");
    let mark = ctx.attrs.get("mark").is_some_and(|v| v == "true");

    let output = match ctx.format.as_str() {
        "html" => render_html(content, em, color.as_ref(), smallcaps, underline, mark),
        "tex" => render_tex(content, em, color.as_ref(), smallcaps, underline, mark),
        "typ" => render_typst(content, em, color.as_ref(), smallcaps, underline, mark),
        _ => content.to_string(),
    };

    Ok(Json(FilterResult::Rendered(output)))
}

fn render_html(
    content: &str,
    em: Option<&String>,
    color: Option<&String>,
    smallcaps: bool,
    underline: bool,
    mark: bool,
) -> String {
    let mut styles = Vec::new();
    if let Some(em) = em {
        styles.push(format!("font-size: {}em", em));
    }
    if let Some(hex) = color {
        styles.push(format!("color: {}", hex));
    }
    if smallcaps {
        styles.push("font-variant: small-caps".to_string());
    }
    if underline {
        styles.push("text-decoration: underline".to_string());
    }

    let mut result = content.to_string();
    if mark {
        result = format!("<mark>{}</mark>", result);
    }
    if styles.is_empty() {
        result
    } else {
        format!("<span style=\"{}\">{}</span>", styles.join("; "), result)
    }
}

fn render_tex(
    content: &str,
    em: Option<&String>,
    color: Option<&String>,
    smallcaps: bool,
    underline: bool,
    mark: bool,
) -> String {
    let mut result = content.to_string();
    if mark {
        result = format!("\\hl{{{}}}", result);
    }
    if underline {
        result = format!("\\underline{{{}}}", result);
    }
    if smallcaps {
        result = format!("\\textsc{{{}}}", result);
    }
    if let Some(hex) = color {
        // Always use HTML hex mode for consistent cross-format results
        let hex_digits = hex.strip_prefix('#').unwrap_or(hex);
        result = format!(
            "\\textcolor[HTML]{{{}}}{{{}}}",
            hex_digits.to_uppercase(),
            result
        );
    }
    if let Some(em) = em {
        if let Ok(scale) = em.parse::<f64>() {
            let size_pt = scale * 10.0;
            let skip_pt = size_pt * 1.2;
            result = format!(
                "{{\\fontsize{{{:.1}pt}}{{{:.1}pt}}\\selectfont {}}}",
                size_pt, skip_pt, result
            );
        }
    }
    result
}

fn render_typst(
    content: &str,
    em: Option<&String>,
    color: Option<&String>,
    smallcaps: bool,
    underline: bool,
    mark: bool,
) -> String {
    let mut result = content.to_string();
    if mark {
        result = format!("#highlight[{}]", result);
    }
    if underline {
        result = format!("#underline[{}]", result);
    }
    if smallcaps {
        result = format!("#smallcaps[{}]", result);
    }
    let mut args = Vec::new();
    if let Some(em) = em {
        args.push(format!("size: {}em", em));
    }
    if let Some(hex) = color {
        // Always use rgb() for consistent cross-format results
        args.push(format!("fill: rgb(\"{}\")", hex));
    }
    if !args.is_empty() {
        result = format!("#text({})[{}]", args.join(", "), result);
    }
    result
}
