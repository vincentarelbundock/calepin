//! # typst-to-latex
//!
//! Converts Typst math expressions to LaTeX math expressions.
//!
//! This is a pure Rust implementation informed by the Haskell reference code
//! in [texmath](https://github.com/jgm/texmath) and
//! [typst-hs](https://github.com/jgm/typst-hs).
//!
//! # Example
//!
//! ```
//! use typst_to_latex::typst_to_latex;
//!
//! let latex = typst_to_latex("alpha + beta").unwrap();
//! assert_eq!(latex, "\\alpha + \\beta");
//! ```

mod ast;
mod emitter;
mod error;
mod parser;
mod symbols;

pub use error::Error;

/// Convert a Typst math expression to LaTeX.
///
/// The input should be the content between Typst's `$...$` delimiters
/// (without the dollar signs themselves).
///
/// Returns the equivalent LaTeX math expression, or an error if the input
/// cannot be parsed.
pub fn typst_to_latex(input: &str) -> Result<String, Error> {
    let nodes = parser::parse(input)?;
    Ok(emitter::emit(&nodes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_variables() {
        assert_eq!(typst_to_latex("x").unwrap(), "x");
        assert_eq!(typst_to_latex("x y z").unwrap(), "x y z");
    }

    #[test]
    fn test_numbers() {
        assert_eq!(typst_to_latex("42").unwrap(), "42");
        assert_eq!(typst_to_latex("3.14").unwrap(), "3.14");
    }

    #[test]
    fn test_greek_letters() {
        assert_eq!(typst_to_latex("alpha").unwrap(), "\\alpha");
        assert_eq!(typst_to_latex("beta").unwrap(), "\\beta");
        assert_eq!(typst_to_latex("Gamma").unwrap(), "\\Gamma");
    }

    #[test]
    fn test_subscript() {
        assert_eq!(typst_to_latex("x_1").unwrap(), "x_{1}");
        assert_eq!(typst_to_latex("x_n").unwrap(), "x_{n}");
    }

    #[test]
    fn test_superscript() {
        assert_eq!(typst_to_latex("x^2").unwrap(), "x^{2}");
    }

    #[test]
    fn test_subsup() {
        assert_eq!(typst_to_latex("x_1^2").unwrap(), "x_{1}^{2}");
    }

    #[test]
    fn test_fraction_operator() {
        assert_eq!(typst_to_latex("a / b").unwrap(), "\\frac{a}{b}");
    }

    #[test]
    fn test_fraction_with_parens() {
        // In Typst, `/` strips outer parens from the denominator only.
        // The numerator (a + b) keeps its parens in the Frac node since
        // hideOuterParens is only applied to the RHS in the Haskell reference.
        // However, `\frac{}{}` doesn't need parens visually, so both sides
        // get their outer parens stripped during emission.
        assert_eq!(
            typst_to_latex("a / (b + c)").unwrap(),
            "\\frac{a}{b + c}"
        );
    }

    #[test]
    fn test_frac_function() {
        assert_eq!(typst_to_latex("frac(a, b)").unwrap(), "\\frac{a}{b}");
    }

    #[test]
    fn test_sqrt() {
        assert_eq!(typst_to_latex("sqrt(x)").unwrap(), "\\sqrt{x}");
    }

    #[test]
    fn test_root() {
        assert_eq!(typst_to_latex("root(3, x)").unwrap(), "\\sqrt[3]{x}");
    }

    #[test]
    fn test_binom() {
        assert_eq!(typst_to_latex("binom(n, k)").unwrap(), "\\binom{n}{k}");
    }

    #[test]
    fn test_sum_with_limits() {
        assert_eq!(
            typst_to_latex("sum_(i=0)^n").unwrap(),
            "\\sum_{i=0}^{n}"
        );
        // With spaces around =
        assert_eq!(
            typst_to_latex("sum_(i = 0)^n").unwrap(),
            "\\sum_{i = 0}^{n}"
        );
    }

    #[test]
    fn test_integral() {
        assert_eq!(
            typst_to_latex("integral_0^1 f(x) dif x").unwrap(),
            "\\int_{0}^{1} f(x) d x"
        );
    }

    #[test]
    fn test_text_style() {
        assert_eq!(typst_to_latex("bold(x)").unwrap(), "\\mathbf{x}");
        assert_eq!(typst_to_latex("cal(A)").unwrap(), "\\mathcal{A}");
        assert_eq!(typst_to_latex("bb(R)").unwrap(), "\\mathbb{R}");
    }

    #[test]
    fn test_string_literal() {
        assert_eq!(typst_to_latex(r#""hello""#).unwrap(), "\\text{hello}");
    }

    #[test]
    fn test_cancel() {
        assert_eq!(typst_to_latex("cancel(x)").unwrap(), "\\cancel{x}");
    }

    #[test]
    fn test_overline_underline() {
        assert_eq!(typst_to_latex("overline(x)").unwrap(), "\\overline{x}");
        assert_eq!(typst_to_latex("underline(x)").unwrap(), "\\underline{x}");
    }

    #[test]
    fn test_underbrace_with_annotation() {
        assert_eq!(
            typst_to_latex("underbrace(x + y, \"sum\")").unwrap(),
            "\\underbrace{x + y}_{\\text{sum}} "
        );
    }

    #[test]
    fn test_op() {
        assert_eq!(
            typst_to_latex(r#"op("curl")"#).unwrap(),
            "\\operatorname{curl}"
        );
    }

    #[test]
    fn test_predefined_operators() {
        assert_eq!(typst_to_latex("sin").unwrap(), "\\sin");
        assert_eq!(typst_to_latex("lim").unwrap(), "\\lim");
        assert_eq!(typst_to_latex("det").unwrap(), "\\det");
    }

    #[test]
    fn test_shorthand_arrow() {
        assert_eq!(typst_to_latex("A -> B").unwrap(), "A \\rightarrow B");
    }

    #[test]
    fn test_shorthand_neq() {
        assert_eq!(typst_to_latex("a != b").unwrap(), "a \\neq b");
    }

    #[test]
    fn test_dots() {
        assert_eq!(typst_to_latex("1, ..., n").unwrap(), "1, \\dots, n");
    }

    #[test]
    fn test_abs() {
        assert_eq!(typst_to_latex("abs(x)").unwrap(), "\\left| x \\right|");
    }

    #[test]
    fn test_norm() {
        assert_eq!(
            typst_to_latex("norm(x)").unwrap(),
            "\\left\\| x \\right\\|"
        );
    }

    #[test]
    fn test_floor_ceil() {
        assert_eq!(
            typst_to_latex("floor(x)").unwrap(),
            "\\left\\lfloor x \\right\\rfloor"
        );
        assert_eq!(
            typst_to_latex("ceil(x)").unwrap(),
            "\\left\\lceil x \\right\\rceil"
        );
    }

    #[test]
    fn test_spaces() {
        assert_eq!(typst_to_latex("thin").unwrap(), "\\,");
        assert_eq!(typst_to_latex("quad").unwrap(), "\\quad");
    }

    #[test]
    fn test_display_style() {
        assert_eq!(
            typst_to_latex("display(x)").unwrap(),
            "\\displaystyle x"
        );
    }

    #[test]
    fn test_complex_expression() {
        let result = typst_to_latex("e^(i pi) + 1 = 0").unwrap();
        assert_eq!(result, "e^{i \\pi} + 1 = 0");
    }

    #[test]
    fn test_matrix() {
        let result = typst_to_latex("mat(1, 0; 0, 1)").unwrap();
        assert_eq!(
            result,
            "\\begin{pmatrix} 1 & 0 \\\\ 0 & 1 \\end{pmatrix}"
        );
    }

    #[test]
    fn test_vec_func() {
        let result = typst_to_latex("vec(x, y, z)").unwrap();
        assert_eq!(
            result,
            "\\begin{pmatrix} x \\\\ y \\\\ z \\end{pmatrix}"
        );
    }

    #[test]
    fn test_lr() {
        let result = typst_to_latex("lr([x + y])").unwrap();
        assert_eq!(result, "\\left[ x + y \\right]");
    }

    #[test]
    fn test_accent() {
        let result = typst_to_latex("accent(x, hat)").unwrap();
        assert_eq!(result, "\\hat{x} ");
    }

    #[test]
    fn test_cases() {
        let result = typst_to_latex("cases(1, x > 0, -1, x < 0)").unwrap();
        assert_eq!(
            result,
            "\\begin{cases} 1 & x > 0 \\\\ -1 & x < 0 \\end{cases}"
        );
    }
}
