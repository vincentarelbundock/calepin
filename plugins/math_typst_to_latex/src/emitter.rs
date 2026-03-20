/// LaTeX emitter: converts the Typst math AST into LaTeX strings.
///
/// Informed by the TeX writer in `src/Text/TeXMath/Writers/TeX.hs` and
/// the Typst writer in `src/Text/TeXMath/Writers/Typst.hs` (for reverse mappings).

use crate::ast::{MathArg, MathNode};
use crate::symbols::{ACCENT_MAP, PREDEFINED_OPERATORS, SYMBOL_MAP};

/// Convert a list of math nodes to a LaTeX string.
///
/// Space nodes in the list produce explicit spaces. Adjacent non-space nodes
/// are concatenated without extra spacing — the parser is responsible for
/// inserting Space nodes where whitespace appeared in the source.
pub fn emit(nodes: &[MathNode]) -> String {
    let mut out = String::new();
    for node in nodes {
        if *node == MathNode::Space {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
            continue;
        }
        let part = emit_node(node);
        out.push_str(&part);
    }
    out
}


fn emit_node(node: &MathNode) -> String {
    match node {
        MathNode::Text(s) => emit_text(s),

        MathNode::Ident(name) => emit_ident(name),

        MathNode::Frac(num, den) => {
            format!("\\frac{{{}}}{{{}}}", emit_nodes_inline(num), emit_nodes_inline(den))
        }

        MathNode::Attach { base, bottom, top } => {
            let b = emit_node(base);
            let base_str = if needs_braces_as_base(base) {
                format!("{{{}}}", b)
            } else {
                b
            };
            let mut out = base_str;
            if let Some(sub) = bottom {
                out.push_str(&format!("_{{{}}}", emit_nodes_inline(sub)));
            }
            if let Some(sup) = top {
                out.push_str(&format!("^{{{}}}", emit_nodes_inline(sup)));
            }
            out
        }

        MathNode::Group {
            open,
            close,
            children,
        } => emit_group(open.as_deref(), close.as_deref(), children),

        MathNode::FuncCall { name, args } => emit_func_call(name, args),

        MathNode::AlignPoint => "&".into(),

        MathNode::Linebreak => " \\\\".into(),

        MathNode::Space => " ".into(),

        MathNode::StringLit(s) => format!("\\text{{{}}}", s),
    }
}

fn emit_text(s: &str) -> String {
    // Single characters are bare; multi-char text needs \text or \mathrm
    if s.len() == 1 || s.chars().count() == 1 {
        let c = s.chars().next().unwrap();
        match c {
            '{' => "\\{".into(),
            '}' => "\\}".into(),
            '\\' => "\\backslash".into(),
            '#' => "\\#".into(),
            '%' => "\\%".into(),
            '&' => "\\&".into(),
            '~' => "\\sim".into(),
            _ => s.to_string(),
        }
    } else if s.chars().all(|c| c.is_ascii_digit() || c == '.') {
        // Numbers pass through
        s.to_string()
    } else {
        // Multi-letter text
        format!("\\mathrm{{{}}}", s)
    }
}

fn emit_ident(name: &str) -> String {
    // Check predefined operators first
    if let Some((latex_cmd, _limits)) = PREDEFINED_OPERATORS.get(name) {
        return latex_cmd.to_string();
    }

    // Check symbol map
    if let Some(latex) = SYMBOL_MAP.get(name) {
        return latex.to_string();
    }

    // Special identifiers
    match name {
        "dif" => "d".into(),
        "Dif" => "D".into(),
        "thin" => "\\,".into(),
        "med" => "\\:".into(),
        "thick" => "\\;".into(),
        "quad" => "\\quad".into(),
        "wide" => "\\qquad".into(),
        _ => {
            // Unknown multi-letter identifier: use \mathrm
            if name.len() > 1 && !name.contains('.') {
                format!("\\mathrm{{{}}}", name)
            } else {
                // Try splitting on dots and looking up
                let parts: Vec<&str> = name.split('.').collect();
                if parts.len() > 1 {
                    if let Some(latex) = SYMBOL_MAP.get(name) {
                        return latex.to_string();
                    }
                }
                format!("\\mathrm{{{}}}", name.replace('.', "\\."))
            }
        }
    }
}

