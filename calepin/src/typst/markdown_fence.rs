pub(super) fn leading_backtick_count(value: &str) -> usize {
    value.chars().take_while(|ch| *ch == '`').count()
}

pub(super) fn is_closing_fence(trimmed_line: &str, fence_len: usize) -> bool {
    let closing_len = leading_backtick_count(trimmed_line);
    if closing_len < fence_len {
        return false;
    }
    let rest = trimmed_line[closing_len..].trim_start();
    rest.is_empty() || rest.starts_with('<')
}
