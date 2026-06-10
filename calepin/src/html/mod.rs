mod assets;
mod syntax;
mod theme;

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

use syntax::HtmlSyntaxTheme;

pub(crate) use theme::{is_builtin_html_theme, is_theme_path_like};
pub(crate) use theme::{
    SiteContextInput, SiteLanguageEntry, SiteNavEntry, SiteNavSection,
};

const HTML_INPUT_LIGHT_THEME_PATH: &str = ".calepin/calepin-input-light.tmTheme";
const HTML_INPUT_LIGHT_THEME_REF: &str = "/.calepin/calepin-input-light.tmTheme";
pub(crate) const DEFAULT_HTML_THEME: &str = "calepin-html";

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
    syntax_theme: &HtmlSyntaxTheme,
    root: &Path,
) -> Result<()> {
    apply_html_theme_file_with_site_context(path, html_theme, syntax_theme, root, None)
}

pub(crate) fn apply_html_theme_file_with_site_context(
    path: &Path,
    html_theme: Option<&str>,
    syntax_theme: &HtmlSyntaxTheme,
    root: &Path,
    site_context: Option<&SiteContextInput>,
) -> Result<()> {
    let html = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let themed = theme::apply_html_theme(
        &html,
        html_theme,
        syntax_theme,
        Some(path),
        Some(root),
        site_context,
    )?;
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

pub(crate) fn write_html_theme_stylesheet(
    html_theme: &str,
    out_dir: &Path,
    rel_path: &Path,
) -> Result<bool> {
    let Some(css) = theme::theme_stylesheet(html_theme)? else {
        return Ok(false);
    };
    let path = out_dir.join(rel_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(&path, css).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(true)
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

    fn apply_html_theme(html: &str, html_theme: Option<&str>) -> Result<String> {
        theme::apply_html_theme(
            html,
            html_theme,
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            None,
        )
    }

    fn write_theme(parent: &Path, name: &str, layout: &str) {
        let dir = parent.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("layout.html"), layout).unwrap();
    }

    #[test]
    fn calepin_html_theme_preserves_title_and_wraps_body() {
        let themed = apply_html_theme(SAMPLE_HTML, Some("calepin-html")).unwrap();

        assert!(themed.contains("<title>Standard Title</title>"));
        assert!(themed.contains("https://cdn.jsdelivr.net/npm/@picocss/pico@2/css/pico.min.css"));
        assert!(themed.contains("<main class=\"container\">"));
        assert!(themed.contains(".sourceCode,"));
        assert!(themed.contains(".cell-output,"));
        assert!(themed.contains("calepin-copy-code"));
        assert!(themed.contains(r#"<nav class="calepin-theme-switcher""#));
        assert!(themed.contains("data-calepin-theme-storage-key=\"calepin-html-theme\""));
        assert!(themed.contains("data-calepin-theme-toggle"));
        assert!(themed.contains("const order = [\"\", \"light\", \"dark\"]"));
        assert!(themed.contains(r#"<h1 id="standard-title">Standard Title</h1>"#));
    }

    #[test]
    fn user_html_theme_can_include_builtin_snippets() {
        let dir = tempfile::tempdir().unwrap();
        write_theme(
            dir.path(),
            "with-snippets",
            r#"{{ doc.body_open }}<style>{{ snippets.css.code }}</style><main>{{ doc.body }}</main><script>{{ snippets.js.copy_code }}</script><script type="text/plain">{{ snippets.typst.code_block }}</script>{{ doc.body_close }}"#,
        );
        let theme_path = dir.path().join("with-snippets");
        let theme_path = theme_path.to_string_lossy();

        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some(&theme_path),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            None,
        )
        .unwrap();

        assert!(themed.contains(".sourceCode,"));
        assert!(themed.contains("window.CalepinCopyCode"));
        assert!(themed.contains("#let code-block("));
    }

    #[test]
    fn user_html_theme_gets_docs_navigation_and_toc_context() {
        let dir = tempfile::tempdir().unwrap();
        let docs = dir.path().join("docs-src2");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("index.typ"), "").unwrap();
        std::fs::write(docs.join("cli.typ"), "").unwrap();
        write_theme(
            dir.path(),
            "zensical",
            r#"{{ doc.body_open }}<aside>{% for item in site.nav %}<a href="{{ item.href }}"{% if item.active %} aria-current="page"{% endif %}>{{ item.label }}</a>{% endfor %}</aside><nav>{% for item in site.toc %}<a href="{{ item.href }}">{{ item.label }}</a>{% endfor %}</nav><main>{{ doc.body }}</main>{{ doc.body_close }}"#,
        );
        let output = dir.path().join("docs-src2-build/html/cli.html");
        let theme_path = dir.path().join("zensical");
        let theme_path = theme_path.to_string_lossy();

        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some(&theme_path),
            &HtmlSyntaxTheme::builtin(),
            Some(&output),
            Some(dir.path()),
            None,
        )
        .unwrap();

        assert!(themed.contains("index.html"));
        assert!(themed.contains("cli.html"));
        assert!(themed.contains("cli.typ"));
        assert!(themed.contains(r#"aria-current="page""#));
        assert!(themed.contains("href=\"#standard-title\""));
        assert!(themed.contains(r#"<h1 id="standard-title">Standard Title</h1>"#));
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
    fn calepin_html_theme_applies_to_bare_html_fragment() {
        // Typst emits a fragment (no <html>/<head>/<body>) for documents that
        // don't call #calepin.html[...].  The theme must still apply.
        let fragment = "<h1>Hello</h1><p>World</p>";
        let themed = apply_html_theme(fragment, Some("calepin-html")).unwrap();

        assert!(themed.contains("https://cdn.jsdelivr.net/npm/@picocss/pico@2/css/pico.min.css"));
        assert!(themed.contains("<main class=\"container\">"));
        assert!(themed.contains(r#"<h1 id="hello">Hello</h1>"#));
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
    fn builtin_theme_names_resolve_to_builtin_theme() {
        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some("calepin-html"),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            None,
        )
        .unwrap();

        assert!(themed.contains("<main class=\"container\">"));
        assert!(themed.contains(r#"<h1 id="standard-title">Standard Title</h1>"#));
    }

    #[test]
    fn user_theme_directory_can_be_referenced_directly() {
        let dir = tempfile::tempdir().unwrap();
        write_theme(
            dir.path(),
            "calepin-html",
            "<custom-shell>{{ doc.body }}</custom-shell>",
        );
        let theme_path = dir.path().join("calepin-html");
        let theme_path = theme_path.to_string_lossy();

        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some(&theme_path),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            None,
        )
        .unwrap();

        assert!(themed.contains("<custom-shell>"));
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
        let theme_path = theme_dir.to_string_lossy();

        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some(&theme_path),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            None,
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
        let theme_path = dir.path().join("bare");
        let theme_path = theme_path.to_string_lossy();

        let err = theme::apply_html_theme(
            SAMPLE_HTML,
            Some(&theme_path),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            None,
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
        let theme_path = dir.path().join("broken");
        let theme_path = theme_path.to_string_lossy();

        let err = theme::apply_html_theme(
            SAMPLE_HTML,
            Some(&theme_path),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            None,
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("broken"));
    }

    #[test]
    fn bundled_website_theme_uses_configured_logo() {
        let site_context = SiteContextInput {
            nav: Vec::new(),
            nav_sections: Vec::new(),
            navbar_left: Vec::new(),
            navbar_center: Vec::new(),
            navbar_right: Vec::new(),
            languages: Vec::new(),
            translations: Vec::new(),
            language: None,
            title: Some("Example".to_string()),
            description: None,
            base_url: None,
            logo: Some("assets/logo.svg".to_string()),
            logo_alt: Some("Example".to_string()),
            home_url: Some("index.html".to_string()),
            github_url: None,
            current_url: None,
            page_title: None,
            stylesheet: None,
        };

        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some("calepin-website"),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            Some(&site_context),
        )
        .unwrap();

        assert!(themed.contains(r#"<img src="assets/logo.svg" alt="Example""#));
        assert!(themed.contains(r#"aria-label="Example home""#));
        assert!(!themed.contains("logo_short_2.svg"));
        assert!(!themed.contains("Calepin home"));
    }

    #[test]
    fn bundled_themes_render_navbar_widgets_links_and_language_picker() {
        let site_context = SiteContextInput {
            nav: Vec::new(),
            nav_sections: Vec::new(),
            navbar_left: vec![SiteNavEntry {
                href: "about.html".to_string(),
                label: "About".to_string(),
                label_html: "About".to_string(),
                widget: None,
                active: true,
            }],
            navbar_center: Vec::new(),
            navbar_right: vec![
                SiteNavEntry {
                    href: String::new(),
                    label: "Theme".to_string(),
                    label_html: "Theme".to_string(),
                    widget: Some("theme".to_string()),
                    active: false,
                },
                SiteNavEntry {
                    href: String::new(),
                    label: "Language".to_string(),
                    label_html: "Language".to_string(),
                    widget: Some("language".to_string()),
                    active: false,
                },
            ],
            languages: vec![
                SiteLanguageEntry {
                    code: "en".to_string(),
                    label: "English".to_string(),
                    href: "index.html".to_string(),
                    active: true,
                },
                SiteLanguageEntry {
                    code: "fr".to_string(),
                    label: "Français".to_string(),
                    href: "fr/index.html".to_string(),
                    active: false,
                },
            ],
            translations: Vec::new(),
            language: Some("en".to_string()),
            title: Some("Example".to_string()),
            description: None,
            base_url: None,
            logo: None,
            logo_alt: None,
            home_url: Some("index.html".to_string()),
            github_url: None,
            current_url: None,
            page_title: None,
            stylesheet: None,
        };

        for theme_name in ["calepin-website", "academic"] {
            let themed = theme::apply_html_theme(
                SAMPLE_HTML,
                Some(theme_name),
                &HtmlSyntaxTheme::builtin(),
                None,
                None,
                Some(&site_context),
            )
            .unwrap();

            assert!(
                themed.contains(r#"href="about.html" aria-label="About""#),
                "{theme_name}: missing navbar page link"
            );
            assert!(
                themed.contains("data-calepin-theme-toggle"),
                "{theme_name}: missing theme widget"
            );
            assert!(
                themed.contains("data-calepin-language-picker"),
                "{theme_name}: missing language picker"
            );
            assert!(
                themed.contains(r#"<option value="fr/index.html" data-calepin-language-code="fr""#),
                "{theme_name}: missing language option"
            );
        }
    }

    #[test]
    fn bundled_website_theme_can_link_external_stylesheet() {
        let site_context = SiteContextInput {
            nav: Vec::new(),
            nav_sections: Vec::new(),
            navbar_left: Vec::new(),
            navbar_center: Vec::new(),
            navbar_right: Vec::new(),
            languages: Vec::new(),
            translations: Vec::new(),
            language: None,
            title: Some("Example".to_string()),
            description: None,
            base_url: None,
            logo: None,
            logo_alt: None,
            home_url: Some("index.html".to_string()),
            github_url: None,
            current_url: None,
            page_title: None,
            stylesheet: Some("../.calepin/calepin-website.css".to_string()),
        };

        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some("calepin-website"),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            Some(&site_context),
        )
        .unwrap();

        assert!(
            themed.contains(r#"<link rel="stylesheet" href="../.calepin/calepin-website.css">"#)
        );
        assert!(!themed.contains("--calepin-code-border"));
        assert!(!themed.contains("--calepin-topbar-height"));
    }

    #[test]
    fn writes_bundled_website_stylesheet() {
        let dir = tempfile::tempdir().unwrap();
        let rel = Path::new(".calepin/calepin-website.css");

        let wrote = write_html_theme_stylesheet("calepin-website", dir.path(), rel).unwrap();

        assert!(wrote);
        let css = std::fs::read_to_string(dir.path().join(rel)).unwrap();
        assert!(css.contains(".cell-output,"));
        assert!(css.contains(".calepin-website-shell"));
        assert!(css.contains("--calepin-syntax-foreground"));
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
        let theme_path = dir.path().join("title-only");
        let theme_path = theme_path.to_string_lossy();

        let themed = theme::apply_html_theme(
            html,
            Some(&theme_path),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            None,
        )
        .unwrap();

        assert!(themed.contains("<h1>Foo &amp; Bar</h1>"));
        assert!(!themed.contains("&amp;amp;"));
    }
}
