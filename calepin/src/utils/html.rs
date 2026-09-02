/// Escape unsafe HTML characters commonly used in text and double-quoted
/// attribute values.
pub fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::escape;

    #[test]
    fn escapes_single_quotes() {
        // A title containing `'` must not close a single-quoted attribute it
        // is later embedded into (e.g. a TOC link's href).
        assert_eq!(escape("O'Brien's"), "O&#39;Brien&#39;s");
    }
}
