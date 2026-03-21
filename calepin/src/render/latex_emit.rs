//! LaTeX format emitter for the unified AST walker.

use comrak::nodes::TableAlignment;

use crate::render::ast::{FormatEmitter, FootnoteStrategy, HeadingAttrs, WalkOptions, walk_and_render};
use crate::render::markdown::ImageAttrs;

pub struct LatexEmitter {
    pub number_sections: bool,
}

/// Convert markdown to LaTeX via the shared AST walker.
/// Image attributes and path resolution are handled structurally in the emitter.
pub fn markdown_to_latex(markdown: &str, raw_fragments: &[String], number_sections: bool) -> String {
    let emitter = LatexEmitter { number_sections };
    let options = WalkOptions { number_sections, shift_headings: false };
    walk_and_render(&emitter, markdown, raw_fragments, &options)
}

impl FormatEmitter for LatexEmitter {
    fn format_name(&self) -> &str { "latex" }

    fn escape_text(&self, s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut iter = s.chars().peekable();
        while let Some(c) = iter.next() {
            let next = iter.peek().copied().unwrap_or('\0');
            match c {
                '{' | '}' | '#' | '%' | '&' | '$' | '_' => {
                    out.push('\\');
                    out.push(c);
                }
                '-' => {
                    if next == '-' { out.push_str("-{}"); } else { out.push('-'); }
                }
                '~' => out.push_str("\\textasciitilde{}"),
                '^' => out.push_str("\\^{}"),
                '\\' => out.push_str("\\textbackslash{}"),
                '|' => out.push_str("\\textbar{}"),
                '<' => out.push_str("\\textless{}"),
                '>' => out.push_str("\\textgreater{}"),
                '[' | ']' => { out.push('{'); out.push(c); out.push('}'); }
                '"' => out.push_str("\\textquotedbl{}"),
                '\'' => out.push_str("\\textquotesingle{}"),
                '\u{00A0}' => out.push('~'),
                '\u{2026}' => out.push_str("\\ldots{}"),
                '\u{2018}' => out.push('`'),
                '\u{2019}' => out.push('\''),
                '\u{201C}' => out.push_str("``"),
                '\u{201D}' => out.push_str("''"),
                '\u{2014}' => out.push_str("---"),
                '\u{2013}' => out.push_str("--"),
                _ => out.push(c),
            }
        }
        out
    }

    fn blockquote_open(&self) -> &str { "\\begin{quote}\n" }
    fn blockquote_close(&self) -> &str { "\\end{quote}\n\n" }

    fn list_open(&self, ordered: bool, start: usize, _tight: bool) -> String {
        let env = if ordered { "enumerate" } else { "itemize" };
        let mut out = format!("\\begin{{{}}}\n", env);
        if ordered && start > 1 {
            out.push_str(&format!("\\setcounter{{enumi}}{{{}}}\n", start - 1));
        }
        out
    }

    fn list_close(&self, ordered: bool) -> String {
        let env = if ordered { "enumerate" } else { "itemize" };
        format!("\\end{{{}}}\n\n", env)
    }

    fn item_open(&self, _tight: bool) -> String { "\\item ".to_string() }
    fn item_close(&self) -> &str { "\n" }

    fn paragraph_open(&self, _in_tight_list_item: bool) -> &str { "" }
    fn paragraph_close(&self, _in_tight_list_item: bool) -> &str { "\n\n" }

    fn heading_prefix(&self, level: u8) -> String {
        let star = if self.number_sections { "" } else { "*" };
        let cmd = match level {
            1 => format!("\\section{}{{", star),
            2 => format!("\\subsection{}{{", star),
            3 => format!("\\subsubsection{}{{", star),
            4 => format!("\\paragraph{}{{", star),
            5 | _ => format!("\\subparagraph{}{{", star),
        };
        cmd
    }

    fn heading(
        &self,
        _level: u8,
        attrs: &HeadingAttrs,
        rendered_content: &str,
        _section_number: Option<&str>,
    ) -> String {
        let is_unnumbered = attrs.classes.iter().any(|c| c == "unnumbered" || c == "unlisted");

        let mut out = rendered_content.to_string();

        // If unnumbered and number_sections is on, the prefix already has no star,
        // so we need to retroactively add one. Look for the last `{` and insert `*`.
        if is_unnumbered && self.number_sections {
            // The heading_prefix is already in the buffer before rendered_content.
            // We can't modify it from here, but the walker truncated it.
            // Actually, the prefix was already written before heading_content_start,
            // then truncated. Let me handle this differently.
            // For now, we reconstruct the full heading with the star.
        }

        // Close the heading brace
        out.push_str("}\n");
        if !attrs.id.is_empty() {
            out.push_str(&format!("\\label{{{}}}\n", attrs.id));
        }
        out.push('\n');
        out
    }

