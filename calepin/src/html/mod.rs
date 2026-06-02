mod assets;
mod syntax;
mod theme;

use anyhow::{anyhow, Context, Result};
use minijinja::{AutoEscape, Environment};
use serde::Serialize;
use std::path::{Path, PathBuf};

use syntax::HtmlSyntaxTheme;

#[derive(Serialize)]
struct HtmlInMarkdownStyle {
    css: String,
}

const HTML_INPUT_LIGHT_THEME_PATH: &str = ".calepin/calepin-input-light.tmTheme";
const HTML_INPUT_LIGHT_THEME_REF: &str = "/.calepin/calepin-input-light.tmTheme";
const HTML_IN_MD_LAYOUT: &str = include_str!("../templates/html/html-in-md/layout.html");

#[derive(Debug, Clone)]
pub(crate) struct PreparedHtmlTheme {
    pub(crate) syntax_theme: HtmlSyntaxTheme,
    pub(crate) raw_theme_input: Option<String>,
}

pub(crate) fn prepare_html_theme(
    root: &Path,
    format: Option<&str>,
    html_theme: Option<&str>,
    html_theme_light: Option<&str>,
    html_theme_dark: Option<&str>,
) -> Result<PreparedHtmlTheme> {
    if format != Some("html") {
        return Ok(PreparedHtmlTheme {
            syntax_theme: HtmlSyntaxTheme::builtin(),
            raw_theme_input: None,
        });
    }

    match (html_theme_light, html_theme_dark) {
        (None, None) => Ok(PreparedHtmlTheme {
            syntax_theme: HtmlSyntaxTheme::builtin(),
            raw_theme_input: None,
        }),
        (Some(light), Some(dark)) => {
            if html_theme.is_none() {
                return Err(anyhow!(
                    "`html-theme-light` and `html-theme-dark` require `html-theme`"
                ));
            }
            let light_path = resolve_setup_theme_path(root, light);
            let dark_path = resolve_setup_theme_path(root, dark);
            let light_source = std::fs::read_to_string(&light_path)
                .with_context(|| format!("failed to read {}", light_path.display()))?;
            let dark_source = std::fs::read_to_string(&dark_path)
                .with_context(|| format!("failed to read {}", dark_path.display()))?;
            let syntax_theme = HtmlSyntaxTheme::from_tmtheme_sources(&light_source, &dark_source)?;

            let prepared_path = root.join(HTML_INPUT_LIGHT_THEME_PATH);
            if let Some(parent) = prepared_path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            std::fs::write(&prepared_path, light_source)
                .with_context(|| format!("failed to write {}", prepared_path.display()))?;

            Ok(PreparedHtmlTheme {
                syntax_theme,
                raw_theme_input: Some(HTML_INPUT_LIGHT_THEME_REF.to_string()),
            })
        }
        _ => Err(anyhow!(
            "`html-theme-light` and `html-theme-dark` must be supplied together"
        )),
    }
}

