//! Escaping Python prose into Typst markup.
//!
//! Docstrings are full of characters Typst treats as syntax — `*args`,
//! `**kwargs`, `_private`, `$PATH`, `@decorator`, `[1]` citations. Everything
//! that is not an explicit code span gets escaped; code spans become `#raw(..)`
//! calls, which sidestep delimiter collisions entirely.

/// Characters that always carry markup meaning, wherever they appear.
const ALWAYS_ESCAPE: &[char] = &['\\', '#', '$', '*', '_', '`', '[', ']', '<', '>', '@', '~'];

/// Characters that only start a construct at the beginning of a line.
const LINE_START_ESCAPE: &[char] = &['=', '-', '+', '/'];

/// Escape a plain-text run for use inside a Typst content block.
pub fn escape_content(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut at_line_start = true;
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\n' {
            out.push('\n');
            at_line_start = true;
            continue;
        }

        let leading_space = at_line_start && (ch == ' ' || ch == '\t');

        if ALWAYS_ESCAPE.contains(&ch) || (at_line_start && LINE_START_ESCAPE.contains(&ch)) {
            out.push('\\');
            out.push(ch);
        } else if at_line_start && ch.is_ascii_digit() {
            // `1.` at line start would become an enumeration item.
            let mut digits = String::from(ch);
            while let Some(next) = chars.peek() {
                if next.is_ascii_digit() {
                    digits.push(*next);
                    chars.next();
                } else {
                    break;
                }
            }
            out.push_str(&digits);
            if chars.peek() == Some(&'.') {
                chars.next();
                out.push_str("\\.");
            }
        } else {
            out.push(ch);
        }

        if !leading_space {
            at_line_start = false;
        }
    }

    out
}

/// Escape a value for use inside a Typst string literal.
pub fn escape_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 4);
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

/// Convert a docstring description into Typst content, turning Markdown-style
/// code spans into `#raw(..)` and escaping everything else.
///
/// Double-backtick spans are recognised before single-backtick ones, so
/// reStructuredText literals (``` ``x`` ```) survive intact.
pub fn render_prose(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;

    while !rest.is_empty() {
        let Some(open) = rest.find('`') else {
            out.push_str(&escape_content(rest));
            break;
        };

        out.push_str(&escape_content(&rest[..open]));
        let after = &rest[open..];

        let (delimiter, body_start) = if after.starts_with("``") {
            ("``", 2)
        } else {
            ("`", 1)
        };

        match after[body_start..].find(delimiter) {
            Some(offset) => {
                let code = &after[body_start..body_start + offset];
                out.push_str(&format!("#raw(\"{}\")", escape_string(code)));
                rest = &after[body_start + offset + delimiter.len()..];
            }
            None => {
                // Unterminated span: treat the backtick as literal text.
                out.push_str(&escape_content(&after[..body_start]));
                rest = &after[body_start..];
            }
        }
    }

    out
}
