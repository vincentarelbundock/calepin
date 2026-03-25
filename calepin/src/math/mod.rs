//! LaTeX math to Typst math converter.
//!
//! Parses LaTeX math expressions and emits equivalent Typst math syntax.
//!
//! # Example
//!
//! ```ignore
//! use crate::math::latex_to_typst;
//!
//! let typst = latex_to_typst("\\frac{a}{b}");
//! assert_eq!(typst, "frac(a, b)");
//! ```

mod ast;
mod emitter;
mod parser;
mod symbols;

/// Convert a LaTeX math expression to Typst math.
///
/// The input should be the content between `$...$` delimiters
/// (without the dollar signs themselves).
pub fn latex_to_typst(input: &str) -> String {
    let nodes = parser::parse(input);
    emitter::emit(&nodes)
}

/// Convert a full math expression (with dollar delimiters) from LaTeX to Typst.
///
/// - `$content$` (inline) becomes `$typst_content$`
/// - `$$content$$` (display) becomes `$ typst_content $`
pub fn convert_math_expression(expr: &str) -> String {
    if expr.starts_with("$$") && expr.ends_with("$$") && expr.len() > 4 {
        let inner = &expr[2..expr.len() - 2];
        let converted = latex_to_typst(inner.trim());
        format!("$ {} $", converted)
    } else if expr.starts_with('$') && expr.ends_with('$') && expr.len() > 2 {
        let inner = &expr[1..expr.len() - 1];
        let converted = latex_to_typst(inner);
        format!("${}$", converted)
    } else {
        expr.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Basics ---

    #[test]
    fn test_variables() {
        assert_eq!(latex_to_typst("x"), "x");
        assert_eq!(latex_to_typst("x y z"), "x y z");
    }

    #[test]
    fn test_numbers() {
        assert_eq!(latex_to_typst("42"), "42");
        assert_eq!(latex_to_typst("3.14"), "3.14");
    }

    // --- Greek letters ---

    #[test]
    fn test_greek() {
        assert_eq!(latex_to_typst("\\alpha"), "alpha");
        assert_eq!(latex_to_typst("\\beta"), "beta");
        assert_eq!(latex_to_typst("\\Gamma"), "Gamma");
    }

    #[test]
    fn test_greek_in_expression() {
        assert_eq!(latex_to_typst("\\alpha + \\beta"), "alpha + beta");
    }

    // --- Sub/superscripts ---

    #[test]
    fn test_subscript() {
        assert_eq!(latex_to_typst("x_1"), "x_1");
        assert_eq!(latex_to_typst("x_{n}"), "x_n");
        assert_eq!(latex_to_typst("x_{n+1}"), "x_(n+1)");
        assert_eq!(latex_to_typst("x_{n + 1}"), "x_(n + 1)");
    }

    #[test]
    fn test_superscript() {
        assert_eq!(latex_to_typst("x^2"), "x^2");
        assert_eq!(latex_to_typst("x^{2}"), "x^2");
        assert_eq!(latex_to_typst("e^{i\\pi}"), "e^(i pi)");
    }

    #[test]
    fn test_subsup() {
        assert_eq!(latex_to_typst("x_1^2"), "x_1^2");
        assert_eq!(latex_to_typst("x_{i}^{n}"), "x_i^n");
    }

    // --- Fractions ---

    #[test]
    fn test_frac() {
        assert_eq!(latex_to_typst("\\frac{a}{b}"), "frac(a, b)");
        assert_eq!(latex_to_typst("\\frac{x+1}{y-1}"), "frac(x+1, y-1)");
        assert_eq!(latex_to_typst("\\frac{x + 1}{y - 1}"), "frac(x + 1, y - 1)");
    }

    // --- Roots ---

    #[test]
    fn test_sqrt() {
        assert_eq!(latex_to_typst("\\sqrt{x}"), "sqrt(x)");
    }

    #[test]
    fn test_root() {
        assert_eq!(latex_to_typst("\\sqrt[3]{x}"), "root(3, x)");
    }

    // --- Binomial ---

    #[test]
    fn test_binom() {
        assert_eq!(latex_to_typst("\\binom{n}{k}"), "binom(n, k)");
    }

    // --- Sums/integrals with limits ---

    #[test]
    fn test_sum_with_limits() {
        assert_eq!(
            latex_to_typst("\\sum_{i=0}^{n}"),
            "sum_(i=0)^n"
        );
        assert_eq!(
            latex_to_typst("\\sum_{i = 0}^{n}"),
            "sum_(i = 0)^n"
        );
    }

    #[test]
    fn test_integral() {
        assert_eq!(
            latex_to_typst("\\int_0^1 f(x) \\, dx"),
            "integral_0^1 f(x) thin d x"
        );
    }

    // --- Text styles ---

    #[test]
    fn test_mathbf() {
        assert_eq!(latex_to_typst("\\mathbf{x}"), "bold(x)");
    }

    #[test]
    fn test_mathcal() {
        assert_eq!(latex_to_typst("\\mathcal{A}"), "cal(A)");
    }

    #[test]
    fn test_mathbb() {
        assert_eq!(latex_to_typst("\\mathbb{R}"), "bb(R)");
    }

    // --- Text ---

    #[test]
    fn test_text() {
        assert_eq!(latex_to_typst("\\text{hello}"), "\"hello\"");
    }

    // --- Operators ---

    #[test]
    fn test_operators() {
        assert_eq!(latex_to_typst("\\sin"), "sin");
        assert_eq!(latex_to_typst("\\lim"), "lim");
    }

    #[test]
    fn test_operatorname() {
        assert_eq!(latex_to_typst("\\operatorname{curl}"), "op(\"curl\")");
    }

    // --- Delimiters ---

    #[test]
    fn test_left_right_parens() {
        assert_eq!(
            latex_to_typst("\\left( x + y \\right)"),
            "lr(( x + y ))"
        );
    }

    #[test]
    fn test_left_right_brackets() {
        assert_eq!(
            latex_to_typst("\\left[ a \\right]"),
            "lr([ a ])"
        );
    }

    #[test]
    fn test_left_right_braces() {
        assert_eq!(
            latex_to_typst("\\left\\{ x \\right\\}"),
            "lr({ x })"
        );
    }

    #[test]
    fn test_left_dot_right() {
        // \left. x \right| — invisible left delimiter
        assert_eq!(
            latex_to_typst("\\left. x \\right|"),
            "lr(x |)"
        );
    }

    // --- Accents ---

    #[test]
    fn test_hat() {
        assert_eq!(latex_to_typst("\\hat{x}"), "accent(x, hat)");
    }

    #[test]
    fn test_vec() {
        assert_eq!(latex_to_typst("\\vec{x}"), "accent(x, arrow)");
    }

    // --- Over/underline ---

    #[test]
    fn test_overline() {
        assert_eq!(latex_to_typst("\\overline{x}"), "overline(x)");
    }

    #[test]
    fn test_underline() {
        assert_eq!(latex_to_typst("\\underline{x}"), "underline(x)");
    }

    // --- Cancel ---

    #[test]
    fn test_cancel() {
        assert_eq!(latex_to_typst("\\cancel{x}"), "cancel(x)");
    }

    // --- Underbrace with annotation ---

    #[test]
    fn test_underbrace() {
        assert_eq!(
            latex_to_typst("\\underbrace{x + y}_{\\text{sum}}"),
            "underbrace(x + y, \"sum\")"
        );
    }

    // --- Matrices ---

    #[test]
    fn test_pmatrix() {
        assert_eq!(
            latex_to_typst("\\begin{pmatrix} 1 & 0 \\\\ 0 & 1 \\end{pmatrix}"),
            "mat(1, 0; 0, 1)"
        );
    }

    #[test]
    fn test_bmatrix() {
        assert_eq!(
            latex_to_typst("\\begin{bmatrix} a & b \\\\ c & d \\end{bmatrix}"),
            "mat(delim: \"[\", a, b; c, d)"
        );
    }

    // --- Cases ---

    #[test]
    fn test_cases() {
        assert_eq!(
            latex_to_typst("\\begin{cases} 1 & x > 0 \\\\ -1 & x < 0 \\end{cases}"),
            "cases(1, x > 0, -1, x < 0)"
        );
    }

    // --- Dots ---

    #[test]
    fn test_dots() {
        assert_eq!(latex_to_typst("1, \\dots, n"), "1, dots, n");
        assert_eq!(latex_to_typst("1, \\ldots, n"), "1, dots, n");
    }

    // --- Arrows ---

    #[test]
    fn test_arrows() {
        assert_eq!(latex_to_typst("A \\rightarrow B"), "A arrow.r B");
        assert_eq!(latex_to_typst("A \\to B"), "A arrow.r B");
        assert_eq!(latex_to_typst("A \\Rightarrow B"), "A arrow.r.double B");
    }

    // --- Relations ---

    #[test]
    fn test_neq() {
        assert_eq!(latex_to_typst("a \\neq b"), "a eq.not b");
    }

    #[test]
    fn test_leq() {
        assert_eq!(latex_to_typst("a \\leq b"), "a lt.eq b");
    }

    // --- Spacing ---

    #[test]
    fn test_spacing() {
        assert_eq!(latex_to_typst("\\,"), "thin");
        assert_eq!(latex_to_typst("\\quad"), "quad");
    }

    // --- Display style ---

    #[test]
    fn test_displaystyle() {
        assert_eq!(latex_to_typst("\\displaystyle x"), "display(x)");
    }

    // --- Complex expressions ---

    #[test]
    fn test_euler() {
        assert_eq!(latex_to_typst("e^{i\\pi} + 1 = 0"), "e^(i pi) + 1 = 0");
    }

    // --- Full expression conversion ---

    #[test]
    fn test_convert_inline() {
        assert_eq!(convert_math_expression("$x^2$"), "$x^2$");
    }

    #[test]
    fn test_convert_display() {
        assert_eq!(
            convert_math_expression("$$\\frac{a}{b}$$"),
            "$ frac(a, b) $"
        );
    }

    #[test]
    fn test_convert_inline_complex() {
        assert_eq!(
            convert_math_expression("$\\alpha + \\beta$"),
            "$alpha + beta$"
        );
    }
}