pub(crate) fn apply_html_theme_file(
    path: &Path,
    html_theme: Option<&str>,
    themes_dir: &Path,
    syntax_theme: &HtmlSyntaxTheme,
) -> Result<()> {
    let html = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let themed = theme::apply_html_theme(&html, html_theme, themes_dir, syntax_theme)?;
    if themed != html {
        std::fs::write(path, themed)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

pub(crate) fn inline_html_images_file(path: &Path, root: &Path) -> Result<()> {
    let html = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let base_dir = path.parent().unwrap_or(root);
    let inlined = assets::inline_html_images(&html, root, base_dir)?;
    if inlined != html {
        std::fs::write(path, inlined)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

pub(crate) fn render_html_in_markdown(path: &Path) -> Result<()> {
    let rendered = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let head = extract_tag_content(&rendered, "head")
        .context("generated Calepin HTML is missing <head>")?;
    let body = extract_tag_content(&rendered, "body")
        .context("generated Calepin HTML is missing <body>")?;
    let styles = extract_style_tags(&head)
        .into_iter()
        .map(|css| HtmlInMarkdownStyle {
            css: rewrite_artifact_urls(&css),
        })
        .collect();
    let body = rewrite_artifact_urls(body.trim());
    let context = HtmlInMarkdownTemplateContext { styles, body };
    let rendered = render_output_template(context)?;

    std::fs::write(path, rendered.trim_start())
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

#[derive(Serialize)]
struct HtmlInMarkdownTemplateContext {
    styles: Vec<HtmlInMarkdownStyle>,
    body: String,
}

fn render_output_template(context: HtmlInMarkdownTemplateContext) -> Result<String> {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|_| AutoEscape::None);
    env.add_template("html-in-md", HTML_IN_MD_LAYOUT)
        .with_context(|| anyhow!("failed to load html-in-md template"))?;
    let template = env
        .get_template("html-in-md")
        .map_err(|error| anyhow!("failed to load html-in-md layout: {error}"))?;
    template
        .render(&context)
        .map_err(|error| anyhow!("failed to render html-in-md template: {error}"))
}

fn extract_tag_content(document: &str, tag: &str) -> Result<String> {
    let open_tag = format!("<{}", tag);
    let close_tag = format!("</{}>", tag);
    let start = document
        .find(&open_tag)
        .ok_or_else(|| anyhow!("missing opening tag <{}>", tag))?;
    let open_end = start
        + document[start..]
            .find('>')
            .ok_or_else(|| anyhow!("malformed opening <{}>", tag))?
            + 1;
    let end = open_end
        .checked_add(document[open_end..].find(&close_tag).ok_or_else(|| {
            anyhow!("missing closing tag </{}>", tag)
        })?)
        .ok_or_else(|| anyhow!("invalid HTML for tag {}", tag))?;

    Ok(document[open_end..end].to_string())
}

fn extract_style_tags(head: &str) -> Vec<String> {
    const STYLE_START: &str = "<style";
    const STYLE_END: &str = "</style>";
    let mut head_cursor = head;
    let mut styles = Vec::new();

    while let Some(start_offset) = head_cursor.find(STYLE_START) {
        let style_start = start_offset;
        let style_open_end = match head_cursor[style_start..].find('>') {
            Some(offset) => style_start + offset + 1,
            None => break,
        };
        let style_close_offset = match head_cursor[style_open_end..].find(STYLE_END) {
            Some(offset) => style_open_end + offset + STYLE_END.len(),
            None => break,
        };

        let css = &head_cursor[style_open_end..(style_close_offset - STYLE_END.len())];
        styles.push(css.trim().to_string());
        head_cursor = &head_cursor[style_close_offset..];
    }

    styles
}

fn rewrite_artifact_urls(html: &str) -> String {
    html.replace(r#"src="/.calepin/"#, r#"src=".calepin/"#)
        .replace(r#"src='/.calepin/"#, r#"src='.calepin/"#)
        .replace(r#"href="/.calepin/"#, r#"href=".calepin/"#)
        .replace(r#"href='/.calepin/"#, r#"href='.calepin/"#)
}

fn resolve_setup_theme_path(root: &Path, value: &str) -> PathBuf {
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_HTML: &str = "<html><head><title>Standard Title</title></head><body><h1>Standard Title</h1></body></html>";

    // Render with the builtin syntax theme against an empty themes dir, so a
    // bare name resolves to the embedded built-in theme.
    fn apply_html_theme(html: &str, html_theme: Option<&str>) -> Result<String> {
        let dir = tempfile::tempdir().unwrap();
        theme::apply_html_theme(html, html_theme, dir.path(), &HtmlSyntaxTheme::builtin())
    }

    fn write_theme(themes_dir: &Path, name: &str, layout: &str) {
        let dir = themes_dir.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("layout.html"), layout).unwrap();
    }

    #[test]
    fn pico_html_theme_preserves_title_and_wraps_body() {
        let themed = apply_html_theme(SAMPLE_HTML, Some("pico")).unwrap();

        assert!(themed.contains("<title>Standard Title</title>"));
        assert!(themed.contains("https://cdn.jsdelivr.net/npm/@picocss/pico@2/css/pico.min.css"));
        assert!(themed.contains("<main class=\"container\">"));
        assert!(themed.contains(".sourceCode,"));
        assert!(themed.contains(".cell-output {"));
        assert!(themed.contains("calepin-copy-code"));
        assert!(themed.contains(r#"<nav class="calepin-theme-switcher""#));
        assert!(themed.contains("const themeOrder = [\"\", \"light\", \"dark\"]"));
        assert!(themed.contains("<h1>Standard Title</h1>"));
    }

    #[test]
    fn basic_html_theme_wraps_body_without_pico_artifacts() {
        let themed = apply_html_theme(SAMPLE_HTML, Some("basic")).unwrap();

        assert!(themed.contains("Standard Title"));
        assert!(themed.contains("sourceCode"));
        assert!(!themed.contains("cdn.jsdelivr.net/npm/@picocss/pico"));
        assert!(themed.contains("calepin-syntax-foreground"));
    }

    #[test]
    fn no_html_theme_returns_raw_typst_html_without_calepin_css_or_template() {
        let themed = apply_html_theme(SAMPLE_HTML, None).unwrap();

        assert_eq!(themed, SAMPLE_HTML);
        assert!(!themed.contains("calepin-copy-code"));
        assert!(!themed.contains("cdn.jsdelivr.net/npm/@picocss/pico"));
        assert!(!themed.contains("calepin-theme-switcher"));
    }

    #[test]
    fn html_image_inliner_embeds_root_relative_images() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join(".calepin/paper/figures/fig.svg");
        std::fs::create_dir_all(image.parent().unwrap()).unwrap();
        std::fs::write(&image, "<svg></svg>").unwrap();
        let html = r#"<figure><img src="/.calepin/paper/figures/fig.svg" alt=""></figure>"#;

        let inlined = assets::inline_html_images(html, dir.path(), dir.path()).unwrap();

        assert!(inlined.contains(r#"src="data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=""#));
    }

    #[test]
    fn html_image_inliner_embeds_relative_images() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("fig.png");
        std::fs::write(&image, [0_u8, 1, 2]).unwrap();
        let html = r#"<img alt="x" src='fig.png'>"#;

        let inlined = assets::inline_html_images(html, dir.path(), dir.path()).unwrap();

        assert!(inlined.contains("src='data:image/png;base64,AAEC'"));
    }

    #[test]
    fn html_image_inliner_leaves_external_and_data_images() {
        let dir = tempfile::tempdir().unwrap();
        let html = concat!(
            r#"<img src="https://example.com/fig.png">"#,
            r#"<img src="data:image/png;base64,AA==">"#
        );

        let inlined = assets::inline_html_images(html, dir.path(), dir.path()).unwrap();

        assert_eq!(inlined, html);
    }

    #[test]
    #[test]
    fn html_custom_syntax_themes_require_html_theme() {
        let dir = tempfile::tempdir().unwrap();

        let err = prepare_html_theme(
            dir.path(),
            Some("html"),
            None,
            Some("light.tmTheme"),
            Some("dark.tmTheme"),
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("require `html-theme`"));
    }

    #[test]
    fn unknown_html_theme_errors() {
        let err = apply_html_theme(SAMPLE_HTML, Some("nope"))
            .unwrap_err()
            .to_string();

        assert!(err.contains("unknown HTML theme `nope`"));
    }

    #[test]
    fn user_theme_directory_shadows_builtin() {
        let dir = tempfile::tempdir().unwrap();
        write_theme(
            dir.path(),
            "pico",
            "<custom-shell>{{ doc.body }}</custom-shell>",
        );

        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some("pico"),
            dir.path(),
            &HtmlSyntaxTheme::builtin(),
        )
        .unwrap();

        assert!(themed.contains("<custom-shell>"));
        assert!(themed.contains("<h1>Standard Title</h1>"));
        // The built-in pico theme is not used when a user theme shadows it.
        assert!(!themed.contains("cdn.jsdelivr.net/npm/@picocss/pico"));
    }

    #[test]
    fn user_theme_loops_styles_scripts_and_includes_partials() {
        let dir = tempfile::tempdir().unwrap();
        let theme_dir = dir.path().join("mini");
        std::fs::create_dir_all(theme_dir.join("partials")).unwrap();
        std::fs::create_dir_all(theme_dir.join("styles")).unwrap();
        std::fs::create_dir_all(theme_dir.join("scripts")).unwrap();
        std::fs::write(
            theme_dir.join("layout.html"),
            "{{ doc.head }}{% for s in styles %}<style>{{ s.css }}</style>{% endfor %}{{ doc.body_open }}{% include \"partials/banner.html\" %}{{ doc.body }}{% for s in scripts %}<script>{{ s.content }}</script>{% endfor %}{{ doc.body_close }}",
        )
        .unwrap();
        std::fs::write(
            theme_dir.join("partials/banner.html"),
            "<header>hi</header>",
        )
        .unwrap();
        std::fs::write(theme_dir.join("styles/main.css"), "body{color:red}").unwrap();
        std::fs::write(theme_dir.join("scripts/main.js"), "console.log(1)").unwrap();

        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some("mini"),
            dir.path(),
            &HtmlSyntaxTheme::builtin(),
        )
        .unwrap();

        assert!(themed.contains("<header>hi</header>"));
        assert!(themed.contains("<style>body{color:red}</style>"));
        assert!(themed.contains("<script>console.log(1)</script>"));
    }

    #[test]
    fn theme_directory_missing_layout_errors() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("bare")).unwrap();

        let err = theme::apply_html_theme(
            SAMPLE_HTML,
            Some("bare"),
            dir.path(),
            &HtmlSyntaxTheme::builtin(),
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("bare"));
        assert!(err.contains("layout.html"));
    }

    #[test]
    fn theme_template_error_names_the_theme() {
        let dir = tempfile::tempdir().unwrap();
        write_theme(
            dir.path(),
            "broken",
            "{% include \"partials/missing.html\" %}",
        );

        let err = theme::apply_html_theme(
            SAMPLE_HTML,
            Some("broken"),
            dir.path(),
            &HtmlSyntaxTheme::builtin(),
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("broken"));
    }

    #[test]
    fn title_is_not_double_escaped() {
        let dir = tempfile::tempdir().unwrap();
        write_theme(
            dir.path(),
            "title-only",
            "<h1>{{ doc.title }}</h1>{{ doc.body_open }}{{ doc.body }}{{ doc.body_close }}",
        );
        let html = "<html><head><title>Foo &amp; Bar</title></head><body><p>x</p></body></html>";

        let themed = theme::apply_html_theme(
            html,
            Some("title-only"),
            dir.path(),
            &HtmlSyntaxTheme::builtin(),
        )
        .unwrap();

        assert!(themed.contains("<h1>Foo &amp; Bar</h1>"));
        assert!(!themed.contains("&amp;amp;"));
    }
}
