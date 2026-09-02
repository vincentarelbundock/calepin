//! Minimal, string-level scanning helpers for locating Typst call syntax
//! (`#name(...)`) inside raw source text without a full parser. Used by
//! `website::metadata`, which looks for `#metadata(...)` calls this way;
//! `health::source` has an equivalent set of helpers for the same reason
//! (delimiter matching, string literals, identifier boundaries) but is owned
//! by a different workstream, so it is not wired to this module yet.

/// Find the index of the `close` delimiter matching the `open` delimiter at
/// `open_index`, skipping over string literals and nested pairs.
pub fn find_matching_delimiter(
    value: &str,
    open_index: usize,
    open: char,
    close: char,
) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = open_index;
    while index < value.len() {
        let ch = value[index..].chars().next()?;
        if ch == '"' {
            index = find_string_end(value, index)? + 1;
            continue;
        }
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(index);
            }
        }
        index += ch.len_utf8();
    }
    None
}

/// Index of the closing `"` for the string literal starting at `quote_index`.
pub fn find_string_end(value: &str, quote_index: usize) -> Option<usize> {
    let mut escaped = false;
    let mut index = quote_index + 1;
    while index < value.len() {
        let ch = value[index..].chars().next()?;
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(index);
        }
        index += ch.len_utf8();
    }
    None
}

pub fn skip_ws(value: &str, mut index: usize) -> usize {
    while index < value.len() {
        let Some(ch) = value[index..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

pub fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'
}

pub fn is_left_identifier_boundary(value: &str, index: usize) -> bool {
    index == 0
        || !value[..index]
            .chars()
            .next_back()
            .is_some_and(is_identifier_char)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_matching_delimiter_skips_nested_pairs_and_strings() {
        let value = r#"(a, (b, ")("), c)"#;
        let close = find_matching_delimiter(value, 0, '(', ')').unwrap();
        assert_eq!(&value[close..=close], ")");
        assert_eq!(close, value.len() - 1);
    }

    #[test]
    fn is_left_identifier_boundary_rejects_mid_identifier() {
        assert!(is_left_identifier_boundary("x metadata", 2));
        assert!(!is_left_identifier_boundary("xmetadata", 1));
    }
}