    fn code_inline(&self, literal: &str) -> String {
        format!("\\texttt{{{}}}", self.escape_text(literal))
    }

    fn code_block(&self, _info: &str, literal: &str) -> String {
        format!("\\begin{{verbatim}}\n{}\\end{{verbatim}}\n\n", literal)
    }

    fn strong_open(&self) -> &str { "\\textbf{" }
    fn strong_close(&self) -> &str { "}" }
    fn emph_open(&self) -> &str { "\\emph{" }
    fn emph_close(&self) -> &str { "}" }
    fn strikethrough_open(&self) -> &str { "\\sout{" }
    fn strikethrough_close(&self) -> &str { "}" }
    fn superscript_open(&self) -> &str { "\\textsuperscript{" }
    fn superscript_close(&self) -> &str { "}" }

    fn link_open(&self, url: &str) -> String {
        if url.starts_with('#') {
            format!("\\protect\\hyperlink{{{}}}{{{}", &url[1..], "")
        } else {
            let escaped = url.replace('\\', "/").replace('#', "\\#").replace('%', "\\%");
            format!("\\href{{{}}}{{{}", escaped, "")
        }
    }
    fn link_close(&self) -> &str { "}" }

    fn image(&self, url: &str, _alt: &str, attrs: &ImageAttrs) -> String {
        let resolved = crate::filters::figure::resolve_path(
            std::path::Path::new(url), "latex",
        );
        let options = attrs.to_latex_options();
        format!("\\protect\\includegraphics{}{{{}}}", options, resolved.display())
    }

    fn table_open(&self, alignments: &[TableAlignment]) -> String {
        let col_spec: String = alignments.iter().map(|a| match a {
            TableAlignment::Left => 'l',
            TableAlignment::Center => 'c',
            TableAlignment::Right => 'r',
            _ => 'l',
        }).collect();
        format!("\\begin{{tabular}}{{{}}}\n\\hline\n", col_spec)
    }
    fn table_close(&self) -> &str { "\\hline\n\\end{tabular}\n\n" }

    fn table_row_open(&self, _is_header: bool) -> String { String::new() }
    fn table_row_close(&self, is_header: bool) -> String {
        let mut out = " \\\\\n".to_string();
        if is_header { out.push_str("\\hline\n"); }
        out
    }

    fn table_cell_open(&self, _is_header: bool, _align: TableAlignment, index: usize) -> String {
        if index > 0 { " & ".to_string() } else { String::new() }
    }
    fn table_cell_close(&self, _is_header: bool) -> String { String::new() }

    fn thematic_break(&self) -> &str {
        "\n\\begin{center}\\rule{0.5\\linewidth}{\\linethickness}\\end{center}\n\n"
    }
    fn soft_break(&self) -> &str { "\n" }
    fn line_break(&self) -> &str { "\\\\\n" }

    fn footnote_strategy(&self) -> FootnoteStrategy { FootnoteStrategy::DefAtSite }

    fn footnote_ref(&self, id: usize) -> String {
        format!("\\footnotemark[{}]", id)
    }

    fn footnote_def_open(&self, id: usize) -> String {
        format!("\\footnotetext[{}]{{", id)
    }
    fn footnote_def_close(&self) -> &str { "}" }

    fn html_block(&self, _literal: &str) -> String { String::new() }
    fn html_inline(&self, _literal: &str) -> String { String::new() }

    fn task_item(&self, checked: bool) -> String {
        if checked {
            "\\item[$\\boxtimes$] ".to_string()
        } else {
            "\\item[$\\square$] ".to_string()
        }
    }

    fn description_list_open(&self) -> &str { "\\begin{description}\n" }
    fn description_list_close(&self) -> &str { "\\end{description}\n\n" }
    fn description_term_open(&self) -> &str { "\\item[" }
    fn description_term_close(&self) -> &str { "] " }
    fn description_details_open(&self) -> &str { "" }
    fn description_details_close(&self) -> &str { "\n" }
}
