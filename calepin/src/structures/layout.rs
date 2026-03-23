//! Layout filter: renders `layout-ncol`, `layout-nrow`, and `layout` divs as grids.

use std::collections::HashMap;

use crate::types::Element;
use crate::structures::figure;

/// Render a layout div with grid-based layout.
pub fn render(
    id: &Option<String>,
    attrs: &HashMap<String, String>,
    children: &[Element],
    format: &str,
    render_element: &dyn Fn(&Element) -> String,
    raw_fragments: &std::cell::RefCell<Vec<String>>,
) -> String {
    let valign = attrs.get("layout-valign").map(|s| s.as_str()).unwrap_or("top");

    let is_figure = id.as_ref().map_or(false, |id| id.starts_with("fig-"));

    let (content_children, caption) = if is_figure {
        if let Some(cap) = attrs.get("fig-cap") {
            (children.to_vec(), cap.clone())
        } else {
            figure::separate_figure_caption(children)
        }
    } else {
        (children.to_vec(), String::new())
    };

    // Parse spec using content children count (after caption separation)
    let layout_spec = parse_spec(attrs, content_children.len());

    let mut child_idx = 0;
    let mut rows_rendered: Vec<Vec<(String, f64)>> = Vec::new();

    for row_spec in &layout_spec {
        let mut row: Vec<(String, f64)> = Vec::new();
        for &width_frac in row_spec {
            if width_frac < 0.0 {
                row.push((String::new(), width_frac));
            } else if child_idx < content_children.len() {
                let rendered = render_element(&content_children[child_idx]);
                row.push((rendered, width_frac));
                child_idx += 1;
            }
        }
        rows_rendered.push(row);
    }

    let id_str = id.as_deref().unwrap_or("");

    match format {
        "html" => render_html(&rows_rendered, id_str, &caption, valign, is_figure, raw_fragments),
        "latex" => render_latex(&rows_rendered, id_str, &caption, valign, is_figure, attrs),
        "typst" => render_typst(&rows_rendered, id_str, &caption, valign, is_figure),
        _ => render_plain(&rows_rendered, &caption),
    }
}

fn render_html(
    rows: &[Vec<(String, f64)>],
    id: &str,
    caption: &str,
    valign: &str,
    is_figure: bool,
    _raw_fragments: &std::cell::RefCell<Vec<String>>,
) -> String {
    let align_items = match valign {
        "center" => "center",
        "bottom" => "end",
        _ => "start",
    };

    let mut html = String::new();
    if is_figure && !id.is_empty() {
        html.push_str(&format!("<div class=\"figure\" id=\"{}\">\n", id));
    }

    for row in rows {
        let cols: Vec<String> = row.iter()
            .filter(|(_, w)| *w > 0.0)
            .map(|(_, w)| format!("{}fr", (w * 100.0).round() as u32))
            .collect();
        html.push_str(&format!(
            "<div class=\"layout-grid\" style=\"display:grid;grid-template-columns:{};gap:1em;align-items:{}\">\n",
            cols.join(" "), align_items
        ));
        for (content, w) in row {
            if *w < 0.0 { continue; }
            // Images inside layout cells should fill the cell width
            let content = content.replace("max-width: 60%", "max-width: 100%");
            html.push_str(&format!("<div class=\"layout-cell\">\n{}\n</div>\n", content));
        }
        html.push_str("</div>\n");
    }

    if is_figure && !caption.is_empty() {
        let cap_clean = crate::render::markdown::render_inline(caption, "html");
        html.push_str(&format!("<p class=\"caption\">{}</p>\n", cap_clean));
    }

    if is_figure && !id.is_empty() {
        html.push_str("</div>");
    }

    html
}

