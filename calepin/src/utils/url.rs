use std::path::PathBuf;

use crate::utils::path::slash_path;

pub(crate) fn output_href_with_extension(url: &str, extension: &str) -> String {
    let url = url.trim();
    if is_external_url(url) {
        return url.to_string();
    }
    let url = url.trim_start_matches('/').trim_start_matches("./");
    if url.ends_with('/') {
        return format!("{url}index.{extension}");
    }
    let mut path = PathBuf::from(url);
    path.set_extension(extension);
    slash_path(&path)
}

fn is_external_url(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once("://") else {
        return false;
    };
    scheme
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || char == '+' || char == '-' || char == '.')
        && !rest.is_empty()
}

#[cfg(test)]
mod tests {
    use super::output_href_with_extension;

    #[test]
    fn output_href_with_extension_normalizes_authored_urls() {
        assert_eq!(
            output_href_with_extension("/guide.typ", "html"),
            "guide.html"
        );
        assert_eq!(
            output_href_with_extension("./notes/intro.pdf", "html"),
            "notes/intro.html"
        );
        assert_eq!(
            output_href_with_extension("slides/", "pdf"),
            "slides/index.pdf"
        );
    }

    #[test]
    fn output_href_with_extension_keeps_external_urls() {
        assert_eq!(
            output_href_with_extension("https://example.com/path", "html"),
            "https://example.com/path"
        );
    }
}
