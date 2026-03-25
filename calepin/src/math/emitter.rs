/// Typst emitter: converts the LaTeX math AST into Typst math strings.

use super::ast::LatexNode;
use super::symbols::{ACCENT_MAP, OPERATOR_MAP, STYLE_MAP, SYMBOL_MAP};

/// Convert a list of LaTeX math nodes to a Typst math string.
pub fn emit(nodes: &[LatexNode]) -> String {
    let mut out = String::new();
    let mut had_space = false;

    for node in nodes {
        if *node == LatexNode::Space {
            had_space = true;
            continue;
        }
        let part = emit_node(node);
        if part.is_empty() {
            continue;
        }
        if !out.is_empty() && !out.ends_with(' ') && !part.starts_with(' ') {
            if had_space || needs_space_between(&out, &part) {
                out.push(' ');
            }
        }
        had_space = false;
        out.push_str(&part);
    }
    out
}

/// Check if a space is needed between two adjacent pieces of output,
/// even when the source didn't have explicit whitespace.
fn needs_space_between(left: &str, right: &str) -> bool {
    if left.is_empty() || right.is_empty() {
        return false;
    }
    let l = left.chars().last().unwrap();
    let r = right.chars().next().unwrap();
    // Space between two alphanumeric tokens to prevent merging
    l.is_alphanumeric() && r.is_alphanumeric()
}

fn emit_node(node: &LatexNode) -> String {
    match node {
        LatexNode::Letter(c) => c.to_string(),
        LatexNode::Number(n) => n.clone(),
        LatexNode::Char(c) => emit_char(*c),
        LatexNode::Command(cmd) => emit_command(cmd),
        LatexNode::TypstIdent(name) => name.clone(),

        LatexNode::Group(children) => {
            if children.len() == 1 {
                emit_node(&children[0])
            } else {
                emit(children)
            }
        }

        LatexNode::Frac(num, den) => {
            let n = emit_group_content(num);
            let d = emit_group_content(den);
            format!("frac({}, {})", n, d)
        }

        LatexNode::Sqrt(body) => {
            let b = emit_group_content(body);
            format!("sqrt({})", b)
        }

        LatexNode::Root(index, body) => {
            let i = emit_group_content(index);
            let b = emit_group_content(body);
            format!("root({}, {})", i, b)
        }

        LatexNode::Binom(upper, lower) => {
            let u = emit_group_content(upper);
            let l = emit_group_content(lower);
            format!("binom({}, {})", u, l)
        }

        LatexNode::Attach { base, sub, sup } => {
            let b = emit_base(base);
            let mut out = b;
            if let Some(s) = sub {
                let sub_str = emit_subscript(s);
                out.push_str(&format!("_{}", sub_str));
            }
            if let Some(s) = sup {
                let sup_str = emit_subscript(s);
                out.push_str(&format!("^{}", sup_str));
            }
            out
        }

        LatexNode::LeftRight(open, close, content) => {
            let inner = emit(content);
            let o = typst_lr_delim(open);
            let c = typst_lr_delim(close);
            if o == "." && c == "." {
                inner
            } else if o == "." {
                format!("lr({})", format!("{} {}", inner, c))
            } else if c == "." {
                format!("lr({})", format!("{} {}", o, inner))
            } else {
                format!("lr({} {} {})", o, inner, c)
            }
        }

        LatexNode::Middle(delim) => {
            format!("mid({})", typst_lr_delim(delim))
        }

        LatexNode::Text(s) => format!("\"{}\"", s),

        LatexNode::OperatorName(name) => {
            format!("op(\"{}\")", name)
        }

        LatexNode::Style(cmd, body) => {
            let func = STYLE_MAP.get(cmd.as_str()).copied().unwrap_or("upright");
            let b = emit_group_content(body);
            format!("{}({})", func, b)
        }

        LatexNode::Accent(cmd, body) => {
            let accent_name = ACCENT_MAP.get(cmd.as_str()).copied().unwrap_or("hat");
            let b = emit_group_content(body);
            format!("accent({}, {})", b, accent_name)
        }

        LatexNode::UnaryFunc(name, body) => {
            let b = emit_group_content(body);
            format!("{}({})", name, b)
        }

        LatexNode::CancelInverted(body) => {
            let b = emit_group_content(body);
            format!("cancel(inverted: true, {})", b)
        }

        LatexNode::CancelCross(body) => {
            let b = emit_group_content(body);
            format!("cancel(cross: true, {})", b)
        }

        LatexNode::OverUnderBrace(kind, body) => {
            let b = emit_group_content(body);
            format!("{}({})", kind, b)
        }

        LatexNode::OverUnderBraceAnnotated(kind, body, annotation) => {
            let b = emit_group_content(body);
            let a = emit_group_content(annotation);
            format!("{}({}, {})", kind, b, a)
        }

        LatexNode::DisplayStyle(func, body) => {
            let b = emit_group_content(body);
            format!("{}({})", func, b)
        }

        LatexNode::Matrix(env, rows) => emit_matrix(env, rows),

        LatexNode::Cases(env, rows) => emit_cases(env, rows),

        LatexNode::Aligned(_env, rows) => emit_aligned(rows),

        LatexNode::Not(inner) => {
            // Try to find negated symbol
            let inner_str = emit_node(inner);
            format!("not {}", inner_str)
        }

        LatexNode::Limits(base) => {
            let b = emit_node(base);
            format!("limits({})", b)
        }

        LatexNode::Scripts(base) => {
            let b = emit_node(base);
            format!("scripts({})", b)
        }

        LatexNode::AlignPoint => "&".into(),
        LatexNode::Linebreak => " \\".into(),
        LatexNode::Space => " ".into(),
        LatexNode::Raw(s) => s.clone(),
    }
}

