use std::path::PathBuf;

use crate::utils::path::slash_path;

pub(crate) fn output_href_with_extension(url: &str, extension: &str) -> String {
    let url = url.trim().trim_start_matches('/').trim_start_matches("./");
    if url.ends_with('/') {
        return format!("{url}index.{extension}");
    }
    let mut path = PathBuf::from(url);
    path.set_extension(extension);
    slash_path(&path)
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
}
