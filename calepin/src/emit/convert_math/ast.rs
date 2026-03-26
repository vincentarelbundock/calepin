/// AST for LaTeX math expressions, designed for Typst emission.

#[derive(Debug, Clone, PartialEq)]
pub enum LatexNode {
    /// Single-letter variable: `x`, `y`.
    Letter(char),

    /// Number: `42`, `3.14`.
    Number(String),

    /// Single non-letter character: `+`, `-`, `=`, `<`, `>`, `(`, `)`, etc.
    Char(char),

    /// A LaTeX command to be resolved via symbol/operator maps: `\alpha`, `\sum`.
    Command(String),

    /// A Typst identifier (already resolved, e.g., spacing like `thin`).
    TypstIdent(String),

    /// Braced group `{...}` (invisible grouping).
    Group(Vec<LatexNode>),

    /// Fraction: `\frac{num}{den}`.
    Frac(Box<LatexNode>, Box<LatexNode>),

    /// Square root: `\sqrt{body}`.
    Sqrt(Box<LatexNode>),

    /// Nth root: `\sqrt[n]{body}`.
    Root(Box<LatexNode>, Box<LatexNode>),

    /// Binomial: `\binom{n}{k}`.
    Binom(Box<LatexNode>, Box<LatexNode>),

    /// Sub/superscript: base with optional sub and sup.
    Attach {
        base: Box<LatexNode>,
        sub: Option<Box<LatexNode>>,
        sup: Option<Box<LatexNode>>,
    },

    /// `\left<open> ... \right<close>`.
    LeftRight(String, String, Vec<LatexNode>),

    /// `\middle<delim>`.
    Middle(String),

    /// String literal (from `\text{...}`).
    Text(String),

    /// Custom operator name: `\operatorname{name}`.
    OperatorName(String),

    /// Style command: `\mathbf{x}`, `\mathcal{A}`, etc.
    Style(String, Box<LatexNode>),

    /// Accent: `\hat{x}`, `\vec{x}`, etc.
    Accent(String, Box<LatexNode>),

    /// Unary function: `overline(x)`, `underline(x)`, `cancel(x)`.
    UnaryFunc(String, Box<LatexNode>),

    /// `\bcancel{x}` (inverted cancel).
    CancelInverted(Box<LatexNode>),

    /// `\xcancel{x}` (cross cancel).
    CancelCross(Box<LatexNode>),

    /// Over/underbrace without annotation: `\underbrace{x+y}`.
    OverUnderBrace(String, Box<LatexNode>),

    /// Over/underbrace with annotation: `\underbrace{x+y}_{text}`.
    OverUnderBraceAnnotated(String, Box<LatexNode>, Box<LatexNode>),

    /// Display style: `\displaystyle x`, etc.
    DisplayStyle(String, Box<LatexNode>),

    /// Matrix: env name + rows of cells.
    Matrix(String, Vec<Vec<Vec<LatexNode>>>),

    /// Cases: env name + rows of cells.
    Cases(String, Vec<Vec<Vec<LatexNode>>>),

    /// Aligned environment: rows of cells.
    Aligned(String, Vec<Vec<Vec<LatexNode>>>),

    /// `\not` negation prefix.
    Not(Box<LatexNode>),

    /// `\limits` modifier.
    Limits(Box<LatexNode>),

    /// `\nolimits` modifier.
    Scripts(Box<LatexNode>),

    /// Alignment point `&`.
    AlignPoint,

    /// Linebreak `\\`.
    Linebreak,

    /// Whitespace.
    Space,

    /// Raw passthrough (for unsupported constructs).
    Raw(String),
}
