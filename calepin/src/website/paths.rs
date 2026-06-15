use std::path::Path;

pub(super) use crate::utils::path::normalize_path;

pub(super) fn rel_posix(src_dir: &Path, path: &Path) -> String {
    path.strip_prefix(src_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(super) fn slash_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

pub(super) fn wildcard_match(pattern: &str, value: &str) -> bool {
    let pattern = glob_tokens(pattern);
    let value = value.as_bytes();
    let mut previous = vec![false; value.len() + 1];
    previous[0] = true;
    for token in pattern {
        let mut current = vec![false; value.len() + 1];
        if matches!(token, GlobToken::SegmentStar | GlobToken::GlobStar) {
            current[0] = previous[0];
        }
        for j in 1..=value.len() {
            current[j] = match token {
                GlobToken::Literal(byte) => previous[j - 1] && byte == value[j - 1],
                GlobToken::AnyChar => previous[j - 1] && value[j - 1] != b'/',
                GlobToken::SegmentStar => previous[j] || (value[j - 1] != b'/' && current[j - 1]),
                GlobToken::GlobStar => previous[j] || current[j - 1],
            };
        }
        previous = current;
    }
    previous[value.len()]
}

#[derive(Clone, Copy)]
enum GlobToken {
    Literal(u8),
    AnyChar,
    SegmentStar,
    GlobStar,
}

fn glob_tokens(pattern: &str) -> Vec<GlobToken> {
    let bytes = pattern.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'*' => {
                let start = index;
                while index < bytes.len() && bytes[index] == b'*' {
                    index += 1;
                }
                if index - start >= 2 {
                    tokens.push(GlobToken::GlobStar);
                } else {
                    tokens.push(GlobToken::SegmentStar);
                }
            }
            b'?' => {
                tokens.push(GlobToken::AnyChar);
                index += 1;
            }
            byte => {
                tokens.push(GlobToken::Literal(byte));
                index += 1;
            }
        }
    }
    tokens
}
