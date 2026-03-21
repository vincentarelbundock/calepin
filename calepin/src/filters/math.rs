// Math filters: format-specific math transformations.
//
// - strip_math_for_typst() — Remove LaTeX math from Typst output.

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
