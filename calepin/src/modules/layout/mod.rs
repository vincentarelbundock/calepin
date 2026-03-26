//! Layout filter: renders `layout-ncol`, `layout-nrow`, and `layout` divs as grids.

use std::collections::HashMap;

use crate::types::Element;
use crate::modules::figure;

/// Render a layout div with grid-based layout.
pub fn render(
    id: &Option<String>,
    attrs: &HashMap<String, String>,
    children: &[Element],
    format: &str,
    render_element: &dyn Fn(&Element) -> String,
    _raw_fragments: &std::cell::RefCell<Vec<String>>,
    defaults: &crate::config::Metadata,
) -> String {
    let defs = defaults;
    let default_valign = defs.layout.as_ref().and_then(|l| l.valign.clone()).unwrap_or_else(|| "top".to_string());
    let valign = attrs.get("layout_valign").map(|s| s.as_str()).unwrap_or(&default_valign);

    let is_figure = id.as_ref().map_or(false, |id| id.starts_with("fig-"));

    let (content_children, caption) = if is_figure {
        if let Some(cap) = attrs.get("fig_cap") {
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

    // Render the inner rows through format-specific partials
    let rows_content = render_rows_via_partials(&rows_rendered, valign, format);

    // Build template variables for the figure wrapper
    let mut vars = HashMap::new();
    vars.insert("base".to_string(), format.to_string());
    vars.insert("writer".to_string(), format.to_string());
    vars.insert("id".to_string(), id_str.to_string());
    vars.insert("caption".to_string(), if !caption.is_empty() {
        crate::render::convert::render_inline(&caption, format)
    } else {
        String::new()
    });
    vars.insert("is_figure".to_string(), if is_figure { "true" } else { "" }.to_string());
    vars.insert("rows".to_string(), rows_content);

    // LaTeX-specific attrs
    let fig_env = attrs.get("fig_env").map(|s| s.as_str()).unwrap_or("figure");
    let fig_pos = attrs.get("fig_pos").map(|s| format!("[{}]", s)).unwrap_or_default();
    vars.insert("fig_env".to_string(), fig_env.to_string());
    vars.insert("fig_pos".to_string(), fig_pos);

    let tpl = crate::render::elements::resolve_builtin_partial("layout", format).unwrap_or("");
    crate::render::template::apply_template(tpl, &vars)
}

/// Render rows through format-specific layout_row and layout_cell partials.
fn render_rows_via_partials(
    rows: &[Vec<(String, f64)>],
    valign: &str,
    format: &str,
) -> String {
    use crate::render::elements::resolve_builtin_partial;
    use crate::render::template::apply_template;

    let cell_tpl = resolve_builtin_partial("layout_cell", format).unwrap_or("{{ content }}");
    let row_tpl = resolve_builtin_partial("layout_row", format).unwrap_or("{{ cells }}");

    let valign_char = match valign {
        "center" => "c",
        "bottom" => "b",
        _ => "t",
    };
    let align_items = match valign {
        "center" => "center",
        "bottom" => "end",
        _ => "start",
    };

    let mut output = String::new();
    for row in rows {
        let positive_cells: Vec<&(String, f64)> = row.iter().filter(|(_, w)| *w > 0.0).collect();
        let total: f64 = positive_cells.iter().map(|(_, w)| w).sum();
        let gap = if positive_cells.len() > 1 { 0.02 } else { 0.0 };

        // Build column spec
        let columns: Vec<String> = positive_cells.iter()
            .map(|(_, w)| format!("{}fr", (w / total * 100.0).round() as u32))
            .collect();

        // Render each cell
        let mut cells_rendered: Vec<String> = Vec::new();
        for (i, (content, w)) in positive_cells.iter().enumerate() {
            let width = w / total * (1.0 - gap * (positive_cells.len() as f64 - 1.0));

            // Format-specific content adjustments
            let content = adjust_cell_content(content, format);

            let mut cell_vars = HashMap::new();
            cell_vars.insert("content".to_string(), content);
            cell_vars.insert("width".to_string(), format!("{:.3}", width));
            cell_vars.insert("valign".to_string(), valign_char.to_string());
            cells_rendered.push(apply_template(cell_tpl, &cell_vars));

            // LaTeX: add \hfill between cells
            if format == "latex" && i < positive_cells.len() - 1 {
                cells_rendered.push("\\hfill".to_string());
            }
        }

        let cell_separator = if format == "latex" { "\n" } else { "\n" };

        let mut row_vars = HashMap::new();
        row_vars.insert("cells".to_string(), cells_rendered.join(cell_separator));
        row_vars.insert("columns".to_string(), columns.join(" "));
        row_vars.insert("align_items".to_string(), align_items.to_string());
        row_vars.insert("valign".to_string(), valign_char.to_string());
        output.push_str(&apply_template(row_tpl, &row_vars));
    }

    output
}

/// Adjust cell content for the target format (e.g. expand images to fill cells,
/// strip nested figure floats for LaTeX).
fn adjust_cell_content(content: &str, format: &str) -> String {
    match format {
        "html" => content.replace("max-width: 60%", "max-width: 100%"),
        "latex" => {
            let inner = unwrap_latex_figure(content);
            inner.replace("width=0.60\\textwidth", "width=\\textwidth")
        }
        "typst" => content.replace("width: 60%", "width: 100%"),
        _ => content.to_string(),
    }
}

/// Strip `\begin{figure}...\end{figure}` wrapper from LaTeX content,
/// keeping the inner body. This prevents nested figure floats inside minipages.
fn unwrap_latex_figure(content: &str) -> String {
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
                return inner.trim().to_string();
            }
        }
    }
    content.to_string()
}

// ---------------------------------------------------------------------------
// Layout spec parsing
// ---------------------------------------------------------------------------

/// Parse layout specification from div attributes.
pub fn parse_spec(attrs: &HashMap<String, String>, num_children: usize) -> Vec<Vec<f64>> {
    if let Some(layout_str) = attrs.get("layout") {
        return parse_custom(layout_str);
    }

    if let Some(ncol_str) = attrs.get("layout_ncol") {
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

    if let Some(nrow_str) = attrs.get("layout_nrow") {
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
        attrs.insert("layout_ncol".to_string(), "2".to_string());
        let spec = parse_spec(&attrs, 4);
        assert_eq!(spec.len(), 2);
        assert_eq!(spec[0].len(), 2);
        assert!((spec[0][0] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_parse_layout_nrow() {
        let mut attrs = HashMap::new();
        attrs.insert("layout_nrow".to_string(), "2".to_string());
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
