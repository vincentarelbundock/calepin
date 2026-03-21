// SPDX-License-Identifier: BSD-2-Clause
//
// Original LaTeX renderer ported from commonmark-gfm (cmark-gfm).
// Now delegates to the unified AST walker via latex_emit.rs.

//! Convert markdown to LaTeX. Delegates to `latex_emit::LatexEmitter`.

/// Convert markdown text to LaTeX by walking comrak's AST.
/// Math expressions and raw span output are protected from LaTeX escaping.
pub fn markdown_to_latex(markdown: &str, raw_fragments: &[String], number_sections: bool) -> String {
    crate::render::latex_emit::markdown_to_latex(markdown, raw_fragments, number_sections)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heading_numbered() {
        let latex = markdown_to_latex("# Hello", &[], true);
        assert!(latex.contains("\\section"), "latex: {}", latex);
        assert!(latex.contains("Hello"), "latex: {}", latex);
    }

    #[test]
    fn test_heading_unnumbered() {
        let latex = markdown_to_latex("# Hello", &[], false);
        assert!(latex.contains("\\section*"), "latex: {}", latex);
    }

    #[test]
    fn test_heading_explicit_id() {
        let latex = markdown_to_latex("# Introduction {#sec-intro}", &[], false);
        assert!(latex.contains("Introduction"), "latex: {}", latex);
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
        assert!(latex.contains("Left"));
        assert!(latex.contains("\\hline"));
        assert!(latex.contains("\\end{tabular}"));
    }
}
