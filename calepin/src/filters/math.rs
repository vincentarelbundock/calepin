// Math filters: format-specific math transformations.
//
// - strip_math_for_typst()   — Remove LaTeX math from Typst output.
// - convert_math_for_typst() — Convert LaTeX math to Typst math syntax.

use std::sync::LazyLock;

use regex::Regex;

/// Strip LaTeX math from Typst output — Typst uses its own math syntax,
/// so `$$...$$` display math and `$...$` inline math are removed.
pub fn strip_math_for_typst(text: &str) -> String {
    // Display math: $$ ... $$ (possibly spanning multiple lines)
    static RE_DISPLAY: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?s)\$\$.*?\$\$").unwrap()
    });
    // Any remaining $ ... $ (including multi-line, e.g. from equation label resolution)
    static RE_DOLLAR: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?s)\$[^$]+?\$").unwrap()
    });
    let text = RE_DISPLAY.replace_all(text, "").to_string();
    RE_DOLLAR.replace_all(&text, "").to_string()
}

/// Convert LaTeX math to Typst math syntax in Typst output.
///
/// - `$$latex$$` (display) becomes `$ typst $`
/// - `$latex$` (inline) becomes `$typst$`
///
/// Uses a single regex that matches both forms to avoid the inline regex
/// re-matching converted display math output.
pub fn convert_math_for_typst(text: &str) -> String {
    // Single regex: match display $$..$$ first (longer delimiter wins), then inline $..$
    static RE_MATH: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?s)\$\$(.*?)\$\$|\$([^$]+?)\$").unwrap()
    });

    RE_MATH.replace_all(text, |caps: &regex::Captures| {
        if let Some(display) = caps.get(1) {
            // Display math: $$ ... $$
            let converted = crate::math::latex_to_typst(display.as_str().trim());
            format!("$ {} $", converted)
        } else if let Some(inline) = caps.get(2) {
            // Inline math: $ ... $
            let converted = crate::math::latex_to_typst(inline.as_str());
            format!("${}$", converted)
        } else {
            caps[0].to_string()
        }
    }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_inline() {
        let result = convert_math_for_typst("text $\\alpha + \\beta$ more");
        assert_eq!(result, "text $alpha + beta$ more");
    }

    #[test]
    fn test_convert_display() {
        let result = convert_math_for_typst("before $$\\alpha + \\beta$$ after");
        assert_eq!(result, "before $ alpha + beta $ after");
    }

    #[test]
    fn test_convert_frac() {
        let result = convert_math_for_typst("$\\frac{a}{b}$");
        assert_eq!(result, "$frac(a, b)$");
    }

    #[test]
    fn test_strip_removes_math() {
        let result = strip_math_for_typst("text $x^2$ more $$a+b$$ end");
        assert_eq!(result, "text  more  end");
    }
}