fn emit_nodes_inline(node: &MathNode) -> String {
    match node {
        MathNode::Group {
            open: None,
            close: None,
            children,
        } => emit(children),
        _ => emit_node(node),
    }
}

fn needs_braces_as_base(node: &MathNode) -> bool {
    matches!(
        node,
        MathNode::Frac(_, _) | MathNode::Attach { .. } | MathNode::FuncCall { .. }
    )
}

fn emit_group(open: Option<&str>, close: Option<&str>, children: &[MathNode]) -> String {
    let inner = emit(children);
    match (open, close) {
        (Some(o), Some(c)) => {
            let lo = latex_delim(o);
            let lc = latex_delim(c);
            format!("{}{}{}", lo, inner, lc)
        }
        (None, None) => inner,
        (Some(o), None) => format!("{}{}", latex_delim(o), inner),
        (None, Some(c)) => format!("{}{}", inner, latex_delim(c)),
    }
}

fn latex_delim(d: &str) -> &str {
    match d {
        "(" => "(",
        ")" => ")",
        "[" => "[",
        "]" => "]",
        "{" => "\\{",
        "}" => "\\}",
        "|" => "|",
        "||" => "\\|",
        _ => d,
    }
}

// ---------------------------------------------------------------------------
// Function call emission
// ---------------------------------------------------------------------------

fn emit_func_call(name: &str, args: &[MathArg]) -> String {
    match name {
        "frac" => emit_frac_call(args),
        "sqrt" => emit_sqrt_call(args),
        "root" => emit_root_call(args),
        "binom" => emit_binom_call(args),
        "vec" => emit_vec_call(args),
        "mat" => emit_mat_call(args),
        "cases" => emit_cases_call(args),
        "abs" => emit_delimited_call("\\left|", "\\right|", args),
        "norm" => emit_delimited_call("\\left\\|", "\\right\\|", args),
        "floor" => emit_delimited_call("\\left\\lfloor", "\\right\\rfloor", args),
        "ceil" => emit_delimited_call("\\left\\lceil", "\\right\\rceil", args),
        "round" => emit_delimited_call("\\left(", "\\right)", args),
        "lr" => emit_lr_call(args),
        "mid" => emit_mid_call(args),
        "accent" => emit_accent_call(args),
        "bold" => emit_style_call("\\mathbf", args),
        "italic" => emit_style_call("\\mathit", args),
        "upright" => emit_style_call("\\mathrm", args),
        "sans" => emit_style_call("\\mathsf", args),
        "mono" => emit_style_call("\\mathtt", args),
        "bb" => emit_style_call("\\mathbb", args),
        "cal" => emit_style_call("\\mathcal", args),
        "frak" => emit_style_call("\\mathfrak", args),
        "serif" => emit_style_call("\\mathrm", args),
        "cancel" => emit_cancel_call(args),
        "display" => emit_display_style("\\displaystyle", args),
        "inline" => emit_display_style("\\textstyle", args),
        "script" => emit_display_style("\\scriptstyle", args),
        "sscript" => emit_display_style("\\scriptscriptstyle", args),
        "overline" => emit_unary_cmd("\\overline", args),
        "underline" => emit_unary_cmd("\\underline", args),
        "overbrace" => emit_brace_annotation("\\overbrace", "^", args),
        "underbrace" => emit_brace_annotation("\\underbrace", "_", args),
        "overbracket" => emit_brace_annotation("\\overbracket", "^", args),
        "underbracket" => emit_brace_annotation("\\underbracket", "_", args),
        "overparen" => emit_unary_cmd("\\overparen", args),
        "underparen" => emit_unary_cmd("\\underparen", args),
        "op" => emit_op_call(args),
        "limits" => emit_limits_call(args),
        "scripts" => emit_scripts_call(args),
        "class" => emit_class_call(args),
        "stretch" => emit_first_positional(args),
        _ => {
            // Unknown function: emit as \mathrm{name}(args)
            let inner = emit_positional_args(args);
            format!("\\mathrm{{{}}}({})", name, inner)
        }
    }
}

