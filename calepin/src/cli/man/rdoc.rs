//! Rd AST to markdown converter.
//!
//! Parses the JSON AST produced by extract_rdocs.R and emits clean
//! markdown-style .qmd files with TOML front matter.

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct RdTopic {
    pub topic: String,
    pub nodes: Vec<RdNode>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum RdNode {
    Text { tag: String, text: String },
    Parent { tag: String, children: Vec<RdNode> },
}

impl RdNode {
    fn tag(&self) -> &str {
        match self {
            RdNode::Text { tag, .. } | RdNode::Parent { tag, .. } => tag,
        }
    }

    fn children(&self) -> &[RdNode] {
        match self {
            RdNode::Parent { children, .. } => children,
            RdNode::Text { .. } => &[],
        }
    }

    fn text(&self) -> &str {
        match self {
            RdNode::Text { text, .. } => text,
            RdNode::Parent { .. } => "",
        }
    }
}

/// Convert a full Rd topic to a .qmd string.
pub fn rd_to_qmd(topic: &RdTopic) -> String {
    let mut title = String::new();
    let mut sections: Vec<(String, String)> = Vec::new();

    for node in &topic.nodes {
        match node.tag() {
            "\\title" => title = collect_text(node),
            "\\description" => sections.push(("Description".into(), render_section_body(node))),
            "\\usage" => sections.push(("Usage".into(), render_usage(node))),
            "\\arguments" => sections.push(("Arguments".into(), render_arguments(node))),
            "\\details" => sections.push(("Details".into(), render_section_body(node))),
            "\\value" => sections.push(("Value".into(), render_section_body(node))),
            "\\note" => sections.push(("Note".into(), render_section_body(node))),
            "\\references" => sections.push(("References".into(), render_section_body(node))),
            "\\seealso" => sections.push(("See Also".into(), render_section_body(node))),
            "\\author" => sections.push(("Author".into(), render_section_body(node))),
            "\\examples" => sections.push(("Examples".into(), render_examples(node))),
            "\\section" => {
                let children = node.children();
                if !children.is_empty() {
                    let sec_title = collect_text(&children[0]).trim().to_string();
                    let body = if children.len() > 1 {
                        render_children(&children[1..], 0)
                    } else {
                        String::new()
                    };
                    sections.push((sec_title, body.trim().to_string()));
                }
            }
            _ => {} // \name, \alias, \keyword, \concept -- skip
        }
    }

    let mut out = format!("---\ntitle = \"`{}`\"\n---\n", topic.topic);
    if !title.is_empty() {
        out.push_str(&format!("\n*{}*\n", title.trim()));
    }
    for (heading, body) in &sections {
        out.push_str(&format!("\n## {}\n\n{}\n", heading, body));
    }
    out
}

// ---------------------------------------------------------------------------
// Node rendering
// ---------------------------------------------------------------------------

/// Render a node to markdown.
fn render_node(node: &RdNode, depth: usize) -> String {
    match node.tag() {
        "TEXT" => node.text().to_string(),
        "RCODE" | "VERB" => node.text().to_string(),
        "COMMENT" => String::new(),
        "GROUP" => render_children(node.children(), depth),
        "\\code" => format!("`{}`", collect_text(node)),
        "\\bold" | "\\strong" => format!("**{}**", render_children(node.children(), depth)),
        "\\emph" | "\\var" => format!("*{}*", render_children(node.children(), depth)),
        "\\pkg" => format!("**{}**", render_children(node.children(), depth)),
        "\\link" => format!("`{}`", collect_text(node)),
        "\\file" | "\\env" | "\\option" | "\\command" => {
            format!("`{}`", collect_text(node))
        }
        "\\url" => format!("<{}>", collect_text(node)),
        "\\href" => {
            let children = node.children();
            let url = if !children.is_empty() { collect_text(&children[0]) } else { String::new() };
            let text = if children.len() >= 2 {
                render_children(&children[1..2], depth)
            } else {
                url.clone()
            };
            format!("[{}]({})", text, url)
        }
        "\\email" => format!("<{}>", collect_text(node)),
        "\\doi" => {
            let doi = collect_text(node);
            format!("[doi:{}](https://doi.org/{})", doi, doi)
        }
        "\\eqn" => {
            let children = node.children();
            let latex = if !children.is_empty() { collect_text(&children[0]) } else { String::new() };
            format!("${}$", latex)
        }
        "\\deqn" => {
            let children = node.children();
            let latex = if !children.is_empty() { collect_text(&children[0]) } else { String::new() };
            format!("\n$$\n{}\n$$\n", latex)
        }
        "\\cr" => "\n".to_string(),
        "\\dots" | "\\ldots" => "...".to_string(),
        "\\R" => "R".to_string(),
        "\\sQuote" => format!("'{}'", render_children(node.children(), depth)),
        "\\dQuote" => format!("\"{}\"", render_children(node.children(), depth)),
        "\\acronym" => render_children(node.children(), depth),
        "\\preformatted" => format!("\n```\n{}\n```\n", collect_text(node)),
        "\\tabular" => render_tabular(node),
        "\\itemize" => render_itemize(node, depth),
        "\\enumerate" => render_enumerate(node, depth),
        "\\describe" => render_describe(node, depth),
        "\\item" => String::new(), // handled by parent list functions
        _ => render_children(node.children(), depth),
    }
}

/// Render all children and concatenate.
fn render_children(nodes: &[RdNode], depth: usize) -> String {
    let mut out = String::new();
    for node in nodes {
        out.push_str(&render_node(node, depth));
    }
    out
}

/// Collect plain text from a node, stripping all markup.
fn collect_text(node: &RdNode) -> String {
    match node {
        RdNode::Text { text, .. } => text.clone(),
        RdNode::Parent { children, .. } => {
            children.iter().map(collect_text).collect::<Vec<_>>().join("")
        }
    }
}

// ---------------------------------------------------------------------------
// Section renderers
// ---------------------------------------------------------------------------

fn render_section_body(node: &RdNode) -> String {
    render_children(node.children(), 0).trim().to_string()
}

fn render_usage(node: &RdNode) -> String {
    let code = collect_text(node);
    let lines: Vec<&str> = code.lines()
        .map(|l| l.trim_end())
        .collect();
    // Trim empty leading/trailing lines
    let start = lines.iter().position(|l| !l.trim().is_empty()).unwrap_or(0);
    let end = lines.iter().rposition(|l| !l.trim().is_empty()).map(|i| i + 1).unwrap_or(lines.len());
    format!("```r\n{}\n```", lines[start..end].join("\n"))
}

/// Render \examples by walking the AST to distinguish executable code from
/// display-only (\dontrun) and hidden (\dontshow) blocks.
///
/// - Regular RCODE/TEXT and \donttest -> ```{r} (executed by Calepin)
/// - \dontrun -> ```r (display only)
/// - \dontshow -> omitted
fn render_examples(node: &RdNode) -> String {
    // Collect spans of (code_text, executable)
    let mut spans: Vec<(String, bool)> = Vec::new();
    collect_example_spans(node.children(), true, &mut spans);

    // Filter out whitespace-only spans, then merge adjacent same-flag spans
    let spans: Vec<_> = spans.into_iter().filter(|(t, _)| !t.trim().is_empty()).collect();
    let mut merged: Vec<(String, bool)> = Vec::new();
    for (text, exec) in spans {
        if let Some(last) = merged.last_mut() {
            if last.1 == exec {
                last.0.push_str(&text);
                continue;
            }
        }
        merged.push((text, exec));
    }

    let mut out = String::new();
    for (code, exec) in &merged {
        let trimmed = code.trim();
        if trimmed.is_empty() { continue; }
        let lines: Vec<&str> = trimmed.lines().map(|l| l.trim_end()).collect();
        let body = lines.join("\n");
        if *exec {
            out.push_str(&format!("```{{r}}\n{}\n```\n\n", body));
        } else {
            out.push_str(&format!("```r\n{}\n```\n\n", body));
        }
    }
    out.trim_end().to_string()
}

fn collect_example_spans(nodes: &[RdNode], executable: bool, out: &mut Vec<(String, bool)>) {
    for node in nodes {
        match node.tag() {
            "TEXT" | "RCODE" | "VERB" => {
                out.push((node.text().to_string(), executable));
            }
            "\\dontrun" => {
                // Display only -- not executed
                collect_example_spans(node.children(), false, out);
            }
            "\\donttest" => {
                // Valid code, just slow -- still execute
                collect_example_spans(node.children(), executable, out);
            }
            "\\dontshow" => {
                // Internal test code -- omit entirely
            }
            _ => {
                // Other tags (e.g., \code inside examples): collect text
                out.push((collect_text(node), executable));
            }
        }
    }
}

fn render_arguments(node: &RdNode) -> String {
    let mut out = String::new();
    for child in node.children() {
        if child.tag() != "\\item" { continue; }
        let children = child.children();
        if children.is_empty() { continue; }
        let name = collect_text(&children[0]).trim().to_string();
        let desc = if children.len() >= 2 {
            render_children(&children[1..], 0).trim().to_string()
        } else {
            String::new()
        };
        out.push_str(&format!("**`{}`**\n: {}\n\n", name, desc));
    }
    out.trim_end().to_string()
}

// ---------------------------------------------------------------------------
// List renderers
// ---------------------------------------------------------------------------

/// \itemize: \item is a marker, content follows as siblings until next \item.
fn render_itemize(node: &RdNode, depth: usize) -> String {
    let indent = "  ".repeat(depth);
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for child in node.children() {
        if child.tag() == "\\item" {
            let text = current.trim().to_string();
            if !text.is_empty() {
                lines.push(format!("{}- {}", indent, text));
            }
            current.clear();
        } else {
            current.push_str(&render_node(child, depth + 1));
        }
    }
    let text = current.trim().to_string();
    if !text.is_empty() {
        lines.push(format!("{}- {}", indent, text));
    }

    format!("\n{}\n", lines.join("\n"))
}

fn render_enumerate(node: &RdNode, depth: usize) -> String {
    let indent = "  ".repeat(depth);
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut counter = 0usize;

    for child in node.children() {
        if child.tag() == "\\item" {
            let text = current.trim().to_string();
            if !text.is_empty() {
                lines.push(format!("{}{}. {}", indent, counter, text));
            }
            current.clear();
            counter += 1;
        } else {
            current.push_str(&render_node(child, depth + 1));
        }
    }
    let text = current.trim().to_string();
    if !text.is_empty() {
        lines.push(format!("{}{}. {}", indent, counter, text));
    }

    format!("\n{}\n", lines.join("\n"))
}

/// \describe: \item has 2 children (term, description).
fn render_describe(node: &RdNode, depth: usize) -> String {
    let indent = "  ".repeat(depth);
    let mut lines: Vec<String> = Vec::new();

    for child in node.children() {
        if child.tag() != "\\item" { continue; }
        let children = child.children();
        if children.is_empty() { continue; }
        let term = render_children(&children[0..1], depth + 1).trim().to_string();
        let desc = if children.len() >= 2 {
            render_children(&children[1..], depth + 1).trim().to_string()
        } else {
            String::new()
        };
        lines.push(format!("{}- **{}**: {}", indent, term, desc));
    }

    format!("\n{}\n", lines.join("\n"))
}

// ---------------------------------------------------------------------------
// Table renderer
// ---------------------------------------------------------------------------

fn render_tabular(node: &RdNode) -> String {
    let children = node.children();
    // children[0] is column spec (TEXT), children[1] is content
    if children.len() < 2 { return String::new(); }
    let content = &children[1];

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut current_cell = String::new();

    for child in content.children() {
        match child.tag() {
            "\\tab" => {
                current_row.push(current_cell.trim().to_string());
                current_cell.clear();
            }
            "\\cr" => {
                current_row.push(current_cell.trim().to_string());
                rows.push(current_row);
                current_row = Vec::new();
                current_cell.clear();
            }
            _ => current_cell.push_str(&render_node(child, 0)),
        }
    }
    if !current_row.is_empty() || !current_cell.is_empty() {
        current_row.push(current_cell.trim().to_string());
        rows.push(current_row);
    }

    if rows.is_empty() { return String::new(); }
    let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0);

    let mut lines: Vec<String> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let mut padded = row.clone();
        padded.resize(ncols, String::new());
        lines.push(format!("| {} |", padded.join(" | ")));
        if i == 0 {
            lines.push(format!("| {} |", vec!["---"; ncols].join(" | ")));
        }
    }

    format!("\n{}\n", lines.join("\n"))
}
