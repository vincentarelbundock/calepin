//! Marker systems for protecting content through markdown conversion.
//!
//! Format-specific output (math, span templates, shortcodes) must survive
//! comrak's markdown-to-format conversion without being re-escaped. This
//! module stores such content in indexed vecs and replaces it with opaque
//! marker strings that comrak passes through unchanged.
//!
//! ## Marker format
//!
//! Every marker uses Unicode noncharacters as delimiters:
//!
//! ```text
//! \u{FFFF}<type><index>\u{FFFE}
//! ```
//!
//! - `M` — math expression (`$...$` or `$$...$$`)
//! - `D` — escaped dollar sign (`\$`)
//! - `L` — equation label (`{#eq-...}`)
//! - `R` — span template output (indexed into external `raw_fragments`)
//! - `S` — shortcode raw output
//! - `X` — escaped shortcode literal
//!
//! Because `\u{FFFF}` and `\u{FFFE}` cannot appear in legitimate document
//! text (they are Unicode noncharacters), markers are collision-proof.
//! Input is sanitized by [`sanitize`] to strip any stray occurrences.

use std::sync::LazyLock;
use regex::Regex;

// ---------------------------------------------------------------------------
// Marker delimiters — Unicode noncharacters, guaranteed absent from input
// ---------------------------------------------------------------------------

/// Start delimiter for all markers.
const MS: char = '\u{FFFF}';
/// End delimiter for all markers.
const ME: char = '\u{FFFE}';

// Type prefixes
const TY_MATH: char = 'M';
const TY_ESC_DOLLAR: char = 'D';
const TY_EQ_LABEL: char = 'L';
const TY_RAW: char = 'R';
const TY_SC_RAW: char = 'S';

// ---------------------------------------------------------------------------
// Compiled regex patterns (one per marker type that needs resolution)
// ---------------------------------------------------------------------------

/// Matches any marker: \u{FFFF}<type><payload>\u{FFFE}
#[allow(dead_code)]
static RE_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("\u{FFFF}([MDLRSX])([^\u{FFFE}]*)\u{FFFE}").unwrap()
});

/// Matches equation label markers adjacent to restored display math.
static RE_EQ_LABEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?s)(\$\$.*?\$\$)\s*{}{}(eq-[a-zA-Z0-9_-]+){}",
        MS, TY_EQ_LABEL, ME
    )).unwrap()
});

// ---------------------------------------------------------------------------
// Input sanitization
// ---------------------------------------------------------------------------

/// Strip marker delimiter characters from input text. Call this once on raw
/// document text before any processing to guarantee markers cannot collide
/// with user content.
pub fn sanitize(text: &str) -> String {
    if text.contains(MS) || text.contains(ME) {
        text.replace(MS, "").replace(ME, "")
    } else {
        text.to_string()
    }
}

// ---------------------------------------------------------------------------
// Marker constructors
// ---------------------------------------------------------------------------

/// Create a math marker: `\u{FFFF}M<idx>\u{FFFE}`
fn math_marker(idx: usize) -> String {
    format!("{}{}{}{}", MS, TY_MATH, idx, ME)
}

/// Create an escaped-dollar marker: `\u{FFFF}D\u{FFFE}`
fn esc_dollar_marker() -> String {
    format!("{}{}{}", MS, TY_ESC_DOLLAR, ME)
}

/// Create an equation label marker: `\u{FFFF}L<label>\u{FFFE}`
fn eq_label_marker(label: &str) -> String {
    format!("{}{}{}{}", MS, TY_EQ_LABEL, label, ME)
}

/// Wrap format-specific span output in a raw marker. Stores the content in
/// `fragments` and returns an opaque `\u{FFFF}R<idx>\u{FFFE}` placeholder.
pub fn wrap_raw(fragments: &mut Vec<String>, content: String) -> String {
    let idx = fragments.len();
    fragments.push(content);
    format!("{}{}{}{}", MS, TY_RAW, idx, ME)
}

/// Wrap shortcode output in a raw marker. Stores the content in `fragments`
/// and returns an opaque `\u{FFFF}S<idx>\u{FFFE}` placeholder.
pub fn wrap_shortcode_raw(fragments: &mut Vec<String>, content: String) -> String {
    let idx = fragments.len();
    fragments.push(content);
    format!("{}{}{}{}", MS, TY_SC_RAW, idx, ME)
}

// ---------------------------------------------------------------------------
// Math protection
// ---------------------------------------------------------------------------

