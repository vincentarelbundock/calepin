// SPDX-License-Identifier: BSD-2-Clause
//
// This module is a Rust port of the LaTeX renderer from the commonmark-gfm
// C library (cmark-gfm), originally licensed under the BSD 2-Clause License.
// See: https://github.com/github/cmark-gfm
//
// Copyright (c) 2014, John MacFarlane
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
// ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
// LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
// CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
// SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
// INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
// CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
// ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.

//! Convert a CommonMark AST (via comrak) to LaTeX.

use comrak::nodes::NodeValue;
use comrak::{parse_document, Arena};
use regex::Regex;
use std::sync::LazyLock;

/// Match `\includegraphics{url}\{key=value ...\}` (escaped braces from text).
static LATEX_IMG_ATTR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\\protect\\includegraphics\{([^}]+)\}\s*\\\{([^}]*)\\\}").unwrap()
});

/// Match any `\includegraphics[...]{url}` or `\includegraphics{url}` for path resolution.
static LATEX_IMG_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\\protect\\includegraphics(\[[^\]]*\])?\{([^}]+)\}").unwrap()
});

/// Post-process LaTeX to absorb `\{key=value\}` attribute blocks into preceding `\includegraphics`.
pub fn apply_image_attrs_latex(latex: &str) -> String {
    use crate::render::markdown::ImageAttrs;
    LATEX_IMG_ATTR_RE.replace_all(latex, |caps: &regex::Captures| {
        let url = &caps[1];
        let attrs = ImageAttrs::parse(&caps[2]);
        let options = attrs.to_latex_options();
        let resolved = crate::filters::figure::resolve_path(std::path::Path::new(url), "latex");
        format!("\\protect\\includegraphics{}{{{}}}", options, resolved.display())
    }).to_string()
}

/// Resolve image paths in `\includegraphics` to preferred format (e.g. .svg → .pdf).
pub fn resolve_image_paths_latex(latex: &str) -> String {
    LATEX_IMG_PATH_RE.replace_all(latex, |caps: &regex::Captures| {
        let options = caps.get(1).map_or("", |m| m.as_str());
        let url = &caps[2];
        let resolved = crate::filters::figure::resolve_path(std::path::Path::new(url), "latex");
        format!("\\protect\\includegraphics{}{{{}}}", options, resolved.display())
    }).to_string()
}

/// Match explicit ID attribute in heading: `\{\#some-id\}` (LaTeX-escaped braces)
static RE_EXPLICIT_ID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s*\\\{\\\#([\w-]+)\\\}").unwrap()
});

/// Convert markdown text to LaTeX by walking comrak's AST.
/// Math expressions and raw span output are protected from LaTeX escaping.
pub fn markdown_to_latex(markdown: &str, raw_fragments: &[String], number_sections: bool) -> String {
    use crate::render::markers;

    let preprocessed = markers::preprocess(markdown);
    let (protected, math) = markers::protect_math(&preprocessed);
    let raw = markdown_to_latex_raw(&protected, number_sections);
    let raw = apply_image_attrs_latex(&raw);
    let raw = resolve_image_paths_latex(&raw);
    let restored = markers::restore_math(&raw, &math);
    let restored = markers::resolve_equation_labels(&restored, "latex");
    let restored = markers::resolve_escaped_dollars(&restored, "latex");
    markers::resolve_raw(&restored, raw_fragments)
}