fn render_latex(
    rows: &[Vec<(String, f64)>],
    id: &str,
    caption: &str,
    valign: &str,
    is_figure: bool,
    attrs: &HashMap<String, String>,
) -> String {
    let valign_char = match valign {
        "center" => "c",
        "bottom" => "b",
        _ => "t",
    };

    let mut latex = String::new();
    if is_figure {
        let env = attrs.get("fig-env").map(|s| s.as_str()).unwrap_or("figure");
        let pos = attrs.get("fig-pos").map(|s| format!("[{}]", s)).unwrap_or_default();
        latex.push_str(&format!("\\begin{{{}}}{}\n\\centering\n", env, pos));
    }

    for row in rows {
        let positive_cells: Vec<&(String, f64)> = row.iter().filter(|(_, w)| *w > 0.0).collect();
        let total: f64 = positive_cells.iter().map(|(_, w)| w).sum();
        let gap = if positive_cells.len() > 1 { 0.02 } else { 0.0 };

        for (i, (content, w)) in positive_cells.iter().enumerate() {
            let width = w / total * (1.0 - gap * (positive_cells.len() as f64 - 1.0));
            // Strip nested \begin{figure}...\end{figure} from children to avoid
            // "not in outer par mode" errors. Keep the inner content (centering,
            // includegraphics, caption, label).
            let inner = unwrap_latex_figure(content);
            // Images inside layout cells should fill the cell width
            let inner = inner.replace("width=0.60\\textwidth", "width=\\textwidth");
            latex.push_str(&format!(
                "\\begin{{minipage}}[{}]{{{:.3}\\textwidth}}\n{}\n\\end{{minipage}}",
                valign_char, width, inner
            ));
            if i < positive_cells.len() - 1 {
                latex.push_str("\\hfill\n");
            }
        }
        latex.push('\n');
    }

    if is_figure {
        if !caption.is_empty() {
            latex.push_str(&format!("\\caption{{{}}}\n", caption));
        }
        if !id.is_empty() {
            latex.push_str(&format!("\\label{{{}}}\n", id));
        }
        let env = attrs.get("fig-env").map(|s| s.as_str()).unwrap_or("figure");
        latex.push_str(&format!("\\end{{{}}}", env));
    }

    latex
}

/// Strip `\begin{figure}...\end{figure}` wrapper from LaTeX content,
/// keeping the inner body. This prevents nested figure floats inside minipages.
fn unwrap_latex_figure(content: &str) -> &str {
    let trimmed = content.trim();
    // Try common figure environments
    for env in &["figure", "figure*"] {
        let begin = format!("\\begin{{{}}}", env);
        let end = format!("\\end{{{}}}", env);
        if let Some(rest) = trimmed.strip_prefix(&begin) {
            if let Some(inner) = rest.strip_suffix(&end) {
                // Skip optional position specifier like [htbp]
                let inner = inner.trim();
                let inner = if inner.starts_with('[') {
                    inner.find(']').map_or(inner, |i| &inner[i + 1..])
                } else {
                    inner
                };
                return inner.trim();
            }
        }
    }
    content
}

fn render_typst(
    rows: &[Vec<(String, f64)>],
    id: &str,
    caption: &str,
    _valign: &str,
    is_figure: bool,
) -> String {
    let mut typ = String::new();

    if is_figure {
        typ.push_str("#figure([\n");
    }

    for row in rows {
        let positive_cells: Vec<&(String, f64)> = row.iter().filter(|(_, w)| *w > 0.0).collect();
        let total: f64 = positive_cells.iter().map(|(_, w)| w).sum();
        let cols: Vec<String> = positive_cells.iter()
            .map(|(_, w)| format!("{}fr", (w / total * 100.0).round() as u32))
            .collect();

        typ.push_str(&format!("#grid(columns: ({}), gutter: 1em,\n", cols.join(", ")));
        for (content, _) in &positive_cells {
            // Images inside layout cells should fill the cell width
            let content = content.replace("width: 60%", "width: 100%");
            typ.push_str(&format!("  [\n{}\n  ],\n", content));
        }
        typ.push_str(")\n");
    }

    if is_figure {
        if !caption.is_empty() {
            typ.push_str(&format!("], caption: [{}])", caption));
        } else {
            typ.push_str("])");
        }
        if !id.is_empty() {
            typ.push_str(&format!(" <{}>", id));
        }
    }

    typ
}