/// Protect math expressions from comrak processing.
///
/// Every `$...$` and `$$...$$` is replaced with an indexed marker.
/// `\$` becomes an escaped-dollar marker (resolved per-format later).
/// An equation label `{#eq-label}` after display math becomes a label marker.
///
/// Returns `(protected_text, math_expressions)`.
pub fn protect_math(text: &str) -> (String, Vec<String>) {
    let mut protected = String::with_capacity(text.len());
    let mut expressions: Vec<String> = Vec::new();

    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut in_code = false;
    let mut code_backtick_count: usize = 0;

    while i < len {
        let ch = bytes[i];

        if ch == b'`' {
            let mut run_len = 1;
            while i + run_len < len && bytes[i + run_len] == b'`' {
                run_len += 1;
            }

            if !in_code {
                in_code = true;
                code_backtick_count = run_len;
            } else if run_len == code_backtick_count {
                in_code = false;
                code_backtick_count = 0;
            }

            for _ in 0..run_len {
                protected.push('`');
            }
            i += run_len;
            continue;
        }

        if in_code {
            if ch < 0x80 {
                protected.push(ch as char);
                i += 1;
            } else {
                let c = text[i..].chars().next().unwrap();
                protected.push(c);
                i += c.len_utf8();
            }
            continue;
        }

        // Escaped dollar: \$ → marker (resolved per-format after conversion)
        if ch == b'\\' && i + 1 < len && bytes[i + 1] == b'$' {
            protected.push_str(&esc_dollar_marker());
            i += 2;
            continue;
        }

        if ch == b'$' {
            let is_display = i + 1 < len && bytes[i + 1] == b'$';
            let delim_len = if is_display { 2 } else { 1 };
            let start = i + delim_len;

            let mut end = None;
            let mut j = start;
            while j < len {
                if bytes[j] == b'\\' && j + 1 < len && bytes[j + 1] == b'$' {
                    j += 2;
                    continue;
                }
                if is_display {
                    if bytes[j] == b'$' && j + 1 < len && bytes[j + 1] == b'$' {
                        end = Some(j);
                        break;
                    }
                } else if bytes[j] == b'$' {
                    end = Some(j);
                    break;
                }
                j += 1;
            }

            if let Some(end_pos) = end {
                let math_content = &text[i..end_pos + delim_len];
                protected.push_str(&math_marker(expressions.len()));
                expressions.push(math_content.to_string());
                i = end_pos + delim_len;

                // For display math, check for {#eq-label} after the closing $$
                if is_display {
                    let rest = &text[i..];
                    let trimmed = rest.trim_start();
                    if trimmed.starts_with("{#eq-") {
                        if let Some(close) = trimmed.find('}') {
                            let label = &trimmed[2..close]; // "eq-something"
                            let skipped = rest.len() - trimmed.len() + close + 1;
                            protected.push_str(&eq_label_marker(label));
                            i += skipped;
                        }
                    }
                }
            } else {
                protected.push('$');
                if is_display {
                    protected.push('$');
                }
                i += delim_len;
            }
        } else if ch < 0x80 {
            protected.push(ch as char);
            i += 1;
        } else {
            let c = text[i..].chars().next().unwrap();
            protected.push(c);
            i += c.len_utf8();
        }
    }

    (protected, expressions)
}

/// Restore math expressions from markers.
pub fn restore_math(text: &str, expressions: &[String]) -> String {
    // Fast path: no markers
    if !text.contains(MS) {
        return text.to_string();
    }
    let re = Regex::new(&format!("{}{}(\\d+){}", MS, TY_MATH, ME)).unwrap();
    re.replace_all(text, |caps: &regex::Captures| {
        let idx: usize = caps[1].parse().unwrap_or(usize::MAX);
        expressions.get(idx).cloned().unwrap_or_default()
    }).to_string()
}

/// Resolve equation labels: wrap display math + label in format-specific containers.
/// Must be called after `restore_math`.
pub fn resolve_equation_labels(text: &str, format: &str) -> String {
    if !text.contains(MS) {
        return text.to_string();
    }
    RE_EQ_LABEL.replace_all(text, |caps: &regex::Captures| {
        let math = &caps[1];
        let label = &caps[2];
        let inner = &math[2..math.len() - 2];
        match format {
            "html" => format!(
                "<div class=\"equation\" id=\"{}\">\n{}\n</div>",
                label, math
            ),
            "latex" => format!(
                "\\begin{{equation}}\n{}\n\\label{{{}}}\n\\end{{equation}}",
                inner.trim(), label
            ),
            "typst" => format!(
                "$ {} $ <{}>",
                inner.trim(), label
            ),
            _ => math.to_string(),
        }
    }).to_string()
}