/// Inner LaTeX conversion (no math protection).
fn markdown_to_latex_raw(markdown: &str, number_sections: bool) -> String {
    let arena = Arena::new();
    let options = comrak_options();
    let root = parse_document(&arena, markdown, &options);

    // Pre-pass: assign numeric IDs to footnotes (LaTeX requires numbers, not string labels)
    let mut footnote_ids: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut fn_counter = 0usize;
    for edge in root.traverse() {
        if let comrak::arena_tree::NodeEdge::Start(node) = edge {
            match &node.data.borrow().value {
                NodeValue::FootnoteDefinition(def) => {
                    if !footnote_ids.contains_key(&def.name) {
                        fn_counter += 1;
                        footnote_ids.insert(def.name.clone(), fn_counter);
                    }
                }
                NodeValue::FootnoteReference(r) => {
                    if !footnote_ids.contains_key(&r.name) {
                        fn_counter += 1;
                        footnote_ids.insert(r.name.clone(), fn_counter);
                    }
                }
                _ => {}
            }
        }
    }

    let mut out = String::new();
    // Track position where heading content starts (after \section{)
    let mut heading_content_start: Option<usize> = None;
    // Table state
    let mut table_cell_index: usize = 0;
    let mut table_in_header = false;

    for edge in root.traverse() {
        match edge {
            comrak::arena_tree::NodeEdge::Start(node) => {
                let val = &node.data.borrow().value;
                match val {
                    NodeValue::Heading(_) => {
                        render_entering(val, &mut out, number_sections);
                        heading_content_start = Some(out.len());
                    }
                    NodeValue::TableRow(header) => {
                        table_cell_index = 0;
                        table_in_header = *header;
                    }
                    NodeValue::TableCell => {
                        if table_cell_index > 0 {
                            out.push_str(" & ");
                        }
                        table_cell_index += 1;
                    }
                    NodeValue::FootnoteDefinition(def) => {
                        let id = footnote_ids.get(&def.name).copied().unwrap_or(0);
                        out.push_str(&format!("\\footnotetext[{}]{{", id));
                    }
                    NodeValue::FootnoteReference(r) => {
                        let id = footnote_ids.get(&r.name).copied().unwrap_or(0);
                        out.push_str(&format!("\\footnotemark[{}]", id));
                    }
                    _ => {
                        render_entering(val, &mut out, number_sections);
                    }
                }
            }
            comrak::arena_tree::NodeEdge::End(node) => {
                let val = &node.data.borrow().value;
                match val {
                    NodeValue::Heading(_) => {
                        let (label, clean_end) = if let Some(start) = heading_content_start.take() {
                            let heading_text = &out[start..];
                            // Check for explicit {#id} attribute
                            if let Some(m) = RE_EXPLICIT_ID.captures(heading_text) {
                                let id = m.get(1).unwrap().as_str().to_string();
                                // Find where the {#id} pattern starts in the heading
                                let attr_start = m.get(0).unwrap().start();
                                (id, Some(start + attr_start))
                            } else {
                                let plain = heading_text
                                    .replace("\\emph{", "").replace("\\textbf{", "")
                                    .replace("\\texttt{", "").replace('}', "")
                                    .replace('\\', "");
                                (slugify(&plain), None)
                            }
                        } else {
                            (String::new(), None)
                        };
                        // Strip the {#id} from heading text if present
                        if let Some(end) = clean_end {
                            out.truncate(end);
                            // Trim trailing whitespace inside heading
                            let trimmed = out.trim_end().len();
                            out.truncate(trimmed);
                        }
                        out.push_str("}\n");
                        if !label.is_empty() {
                            out.push_str(&format!("\\label{{{}}}\n", label));
                        }
                        out.push('\n');
                    }
                    NodeValue::TableRow(_) => {
                        out.push_str(" \\\\\n");
                        if table_in_header {
                            out.push_str("\\hline\n");
                        }
                    }
                    NodeValue::TableCell => {}
                    _ => {
                        render_leaving(val, &mut out);
                    }
                }
            }
        }
    }

    out
}

use crate::render::markdown::comrak_options;

