//! A compact Markdown renderer for docstrings that are neither Google nor
//! NumPy style.
//!
//! `pydocstring` reports such docstrings as `Style::Plain` and hands back their
//! prose unstructured — correctly, since there are no sections to find. But
//! plenty of projects write Markdown in docstrings, and escaping it wholesale
//! renders `## Parameters` and `- item` as literal punctuation. This handles
//! the block constructs that actually show up: headings, fenced code, bullet
//! lists, and paragraphs. Inline spans stay with [`crate::typst_escape`].

use crate::typst_escape::{escape_string, render_prose};

/// Heading level that a top-level Markdown `#` maps to. Definitions render at
/// heading level 2, so their prose starts one level below.
const BASE_HEADING_LEVEL: usize = 3;
const MAX_HEADING_LEVEL: usize = 6;

#[derive(Debug)]
enum Block {
    Heading {
        level: usize,
        text: String,
    },
    Code {
        language: Option<String>,
        body: String,
    },
    List(Vec<ListItem>),
    Paragraph(String),
}

#[derive(Debug)]
struct ListItem {
    text: String,
    children: Vec<ListItem>,
}

/// Does this text use Markdown block constructs? Prose that does not is left
/// to the plain escaper, which keeps its line breaks intact.
pub fn looks_like_markdown(text: &str) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("```")
            || (trimmed.starts_with('#') && trimmed.contains(' '))
            || list_marker(line).is_some()
    })
}

/// Render Markdown source as Typst markup.
pub fn render_markdown(text: &str) -> String {
    let blocks = parse_blocks(text);
    let mut out = Vec::new();
    for block in &blocks {
        out.push(render_block(block));
    }
    out.join("\n\n")
}

/// The indent width and content of a bullet line, if it is one.
fn list_marker(line: &str) -> Option<(usize, &str)> {
    let indent = line.len() - line.trim_start().len();
    let trimmed = line.trim_start();
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = trimmed.strip_prefix(marker) {
            return Some((indent, rest));
        }
    }
    None
}

fn parse_blocks(text: &str) -> Vec<Block> {
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();
    let mut index = 0;

    fn flush(paragraph: &mut Vec<String>, blocks: &mut Vec<Block>) {
        if !paragraph.is_empty() {
            blocks.push(Block::Paragraph(paragraph.join("\n")));
            paragraph.clear();
        }
    }

    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();

        // Fenced code.
        if let Some(fence) = trimmed.strip_prefix("```") {
            flush(&mut paragraph, &mut blocks);
            let language = fence.trim();
            let language = (!language.is_empty()).then(|| language.to_string());
            let mut body = Vec::new();
            index += 1;
            while index < lines.len() && !lines[index].trim_start().starts_with("```") {
                body.push(lines[index]);
                index += 1;
            }
            index += 1; // closing fence
            blocks.push(Block::Code {
                language,
                body: dedent_block(&body),
            });
            continue;
        }

        // ATX heading.
        if trimmed.starts_with('#') {
            let hashes = trimmed.chars().take_while(|c| *c == '#').count();
            let rest = &trimmed[hashes..];
            if hashes <= MAX_HEADING_LEVEL && rest.starts_with(' ') {
                flush(&mut paragraph, &mut blocks);
                blocks.push(Block::Heading {
                    level: (BASE_HEADING_LEVEL + hashes - 1).min(MAX_HEADING_LEVEL),
                    text: rest.trim().to_string(),
                });
                index += 1;
                continue;
            }
        }

        // Bullet list, including its indented continuation lines.
        if list_marker(line).is_some() {
            flush(&mut paragraph, &mut blocks);
            let start = index;
            while index < lines.len() {
                let candidate = lines[index];
                let is_item = list_marker(candidate).is_some();
                let is_continuation = !candidate.trim().is_empty()
                    && candidate.len() - candidate.trim_start().len() > 0;
                if index > start && !is_item && !is_continuation {
                    break;
                }
                if candidate.trim().is_empty() {
                    // A blank line ends the list unless the next line continues it.
                    let continues = lines
                        .get(index + 1)
                        .map(|next| list_marker(next).is_some())
                        .unwrap_or(false);
                    if !continues {
                        break;
                    }
                }
                index += 1;
            }
            blocks.push(Block::List(parse_list(&lines[start..index])));
            continue;
        }

        if line.trim().is_empty() {
            flush(&mut paragraph, &mut blocks);
        } else {
            paragraph.push(line.to_string());
        }
        index += 1;
    }

    flush(&mut paragraph, &mut blocks);
    blocks
}

/// Build the item tree from a run of list lines, nesting by indent width.
fn parse_list(lines: &[&str]) -> Vec<ListItem> {
    let mut items: Vec<(usize, ListItem)> = Vec::new();

    for line in lines {
        if let Some((indent, content)) = list_marker(line) {
            items.push((
                indent,
                ListItem {
                    text: content.trim_end().to_string(),
                    children: Vec::new(),
                },
            ));
        } else if let Some((_, last)) = items.last_mut() {
            // A continuation line belongs to the item above it.
            let text = line.trim();
            if !text.is_empty() {
                last.text.push(' ');
                last.text.push_str(text);
            }
        }
    }

    nest(items)
}

/// Fold a flat, indent-tagged item list into a tree.
fn nest(items: Vec<(usize, ListItem)>) -> Vec<ListItem> {
    let mut roots: Vec<ListItem> = Vec::new();
    // Indent width of each currently open level, outermost first.
    let mut open: Vec<usize> = Vec::new();

    for (indent, item) in items {
        while open.last().map(|level| indent <= *level).unwrap_or(false) {
            open.pop();
        }

        let depth = open.len();
        open.push(indent);

        let mut target = &mut roots;
        for _ in 0..depth {
            if target.is_empty() {
                break;
            }
            let last = target.len() - 1;
            target = &mut target[last].children;
        }
        target.push(item);
    }

    roots
}

fn dedent_block(lines: &[&str]) -> String {
    let indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|line| {
            if line.len() >= indent {
                &line[indent..]
            } else {
                line.trim()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end()
        .to_string()
}

fn render_block(block: &Block) -> String {
    match block {
        Block::Heading { level, text } => {
            format!("#heading(level: {level})[{}]", render_prose(text))
        }
        Block::Code { language, body } => {
            let language = language
                .as_ref()
                .map(|l| format!(", lang: \"{}\"", escape_string(normalize_language(l))))
                .unwrap_or_default();
            format!("#raw(\"{}\"{language}, block: true)", escape_string(body))
        }
        Block::List(items) => render_list(items),
        Block::Paragraph(text) => render_prose(text),
    }
}

/// Typst knows languages by their common name; `py` is the one abbreviation
/// that shows up often enough in docstrings to be worth mapping.
fn normalize_language(language: &str) -> &str {
    match language {
        "py" => "python",
        other => other,
    }
}

fn render_list(items: &[ListItem]) -> String {
    let rendered: Vec<String> = items
        .iter()
        .map(|item| {
            let text = render_prose(&item.text);
            if item.children.is_empty() {
                format!("[{text}]")
            } else {
                format!("[{text}\n\n{}]", render_list(&item.children))
            }
        })
        .collect();
    format!("#list(\n{}\n)", rendered.join(",\n"))
}