/// Resolve escaped dollar signs for the given output format.
pub fn resolve_escaped_dollars(text: &str, format: &str) -> String {
    if !text.contains(MS) {
        return text.to_string();
    }
    let marker = esc_dollar_marker();
    let replacement = match format {
        "html" => "<span class=\"nodollar\">$</span>",
        _ => "\\$",
    };
    text.replace(&marker, replacement)
}

// ---------------------------------------------------------------------------
// Raw output resolution
// ---------------------------------------------------------------------------

/// Resolve raw span markers (`R` type) to their content.
pub fn resolve_raw(text: &str, fragments: &[String]) -> String {
    if !text.contains(MS) {
        return text.to_string();
    }
    let re = Regex::new(&format!("{}{}(\\d+){}", MS, TY_RAW, ME)).unwrap();
    re.replace_all(text, |caps: &regex::Captures| {
        let idx: usize = caps[1].parse().unwrap_or(usize::MAX);
        fragments.get(idx).cloned().unwrap_or_default()
    }).to_string()
}

/// Resolve shortcode raw markers (`S` type) to their content.
pub fn resolve_shortcode_raw(text: &str, fragments: &[String]) -> String {
    if !text.contains(MS) {
        return text.to_string();
    }
    let re = Regex::new(&format!("{}{}(\\d+){}", MS, TY_SC_RAW, ME)).unwrap();
    re.replace_all(text, |caps: &regex::Captures| {
        let idx: usize = caps[1].parse().unwrap_or(usize::MAX);
        fragments.get(idx).cloned().unwrap_or_default()
    }).to_string()
}

/// Resolve all marker types in a single pass. This is the preferred entry
/// point when all fragment vecs are available.
#[allow(dead_code)]
pub fn resolve_all(
    text: &str,
    format: &str,
    math: &[String],
    raw_fragments: &[String],
    sc_fragments: &[String],
) -> String {
    if !text.contains(MS) {
        return text.to_string();
    }

    let esc_dollar_replacement = match format {
        "html" => "<span class=\"nodollar\">$</span>",
        _ => "\\$",
    };

    RE_MARKER.replace_all(text, |caps: &regex::Captures| {
        let ty = caps[1].chars().next().unwrap_or('?');
        let payload = &caps[2];
        match ty {
            'M' => {
                let idx: usize = payload.parse().unwrap_or(usize::MAX);
                math.get(idx).cloned().unwrap_or_default()
            }
            'D' => esc_dollar_replacement.to_string(),
            'R' => {
                let idx: usize = payload.parse().unwrap_or(usize::MAX);
                raw_fragments.get(idx).cloned().unwrap_or_default()
            }
            'S' => {
                let idx: usize = payload.parse().unwrap_or(usize::MAX);
                sc_fragments.get(idx).cloned().unwrap_or_default()
            }
            _ => String::new(),
        }
    }).to_string()
}

// ---------------------------------------------------------------------------
// Preprocessing (shared by markdown.rs and latex.rs)
// ---------------------------------------------------------------------------

/// Convert inline footnotes `^[text]` to regular footnotes.
/// Generates `[^snb-N]` references and appends `[^snb-N]: text` definitions.
fn expand_inline_footnotes(text: &str) -> String {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\^\[([^\]]+)\]").unwrap()
    });
    let mut counter = 0;
    let mut definitions = Vec::new();

    let result = RE.replace_all(text, |caps: &regex::Captures| {
        let full_match = caps.get(0).unwrap();
        // Skip if inside a backtick code span
        let before = &text[..full_match.start()];
        if before.chars().filter(|&c| c == '`').count() % 2 != 0 {
            return caps[0].to_string();
        }
        counter += 1;
        let label = format!("snb-ifn-{}", counter);
        definitions.push(format!("[^{}]: {}", label, &caps[1]));
        format!("[^{}]", label)
    }).to_string();

    if definitions.is_empty() {
        return result;
    }

    format!("{}\n\n{}", result, definitions.join("\n"))
}

/// Convert Pandoc-style line blocks (`| line`) to preserved-whitespace blocks.
fn expand_line_blocks(text: &str) -> String {
    let mut result = String::new();
    let mut in_block = false;
    let mut block_lines: Vec<String> = Vec::new();

    for line in text.lines() {
        if let Some(content) = line.strip_prefix("| ").or_else(|| {
            if line == "|" { Some("") } else { None }
        }) {
            if !in_block {
                in_block = true;
                block_lines.clear();
            }
            let leading_spaces = content.len() - content.trim_start().len();
            let indent = "\u{00A0}".repeat(leading_spaces);
            block_lines.push(format!("{}{}", indent, content.trim_start()));
        } else {
            if in_block {
                result.push_str(&block_lines.join("  \n"));
                result.push('\n');
                in_block = false;
            }
            result.push_str(line);
            result.push('\n');
        }
    }

    if in_block {
        result.push_str(&block_lines.join("  \n"));
        result.push('\n');
    }

    if !text.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    result
}