fn render_entering(val: &NodeValue, out: &mut String, number_sections: bool) {
    match val {
        NodeValue::Document => {}
        NodeValue::BlockQuote => {
            out.push_str("\\begin{quote}\n");
        }
        NodeValue::List(nl) => {
            let env = match nl.list_type {
                comrak::nodes::ListType::Ordered => "enumerate",
                _ => "itemize",
            };
            out.push_str(&format!("\\begin{{{}}}\n", env));
            if nl.list_type == comrak::nodes::ListType::Ordered && nl.start > 1 {
                out.push_str(&format!("\\setcounter{{enumi}}{{{}}}\n", nl.start - 1));
            }
        }
        NodeValue::Item(_) => {
            out.push_str("\\item ");
        }
        NodeValue::Heading(h) => {
            let star = if number_sections { "" } else { "*" };
            let cmd = match h.level {
                1 => format!("\\section{}{{", star),
                2 => format!("\\subsection{}{{", star),
                3 => format!("\\subsubsection{}{{", star),
                4 => format!("\\paragraph{}{{", star),
                5 => format!("\\subparagraph{}{{", star),
                _ => format!("\\subparagraph{}{{", star),
            };
            out.push_str(&cmd);
        }
        NodeValue::CodeBlock(cb) => {
            out.push_str("\\begin{verbatim}\n");
            out.push_str(&cb.literal);
            out.push_str("\\end{verbatim}\n\n");
        }
        NodeValue::Paragraph => {}
        NodeValue::ThematicBreak => {
            out.push_str(
                "\n\\begin{center}\\rule{0.5\\linewidth}{\\linethickness}\\end{center}\n\n",
            );
        }
        NodeValue::Text(t) => {
            out.push_str(&escape_latex(t));
        }
        NodeValue::SoftBreak => {
            out.push('\n');
        }
        NodeValue::LineBreak => {
            out.push_str("\\\\\n");
        }
        NodeValue::Code(c) => {
            out.push_str("\\texttt{");
            out.push_str(&escape_latex(&c.literal));
            out.push('}');
        }
        NodeValue::Strong => {
            out.push_str("\\textbf{");
        }
        NodeValue::Emph => {
            out.push_str("\\emph{");
        }
        NodeValue::Strikethrough => {
            out.push_str("\\sout{");
        }
        NodeValue::Superscript => {
            out.push_str("\\textsuperscript{");
        }
        NodeValue::Link(link) => {
            if link.url.starts_with('#') {
                // Internal link
                out.push_str(&format!(
                    "\\protect\\hyperlink{{{}}}{{",
                    &link.url[1..]
                ));
            } else {
                out.push_str(&format!("\\href{{{}}}{{{}", escape_url(&link.url), ""));
                // Close brace will be added in render_leaving
                // But we need the opening { for the text
                // Restructure: \href{url}{text}
            }
        }
        NodeValue::Image(link) => {
            out.push_str(&format!(
                "\\protect\\includegraphics{{{}}}",
                escape_url(&link.url)
            ));
        }
        NodeValue::Table(table) => {
            let col_spec: String = table
                .alignments
                .iter()
                .map(|a| match a {
                    comrak::nodes::TableAlignment::Left => 'l',
                    comrak::nodes::TableAlignment::Center => 'c',
                    comrak::nodes::TableAlignment::Right => 'r',
                    _ => 'l',
                })
                .collect();
            out.push_str(&format!("\\begin{{tabular}}{{{}}}\n\\hline\n", col_spec));
        }
        // TableRow and TableCell are handled in the main traversal loop
        NodeValue::TableRow(_) | NodeValue::TableCell => {}
        NodeValue::FootnoteDefinition(_) | NodeValue::FootnoteReference(_) => {
            // Handled in main loop with numeric ID mapping
        }
        NodeValue::HtmlBlock(_) | NodeValue::HtmlInline(_) => {
            // Skip raw HTML in LaTeX output
        }
        NodeValue::TaskItem(ti) => {
            if ti.symbol.is_some() {
                out.push_str("\\item[$\\boxtimes$] ");
            } else {
                out.push_str("\\item[$\\square$] ");
            }
        }
        _ => {}
    }
}

fn render_leaving(val: &NodeValue, out: &mut String) {
    match val {
        NodeValue::BlockQuote => {
            out.push_str("\\end{quote}\n\n");
        }
        NodeValue::List(nl) => {
            let env = match nl.list_type {
                comrak::nodes::ListType::Ordered => "enumerate",
                _ => "itemize",
            };
            out.push_str(&format!("\\end{{{}}}\n\n", env));
        }
        NodeValue::Item(_) => {
            out.push('\n');
        }
        NodeValue::Heading(_) => {
            // Handled in the main loop with \label
        }
        NodeValue::Paragraph => {
            out.push_str("\n\n");
        }
        NodeValue::Strong | NodeValue::Emph | NodeValue::Strikethrough | NodeValue::Superscript => {
            out.push('}');
        }
        NodeValue::Link(_) => {
            out.push('}');
        }
        NodeValue::FootnoteDefinition(_) => {
            out.push('}');
        }
        NodeValue::Table(_) => {
            out.push_str("\\hline\n\\end{tabular}\n\n");
        }
        // Leaf nodes and nodes handled entirely in render_entering
        NodeValue::CodeBlock(_)
        | NodeValue::Code(_)
        | NodeValue::Text(_)
        | NodeValue::Image(_)
        | NodeValue::ThematicBreak
        | NodeValue::SoftBreak
        | NodeValue::LineBreak
        | NodeValue::FootnoteReference(_)
        | NodeValue::HtmlBlock(_)
        | NodeValue::HtmlInline(_)
        | NodeValue::TaskItem(_)
        | NodeValue::Document
        | NodeValue::TableRow(_)
        | NodeValue::TableCell => {}
        _ => {}
    }
}