fn render_plain(rows: &[Vec<(String, f64)>], caption: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for row in rows {
        for (content, w) in row {
            if *w > 0.0 && !content.is_empty() {
                parts.push(content.clone());
            }
        }
    }
    if !caption.is_empty() {
        parts.push(format!("*{}*", caption));
    }
    parts.join("\n\n")
}

// ---------------------------------------------------------------------------
// Layout spec parsing
// ---------------------------------------------------------------------------

/// Parse layout specification from div attributes.
pub fn parse_spec(attrs: &HashMap<String, String>, num_children: usize) -> Vec<Vec<f64>> {
    if let Some(layout_str) = attrs.get("layout") {
        return parse_custom(layout_str);
    }

    if let Some(ncol_str) = attrs.get("layout-ncol") {
        let ncol: usize = ncol_str.parse().unwrap_or(1).max(1);
        let width = 1.0 / ncol as f64;
        let mut rows = Vec::new();
        let mut row = Vec::new();
        for i in 0..num_children {
            row.push(width);
            if row.len() == ncol || i == num_children - 1 {
                rows.push(row);
                row = Vec::new();
            }
        }
        return rows;
    }

    if let Some(nrow_str) = attrs.get("layout-nrow") {
        let nrow: usize = nrow_str.parse().unwrap_or(1).max(1);
        let ncol = (num_children + nrow - 1) / nrow;
        let width = 1.0 / ncol as f64;
        let mut rows = Vec::new();
        let mut row = Vec::new();
        for i in 0..num_children {
            row.push(width);
            if row.len() == ncol || i == num_children - 1 {
                rows.push(row);
                row = Vec::new();
            }
        }
        return rows;
    }

    vec![vec![1.0]; num_children]
}

/// Parse a custom layout string like `[[1,1],[1]]` or `[[40,-20,40],[100]]`.
pub fn parse_custom(s: &str) -> Vec<Vec<f64>> {
    let s = s.trim();
    let mut rows = Vec::new();

    let inner = s.strip_prefix('[').unwrap_or(s)
        .strip_suffix(']').unwrap_or(s);

    let mut depth = 0;
    let mut start = 0;
    let bytes = inner.as_bytes();
    for i in 0..bytes.len() {
        match bytes[i] {
            b'[' => {
                if depth == 0 { start = i; }
                depth += 1;
            }
            b']' => {
                depth -= 1;
                if depth == 0 {
                    let row_str = &inner[start + 1..i];
                    let values: Vec<f64> = row_str.split(',')
                        .filter_map(|v| v.trim().parse::<f64>().ok())
                        .collect();
                    if !values.is_empty() {
                        let total: f64 = values.iter().map(|v| v.abs()).sum();
                        if total > 0.0 {
                            rows.push(values.iter().map(|v| v / total).collect());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if rows.is_empty() { vec![vec![1.0]] } else { rows }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_layout_ncol() {
        let mut attrs = HashMap::new();
        attrs.insert("layout-ncol".to_string(), "2".to_string());
        let spec = parse_spec(&attrs, 4);
        assert_eq!(spec.len(), 2);
        assert_eq!(spec[0].len(), 2);
        assert!((spec[0][0] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_parse_layout_nrow() {
        let mut attrs = HashMap::new();
        attrs.insert("layout-nrow".to_string(), "2".to_string());
        let spec = parse_spec(&attrs, 4);
        assert_eq!(spec.len(), 2);
        assert_eq!(spec[0].len(), 2);
    }

    #[test]
    fn test_parse_custom_layout() {
        let rows = parse_custom("[[1,1],[1]]");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].len(), 2);
        assert!((rows[0][0] - 0.5).abs() < 0.01);
        assert_eq!(rows[1].len(), 1);
        assert!((rows[1][0] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_custom_layout_with_spacing() {
        let rows = parse_custom("[[40,-20,40]]");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].len(), 3);
        assert!(rows[0][1] < 0.0);
    }
}