fn emit_char(c: char) -> String {
    match c {
        // Characters that need no translation in Typst math
        '+' | '-' | '=' | '<' | '>' | '(' | ')' | '[' | ']' | '/' | '!' | ',' | ';'
        | ':' | '.' | '\'' => c.to_string(),
        '|' => "|".into(),
        '~' => "space".into(),
        _ => c.to_string(),
    }
}

fn emit_command(cmd: &str) -> String {
    // Check operator map first (multi-letter operators like \sin, \log)
    if let Some(typst_name) = OPERATOR_MAP.get(cmd) {
        return typst_name.to_string();
    }

    // Check symbol map
    if let Some(typst_name) = SYMBOL_MAP.get(cmd) {
        return typst_name.to_string();
    }

    // Special one-offs
    match cmd {
        "\\dif" | "\\mathrm{d}" => "dif".into(),
        "\\iff" => "<==>".into(),
        "\\implies" => "==>".into(),
        "\\impliedby" => "<==".into(),
        _ => {
            // Strip backslash and pass through as identifier
            if let Some(name) = cmd.strip_prefix('\\') {
                if name.chars().all(|c| c.is_ascii_alphabetic()) {
                    name.to_string()
                } else {
                    cmd.to_string()
                }
            } else {
                cmd.to_string()
            }
        }
    }
}

/// Emit a node that's used as a base for sub/superscripts.
fn emit_base(node: &LatexNode) -> String {
    emit_node(node)
}

/// Emit a sub/superscript argument. Wraps in parens if multi-node.
fn emit_subscript(node: &LatexNode) -> String {
    match node {
        // Single atom: no parens needed
        LatexNode::Letter(_) | LatexNode::Number(_) | LatexNode::Char(_)
        | LatexNode::Command(_) | LatexNode::TypstIdent(_) => emit_node(node),
        // Group with single child: unwrap
        LatexNode::Group(children) if children.len() == 1 => emit_subscript(&children[0]),
        // Group: emit contents, wrap in parens if multi-token
        LatexNode::Group(children) => {
            let inner = emit(children);
            if children.len() > 1 {
                format!("({})", inner)
            } else {
                inner
            }
        }
        // Complex expression: wrap in parens
        _ => {
            let inner = emit_node(node);
            format!("({})", inner)
        }
    }
}

/// Emit the content of a group node (unwrap if Group, otherwise emit as-is).
fn emit_group_content(node: &LatexNode) -> String {
    match node {
        LatexNode::Group(children) => emit(children),
        _ => emit_node(node),
    }
}

/// Convert a delimiter string to Typst syntax for use inside `lr()`.
fn typst_lr_delim(d: &str) -> String {
    match d {
        "(" => "(".into(),
        ")" => ")".into(),
        "[" => "[".into(),
        "]" => "]".into(),
        "{" => "{".into(),
        "}" => "}".into(),
        "|" => "|".into(),
        "||" => "||".into(),
        "." => ".".into(),
        // Already a Typst ident (e.g., "angle.l")
        _ if d.contains('.') || d.len() > 2 => d.to_string(),
        _ => d.to_string(),
    }
}

/// Emit a matrix environment.
fn emit_matrix(env: &str, rows: &[Vec<Vec<LatexNode>>]) -> String {
    let delim = match env {
        "pmatrix" => "",
        "bmatrix" => "delim: \"[\"",
        "Bmatrix" => "delim: \"{\"",
        "vmatrix" => "delim: \"|\"",
        "Vmatrix" => "delim: \"||\"",
        "matrix" | "smallmatrix" => "delim: \"(\"",
        _ => "",
    };

    let row_strs: Vec<String> = rows.iter().map(|row| {
        row.iter()
            .map(|cell| emit(cell))
            .collect::<Vec<_>>()
            .join(", ")
    }).collect();

    if delim.is_empty() {
        format!("mat({})", row_strs.join("; "))
    } else {
        format!("mat({}, {})", delim, row_strs.join("; "))
    }
}

/// Emit a cases environment.
fn emit_cases(env: &str, rows: &[Vec<Vec<LatexNode>>]) -> String {
    let delim = if env == "rcases" {
        "delim: \"}\""
    } else {
        ""
    };

    let row_strs: Vec<String> = rows.iter().map(|row| {
        row.iter()
            .map(|cell| emit(cell))
            .collect::<Vec<_>>()
            .join(", ")
    }).collect();

    if delim.is_empty() {
        format!("cases({})", row_strs.join(", "))
    } else {
        format!("cases({}, {})", delim, row_strs.join(", "))
    }
}

/// Emit an aligned environment.
fn emit_aligned(rows: &[Vec<Vec<LatexNode>>]) -> String {
    // In Typst, aligned equations use `&` alignment and `\` linebreaks directly
    let row_strs: Vec<String> = rows.iter().map(|row| {
        row.iter()
            .map(|cell| emit(cell))
            .collect::<Vec<_>>()
            .join(" & ")
    }).collect();
    row_strs.join(" \\\n")
}