/// Apply Pandoc-extension preprocessing before comrak.
pub fn preprocess(text: &str) -> String {
    let text = expand_inline_footnotes(text);
    expand_line_blocks(&text)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_strips_delimiters() {
        let input = format!("hello{}world{}!", MS, ME);
        assert_eq!(sanitize(&input), "helloworld!");
    }

    #[test]
    fn test_sanitize_noop_clean_input() {
        let input = "hello world!";
        assert_eq!(sanitize(input), input);
    }

    #[test]
    fn test_protect_math_inline() {
        let (protected, exprs) = protect_math("The value $x^2$ is important.");
        assert_eq!(exprs.len(), 1);
        assert_eq!(exprs[0], "$x^2$");
        assert!(protected.contains(MS));
        assert!(!protected.contains('$'));
    }

    #[test]
    fn test_protect_math_display() {
        let (protected, exprs) = protect_math("$$a + b$$");
        assert_eq!(exprs.len(), 1);
        assert_eq!(exprs[0], "$$a + b$$");
        assert!(protected.contains(MS));
    }

    #[test]
    fn test_protect_math_escaped_dollar() {
        let (protected, exprs) = protect_math(r"Costs \$5.");
        assert!(exprs.is_empty());
        assert!(protected.contains(&esc_dollar_marker()));
    }

    #[test]
    fn test_restore_math_roundtrip() {
        let input = "Before $a^2$ and $b^2$ after.";
        let (protected, exprs) = protect_math(input);
        let restored = restore_math(&protected, &exprs);
        assert_eq!(restored, input);
    }

    #[test]
    fn test_resolve_escaped_dollars_html() {
        let marker = esc_dollar_marker();
        let text = format!("Price: {}5", marker);
        let result = resolve_escaped_dollars(&text, "html");
        assert!(result.contains("nodollar"));
        assert!(result.contains("$"));
    }

    #[test]
    fn test_resolve_escaped_dollars_latex() {
        let marker = esc_dollar_marker();
        let text = format!("Price: {}5", marker);
        let result = resolve_escaped_dollars(&text, "latex");
        assert!(result.contains("\\$5"));
    }

    #[test]
    fn test_wrap_raw_roundtrip() {
        let mut fragments = Vec::new();
        let marker = wrap_raw(&mut fragments, "\\textbf{hello}".to_string());
        assert_eq!(fragments.len(), 1);
        let resolved = resolve_raw(&marker, &fragments);
        assert_eq!(resolved, "\\textbf{hello}");
    }

    #[test]
    fn test_wrap_shortcode_raw_roundtrip() {
        let mut fragments = Vec::new();
        let marker = wrap_shortcode_raw(&mut fragments, "\\newpage{}".to_string());
        assert_eq!(fragments.len(), 1);
        let resolved = resolve_shortcode_raw(&marker, &fragments);
        assert_eq!(resolved, "\\newpage{}");
    }

    #[test]
    fn test_resolve_all_mixed() {
        let mut raw = Vec::new();
        let mut sc = Vec::new();
        let (math_text, math_exprs) = protect_math("$x$ hello");
        let raw_marker = wrap_raw(&mut raw, "<b>bold</b>".to_string());
        let sc_marker = wrap_shortcode_raw(&mut sc, "\\newpage".to_string());
        let text = format!("{} {} {}", math_text, raw_marker, sc_marker);
        let result = resolve_all(&text, "html", &math_exprs, &raw, &sc);
        assert!(result.contains("$x$"));
        assert!(result.contains("<b>bold</b>"));
        assert!(result.contains("\\newpage"));
    }

    #[test]
    fn test_markers_cannot_collide_with_user_content() {
        // Even if user writes "SNBMATH" or similar, no collision
        let input = "The marker SNBMATH0SNBMATH should be literal text.";
        let (protected, exprs) = protect_math(input);
        let restored = restore_math(&protected, &exprs);
        assert_eq!(restored, input);
    }

    #[test]
    fn test_equation_label() {
        let (protected, exprs) = protect_math("$$E = mc^2$$ {#eq-einstein}");
        assert_eq!(exprs.len(), 1);
        let restored = restore_math(&protected, &exprs);
        assert!(restored.contains("$$E = mc^2$$"));
        let html = resolve_equation_labels(&restored, "html");
        assert!(html.contains("id=\"eq-einstein\""));
    }

    #[test]
    fn test_math_in_code_not_protected() {
        let (_, exprs) = protect_math("In code: `$x$` is literal.");
        assert!(exprs.is_empty());
    }
}