/// Escape special LaTeX characters in text.
/// Ported from commonmark-gfm latex.c `outc()`.
fn escape_latex(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut iter = s.chars().peekable();

    while let Some(c) = iter.next() {
        let next = iter.peek().copied().unwrap_or('\0');
        match c {
            '{' | '}' | '#' | '%' | '&' => {
                out.push('\\');
                out.push(c);
            }
            '$' | '_' => {
                out.push('\\');
                out.push(c);
            }
            '-' => {
                if next == '-' {
                    out.push_str("-{}");
                } else {
                    out.push('-');
                }
            }
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\^{}"),
            '\\' => out.push_str("\\textbackslash{}"),
            '|' => out.push_str("\\textbar{}"),
            '<' => out.push_str("\\textless{}"),
            '>' => out.push_str("\\textgreater{}"),
            '[' | ']' => {
                out.push('{');
                out.push(c);
                out.push('}');
            }
            '"' => out.push_str("\\textquotedbl{}"),
            '\'' => out.push_str("\\textquotesingle{}"),
            '\u{00A0}' => out.push('~'),           // nbsp
            '\u{2026}' => out.push_str("\\ldots{}"), // hellip
            '\u{2018}' => out.push('`'),             // lsquo
            '\u{2019}' => out.push('\''),            // rsquo
            '\u{201C}' => out.push_str("``"),        // ldquo
            '\u{201D}' => out.push_str("''"),        // rdquo
            '\u{2014}' => out.push_str("---"),       // emdash
            '\u{2013}' => out.push_str("--"),        // endash
            _ => out.push(c),
        }
    }

    out
}

use crate::util::slugify;

/// Escape a URL for LaTeX \href{}.
fn escape_url(url: &str) -> String {
    url.replace('\\', "/")
        .replace('#', "\\#")
        .replace('%', "\\%")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_numbered() {
        let latex = markdown_to_latex("# Hello", &[], true);
        assert!(latex.contains("\\section{Hello}"));
    }

    #[test]
    fn test_heading_unnumbered() {
        let latex = markdown_to_latex("# Hello", &[], false);
        assert!(latex.contains("\\section*{Hello}"));
    }

    #[test]
    fn test_heading_explicit_id() {
        let latex = markdown_to_latex("# Introduction {#sec-intro}", &[], false);
        assert!(latex.contains("\\section*{Introduction}"), "heading text should be clean: {}", latex);
        assert!(latex.contains("\\label{sec-intro}"), "label should use explicit id: {}", latex);
        assert!(!latex.contains("#sec-intro}"), "heading should not contain id attr: {}", latex);
    }

    #[test]
    fn test_emphasis() {
        let latex = markdown_to_latex("*italic* and **bold**", &[], false);
        assert!(latex.contains("\\emph{italic}"));
        assert!(latex.contains("\\textbf{bold}"));
    }

    #[test]
    fn test_escape_specials() {
        let latex = markdown_to_latex("Price is $10 & 20% off", &[], false);
        assert!(latex.contains("\\$"));
        assert!(latex.contains("\\&"));
        assert!(latex.contains("\\%"));
    }

    #[test]
    fn test_code_block() {
        let latex = markdown_to_latex("```\nx <- 1\n```", &[], false);
        assert!(latex.contains("\\begin{verbatim}"));
        assert!(latex.contains("x <- 1"));
        assert!(latex.contains("\\end{verbatim}"));
    }

    #[test]
    fn test_list() {
        let latex = markdown_to_latex("- one\n- two", &[], false);
        assert!(latex.contains("\\begin{itemize}"));
        assert!(latex.contains("\\item one"));
        assert!(latex.contains("\\end{itemize}"));
    }

    #[test]
    fn test_table() {
        let md = "| Left | Center | Right |\n|:-----|:------:|------:|\n| a | b | c |\n| d | e | f |";
        let latex = markdown_to_latex(md, &[], false);
        assert!(latex.contains("\\begin{tabular}{lcr}"));
        assert!(latex.contains("Left & Center & Right"));
        assert!(latex.contains("a & b & c"));
        assert!(latex.contains("\\hline"));
        assert!(latex.contains("\\end{tabular}"));
    }
}
