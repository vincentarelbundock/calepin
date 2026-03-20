/// AST for Typst math expressions.
///
/// This is a simplified representation that captures only the math-relevant
/// constructs from Typst's syntax, informed by the `Markup` type in
/// `typst-hs/src/Typst/Syntax.hs`.

/// A single node in a Typst math expression tree.
#[derive(Debug, Clone, PartialEq)]
pub enum MathNode {
    /// A text/number token (single letter variable, number, or literal text).
    Text(String),

    /// A symbol identifier like `alpha`, `sum`, or dotted like `arrow.r`.
    Ident(String),

    /// Fraction: numerator / denominator.
    Frac(Box<MathNode>, Box<MathNode>),

    /// Subscript/superscript attachment: base, optional bottom, optional top.
    Attach {
        base: Box<MathNode>,
        bottom: Option<Box<MathNode>>,
        top: Option<Box<MathNode>>,
    },

    /// Grouped expressions with optional delimiters.
    Group {
        open: Option<String>,
        close: Option<String>,
        children: Vec<MathNode>,
    },

    /// Function call: `name(args...)`.
    FuncCall {
        name: String,
        args: Vec<MathArg>,
    },

    /// Alignment point `&`.
    AlignPoint,

    /// Line break `\\`.
    Linebreak,

    /// A space between tokens.
    Space,

    /// A string literal `"text"`.
    StringLit(String),
}

/// A function argument in a Typst math function call.
#[derive(Debug, Clone, PartialEq)]
pub enum MathArg {
    /// Positional argument (math content).
    Positional(Vec<MathNode>),
    /// Named argument: `key: value`.
    Named(String, Vec<MathNode>),
    /// Array argument (rows separated by `;`, cells by `,`).
    Array(Vec<Vec<Vec<MathNode>>>),
}