fn get_positional(args: &[MathArg], index: usize) -> Option<&Vec<MathNode>> {
    let mut count = 0;
    for arg in args {
        if let MathArg::Positional(nodes) = arg {
            if count == index {
                return Some(nodes);
            }
            count += 1;
        }
    }
    None
}

fn get_named<'a>(args: &'a [MathArg], key: &str) -> Option<&'a Vec<MathNode>> {
    for arg in args {
        if let MathArg::Named(k, v) = arg {
            if k == key {
                return Some(v);
            }
        }
    }
    None
}

fn emit_positional_args(args: &[MathArg]) -> String {
    args.iter()
        .filter_map(|a| {
            if let MathArg::Positional(nodes) = a {
                Some(emit(nodes))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn emit_frac_call(args: &[MathArg]) -> String {
    let num = get_positional(args, 0).map(|n| emit(n)).unwrap_or_default();
    let den = get_positional(args, 1).map(|n| emit(n)).unwrap_or_default();
    format!("\\frac{{{}}}{{{}}}", num, den)
}

fn emit_sqrt_call(args: &[MathArg]) -> String {
    let body = get_positional(args, 0).map(|n| emit(n)).unwrap_or_default();
    format!("\\sqrt{{{}}}", body)
}

fn emit_root_call(args: &[MathArg]) -> String {
    let index = get_positional(args, 0).map(|n| emit(n)).unwrap_or_default();
    let radicand = get_positional(args, 1).map(|n| emit(n)).unwrap_or_default();
    format!("\\sqrt[{}]{{{}}}", index, radicand)
}

fn emit_binom_call(args: &[MathArg]) -> String {
    let upper = get_positional(args, 0).map(|n| emit(n)).unwrap_or_default();
    let lower = get_positional(args, 1).map(|n| emit(n)).unwrap_or_default();
    format!("\\binom{{{}}}{{{}}}", upper, lower)
}

fn emit_vec_call(args: &[MathArg]) -> String {
    // Check for delim named arg
    let delim = get_named(args, "delim")
        .map(|n| emit(n))
        .unwrap_or_else(|| "(".into());

    let (env, _open, _close) = delim_to_matrix_env(&delim);

    let entries: Vec<String> = args
        .iter()
        .filter_map(|a| match a {
            MathArg::Positional(nodes) => Some(emit(nodes)),
            _ => None,
        })
        .collect();

    format!(
        "\\begin{{{}}} {} \\end{{{}}}",
        env,
        entries.join(" \\\\ "),
        env
    )
}

fn emit_mat_call(args: &[MathArg]) -> String {
    let delim = get_named(args, "delim")
        .map(|n| emit(n))
        .unwrap_or_else(|| "(".into());

    let (env, _open, _close) = delim_to_matrix_env(&delim);

    // Look for array args first
    for arg in args {
        if let MathArg::Array(rows) = arg {
            let row_strs: Vec<String> = rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|cell| emit(cell))
                        .collect::<Vec<_>>()
                        .join(" & ")
                })
                .collect();
            return format!(
                "\\begin{{{}}} {} \\end{{{}}}",
                env,
                row_strs.join(" \\\\ "),
                env
            );
        }
    }

    // Fallback: positional args as single row
    let entries = emit_positional_args(args);
    format!("\\begin{{{}}} {} \\end{{{}}}", env, entries, env)
}

fn emit_cases_call(args: &[MathArg]) -> String {
    // Cases: pairs of (value, condition) become rows
    let entries: Vec<String> = args
        .iter()
        .filter_map(|a| match a {
            MathArg::Positional(nodes) => Some(emit(nodes)),
            MathArg::Array(rows) => {
                let row_strs: Vec<String> = rows
                    .iter()
                    .map(|row| {
                        row.iter()
                            .map(|cell| emit(cell))
                            .collect::<Vec<_>>()
                            .join(" & ")
                    })
                    .collect();
                Some(row_strs.join(" \\\\ "))
            }
            _ => None,
        })
        .collect();

    // Check for delim named arg
    let delim = get_named(args, "delim");
    let use_rcases = delim
        .map(|d| {
            let s = emit(d);
            s.contains('}') || s.contains(")")
        })
        .unwrap_or(false);

    let env = if use_rcases { "rcases" } else { "cases" };

    // Pair entries: every two positional args form a case line
    if entries.len() >= 2 && !args.iter().any(|a| matches!(a, MathArg::Array(_))) {
        let mut lines = Vec::new();
        let mut i = 0;
        while i + 1 < entries.len() {
            lines.push(format!("{} & {}", entries[i], entries[i + 1]));
            i += 2;
        }
        if i < entries.len() {
            lines.push(entries[i].clone());
        }
        format!(
            "\\begin{{{}}} {} \\end{{{}}}",
            env,
            lines.join(" \\\\ "),
            env
        )
    } else {
        format!(
            "\\begin{{{}}} {} \\end{{{}}}",
            env,
            entries.join(" \\\\ "),
            env
        )
    }
}

fn emit_delimited_call(open: &str, close: &str, args: &[MathArg]) -> String {
    let body = get_positional(args, 0).map(|n| emit(n)).unwrap_or_default();
    format!("{} {} {}", open, body, close)
}

fn emit_lr_call(args: &[MathArg]) -> String {
    // lr() wraps content with \left...\right scaling
    let mut all_content = Vec::new();
    for arg in args {
        if let MathArg::Positional(nodes) = arg {
            all_content.extend(nodes.iter().cloned());
        }
    }

    // The first and last elements should be delimiters
    if all_content.is_empty() {
        return String::new();
    }

    let inner = emit(&all_content);

    // Try to extract delimiters from the content
    // In typical usage: lr((content)) or lr([content]) etc.
    // The children of the group carry the delimiters
    if all_content.len() == 1 {
        if let MathNode::Group {
            open: Some(o),
            close: Some(c),
            children,
        } = &all_content[0]
        {
            let lo = lr_delim(o);
            let lc = lr_delim(c);
            return format!("\\left{} {} \\right{}", lo, emit(children), lc);
        }
    }

    // Fallback: wrap with \left. \right.
    format!("\\left. {} \\right.", inner)
}

fn emit_mid_call(args: &[MathArg]) -> String {
    let body = get_positional(args, 0).map(|n| emit(n)).unwrap_or("|".into());
    format!("\\middle{}", body)
}

fn lr_delim(d: &str) -> String {
    match d {
        "(" => "(".into(),
        ")" => ")".into(),
        "[" => "[".into(),
        "]" => "]".into(),
        "{" => "\\{".into(),
        "}" => "\\}".into(),
        "|" => "|".into(),
        "||" => "\\|".into(),
        _ => d.to_string(),
    }
}

fn emit_accent_call(args: &[MathArg]) -> String {
    let base = get_positional(args, 0).map(|n| emit(n)).unwrap_or_default();
    let accent_name = get_positional(args, 1)
        .map(|n| {
            // The accent arg is typically an ident node
            if let Some(MathNode::Ident(name)) = n.first() {
                name.clone()
            } else {
                emit(n)
            }
        })
        .unwrap_or_default();

    if let Some(cmd) = ACCENT_MAP.get(accent_name.as_str()) {
        format!("{}{{{}}} ", cmd, base)
    } else {
        // Fallback
        format!("\\hat{{{}}}", base)
    }
}

fn emit_style_call(cmd: &str, args: &[MathArg]) -> String {
    let body = get_positional(args, 0).map(|n| emit(n)).unwrap_or_default();
    format!("{}{{{}}}", cmd, body)
}

fn emit_cancel_call(args: &[MathArg]) -> String {
    let body = get_positional(args, 0).map(|n| emit(n)).unwrap_or_default();
    let inverted = get_named(args, "inverted")
        .map(|n| emit(n).contains("true"))
        .unwrap_or(false);
    let cross = get_named(args, "cross")
        .map(|n| emit(n).contains("true"))
        .unwrap_or(false);
    if cross {
        format!("\\xcancel{{{}}}", body)
    } else if inverted {
        format!("\\bcancel{{{}}}", body)
    } else {
        format!("\\cancel{{{}}}", body)
    }
}

fn emit_display_style(cmd: &str, args: &[MathArg]) -> String {
    let body = get_positional(args, 0).map(|n| emit(n)).unwrap_or_default();
    format!("{} {}", cmd, body)
}

fn emit_unary_cmd(cmd: &str, args: &[MathArg]) -> String {
    let body = get_positional(args, 0).map(|n| emit(n)).unwrap_or_default();
    format!("{}{{{}}}", cmd, body)
}

fn emit_brace_annotation(cmd: &str, script: &str, args: &[MathArg]) -> String {
    let body = get_positional(args, 0).map(|n| emit(n)).unwrap_or_default();
    let annotation = get_positional(args, 1).map(|n| emit(n));

    match annotation {
        Some(ann) => format!("{}{{{}}}{}{{{}}} ", cmd, body, script, ann),
        None => format!("{}{{{}}}", cmd, body),
    }
}

fn emit_op_call(args: &[MathArg]) -> String {
    // op("text") => \operatorname{text}
    let text = get_positional(args, 0)
        .map(|n| {
            // Extract raw text from StringLit or Text nodes
            if let Some(MathNode::StringLit(s)) = n.first() {
                s.clone()
            } else {
                emit(n)
            }
        })
        .unwrap_or_default();

    // Check if it matches a standard LaTeX operator
    if let Some((latex_cmd, _)) = PREDEFINED_OPERATORS.get(text.as_str()) {
        latex_cmd.to_string()
    } else {
        format!("\\operatorname{{{}}}", text)
    }
}

fn emit_limits_call(args: &[MathArg]) -> String {
    let body = get_positional(args, 0).map(|n| emit(n)).unwrap_or_default();
    format!("{}\\limits", body)
}

fn emit_scripts_call(args: &[MathArg]) -> String {
    // scripts() forces subscript/superscript (non-limits) placement
    let body = get_positional(args, 0).map(|n| emit(n)).unwrap_or_default();
    format!("{}\\nolimits", body)
}

fn emit_class_call(args: &[MathArg]) -> String {
    // class("binary", x) just emits x for our purposes
    get_positional(args, 1)
        .or_else(|| get_positional(args, 0))
        .map(|n| emit(n))
        .unwrap_or_default()
}

fn emit_first_positional(args: &[MathArg]) -> String {
    get_positional(args, 0).map(|n| emit(n)).unwrap_or_default()
}

fn delim_to_matrix_env(delim: &str) -> (&str, &str, &str) {
    match delim.trim().trim_matches(|c| c == '"' || c == '\\') {
        "(" | "paren" => ("pmatrix", "(", ")"),
        "[" | "bracket" => ("bmatrix", "[", "]"),
        "{" | "brace" => ("Bmatrix", "\\{", "\\}"),
        "|" | "bar" => ("vmatrix", "|", "|"),
        "||" | "bar.double" => ("Vmatrix", "\\|", "\\|"),
        _ => ("pmatrix", "(", ")"),
    }
}
