use std::path::Path;

use anyhow::{Context, Result};

pub(crate) fn minify_html(source: &str) -> String {
    // minify-html currently parses MathML as HTML, which corrupts self-closing
    // MathML elements such as <mspace/> and can swallow following content.
    if source.contains("<math") {
        return source.to_owned();
    }

    let mut cfg = minify_html::Cfg::new();
    cfg.keep_html_and_head_opening_tags = true;
    cfg.minify_css = true;
    cfg.minify_js = true;
    cfg.preserve_brace_template_syntax = true;
    String::from_utf8_lossy(&minify_html::minify(source.as_bytes(), &cfg)).into_owned()
}

pub(crate) fn minify_html_file(path: &Path) -> Result<()> {
    let html = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let minified = minify_html(&html);
    if minified != html {
        std::fs::write(path, minified)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}
