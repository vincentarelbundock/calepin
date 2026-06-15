mod assets;
mod minify;
mod syntax;
mod theme;

#[cfg(test)]
use anyhow::anyhow;
use anyhow::{Context, Result};
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

use syntax::HtmlSyntaxTheme;

pub(crate) use minify::minify_html_file;
pub(crate) use theme::{
    SiteContextInput, SiteLanguageEntry, SiteNavEntry, SiteNavSection, SitePagefindEntry,
};

#[cfg(test)]
const HTML_INPUT_LIGHT_THEME_PATH: &str = ".calepin/calepin-input-light.tmTheme";
#[cfg(test)]
const HTML_INPUT_LIGHT_THEME_REF: &str = "/.calepin/calepin-input-light.tmTheme";

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct PreparedHtmlTheme {
    pub(crate) syntax_theme: HtmlSyntaxTheme,
    pub(crate) raw_theme_input: Option<String>,
}

#[cfg(test)]
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

/// Read `path`, transform its contents, and write the result back only when it
/// changed (so `watch` does not retrigger the child `typst watch` on a no-op).
fn rewrite_file_in_place(
    path: &Path,
    transform: impl FnOnce(&str) -> Result<String>,
) -> Result<()> {
    let original = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let updated = transform(&original)?;
    if updated != original {
        std::fs::write(path, &updated)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

/// Concatenate non-empty CSS/JS chunks separated by a blank line, each
/// terminated by a newline. Returns `None` when there is nothing to join.
fn join_blocks(chunks: impl IntoIterator<Item = String>) -> Option<String> {
    let mut out = String::new();
    for chunk in chunks {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&chunk);
        if !chunk.ends_with('\n') {
            out.push('\n');
        }
    }
    (!out.is_empty()).then_some(out)
}

pub(crate) fn apply_html_theme_file(
    path: &Path,
    entry: Option<&crate::theme::HtmlEntry>,
    root: &Path,
    site_context: Option<&SiteContextInput>,
) -> Result<()> {
    apply_html_theme_file_with_site_context(
        path,
        entry,
        &HtmlSyntaxTheme::builtin(),
        root,
        site_context,
    )
}

pub(crate) fn apply_html_theme_file_with_site_context(
    path: &Path,
    entry: Option<&crate::theme::HtmlEntry>,
    syntax_theme: &HtmlSyntaxTheme,
    root: &Path,
    site_context: Option<&SiteContextInput>,
) -> Result<()> {
    rewrite_file_in_place(path, |source| {
        theme::apply_html_theme(
            source,
            entry,
            syntax_theme,
            Some(path),
            Some(root),
            site_context,
        )
    })
}

pub(crate) fn inline_html_images_file(path: &Path, root: &Path) -> Result<()> {
    let base_dir = path.parent().unwrap_or(root);
    rewrite_file_in_place(path, |source| {
        assets::inline_html_images(source, root, base_dir)
    })
}

pub(crate) fn html_theme_stylesheet(entry: &crate::theme::HtmlEntry) -> Result<Option<String>> {
    theme::theme_stylesheet(entry)
}

pub(crate) fn html_theme_script(entry: &crate::theme::HtmlEntry) -> Option<String> {
    join_blocks(entry.scripts.iter().map(|(_, content)| content.clone()))
}

#[cfg(test)]
pub(crate) fn write_html_theme_stylesheet(
    entry: &crate::theme::HtmlEntry,
    out_dir: &Path,
    rel_path: &Path,
) -> Result<bool> {
    let Some(css) = html_theme_stylesheet(entry)? else {
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

#[cfg(test)]
mod minify_tests {
    use super::minify::minify_html;

    #[test]
    fn minifies_html_css_and_js() {
        let html = r#"<!doctype html>
<html>
  <head>
    <style>
      body { color: red; }
    </style>
  </head>
  <body>
    <p>  Hello, world!  </p>
    <script>const value = 1 + 2;</script>
  </body>
</html>
"#;

        let minified = minify_html(html);

        assert!(minified.len() < html.len());
        assert!(minified.contains("<p>Hello, world!"));
        assert!(!minified.contains("\n  <body>"));
    }
}

#[cfg(test)]
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
    use crate::theme::{resolve_html_entry, HtmlEntry, HtmlScope, ThemeSelection};

    const SAMPLE_HTML: &str = "<html><head><title>Standard Title</title></head><body><h1>Standard Title</h1></body></html>";

    fn entry_for(selection: &ThemeSelection, scope: HtmlScope) -> HtmlEntry {
        resolve_html_entry(selection, scope).unwrap().unwrap()
    }

    fn apply_html_theme(html: &str, entry: Option<&HtmlEntry>) -> Result<String> {
        theme::apply_html_theme(html, entry, &HtmlSyntaxTheme::builtin(), None, None, None)
    }

    /// Writes a minimal theme bundle directory holding one entry file.
    fn write_theme(parent: &Path, name: &str, entry_file: &str, layout: &str) -> PathBuf {
        let dir = parent.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(entry_file), layout).unwrap();
        dir
    }

    fn dir_document_entry(dir: &Path) -> HtmlEntry {
        entry_for(&ThemeSelection::Dir(dir.to_path_buf()), HtmlScope::Document)
    }

    fn dir_site_entry(dir: &Path) -> HtmlEntry {
        entry_for(&ThemeSelection::Dir(dir.to_path_buf()), HtmlScope::Site)
    }

    #[test]
    fn default_document_theme_preserves_title_and_wraps_body() {
        let entry = entry_for(&ThemeSelection::Default, HtmlScope::Document);
        let themed = apply_html_theme(SAMPLE_HTML, Some(&entry)).unwrap();

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
    fn document_theme_inlines_substituted_snippet_css() {
        let entry = entry_for(&ThemeSelection::Default, HtmlScope::Document);
        let themed = apply_html_theme(SAMPLE_HTML, Some(&entry)).unwrap();

        assert!(themed.contains("--calepin-syntax-foreground"));
        assert!(!themed.contains("__CALEPIN_SYNTAX_LIGHT__"));
        assert!(!themed.contains("__CALEPIN_SYNTAX_DARK__"));
    }

    #[test]
    fn user_html_theme_cannot_include_removed_template_context() {
        let dir = tempfile::tempdir().unwrap();
        let theme_dir = write_theme(
            dir.path(),
            "with-removed-context",
            "document.html",
            r#"{{ doc.body_open }}<style>{{ removed_template_context.css.code }}</style><main>{{ doc.body }}</main>{{ doc.body_close }}"#,
        );

        let result = apply_html_theme(SAMPLE_HTML, Some(&dir_document_entry(&theme_dir)));

        assert!(result.is_err());
    }

    #[test]
    fn user_html_theme_gets_docs_navigation_and_toc_context() {
        let dir = tempfile::tempdir().unwrap();
        let docs = dir.path().join("docs-src2");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("index.typ"), "").unwrap();
        std::fs::write(docs.join("cli.typ"), "").unwrap();
        let theme_dir = write_theme(
            dir.path(),
            "zensical",
            "document.html",
            r#"{{ doc.body_open }}<aside>{% for item in site.sidebar %}<a href="{{ item.href }}"{% if item.active %} aria-current="page"{% endif %}>{{ item.label }}</a>{% endfor %}</aside><nav>{% for item in site.toc %}<a href="{{ item.href }}">{{ item.label }}</a>{% endfor %}</nav><main>{{ doc.body }}</main>{{ doc.body_close }}"#,
        );
        let output = dir.path().join("docs-src2-build/html/cli.html");

        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some(&dir_document_entry(&theme_dir)),
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
    fn website_toc_skips_title_heading() {
        let dir = tempfile::tempdir().unwrap();
        let theme_dir = write_theme(
            dir.path(),
            "site-toc",
            "site.html",
            r#"{{ doc.body_open }}<nav>{% for item in site.toc %}<a href="{{ item.href }}" class="level-{{ item.level }}">{{ item.label }}</a>{% endfor %}</nav><main>{{ doc.body }}</main>{{ doc.body_close }}"#,
        );
        let html = "<html><head><title>Standard Title</title></head><body><h1>Standard Title</h1><h2>Section</h2></body></html>";

        let themed = theme::apply_html_theme(
            html,
            Some(&dir_site_entry(&theme_dir)),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            Some(&SiteContextInput::default()),
        )
        .unwrap();

        assert!(!themed.contains(r##"href="#standard-title""##));
        assert!(themed.contains(r##"href="#section" class="level-2">Section"##));
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
    fn default_document_theme_applies_to_bare_html_fragment() {
        // Typst emits a fragment (no <html>/<head>/<body>) for documents that
        // don't call #calepin.html[...].  The theme must still apply.
        let fragment = "<h1>Hello</h1><p>World</p>";
        let entry = entry_for(&ThemeSelection::Default, HtmlScope::Document);
        let themed = apply_html_theme(fragment, Some(&entry)).unwrap();

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
    fn prepare_html_theme_defaults_without_custom_syntax_theme() {
        let dir = tempfile::tempdir().unwrap();

        let prepared = prepare_html_theme(dir.path(), Some("html"), None, None, None).unwrap();

        assert!(prepared.raw_theme_input.is_none());
        assert!(prepared.syntax_theme.class_rules().contains("sourceCode"));
    }

    #[test]
    fn user_theme_directory_can_be_referenced_directly() {
        let dir = tempfile::tempdir().unwrap();
        let theme_dir = write_theme(
            dir.path(),
            "custom",
            "document.html",
            "<custom-shell>{{ doc.body }}</custom-shell>",
        );

        let themed = apply_html_theme(SAMPLE_HTML, Some(&dir_document_entry(&theme_dir))).unwrap();

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
            theme_dir.join("document.html"),
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

        let themed = apply_html_theme(SAMPLE_HTML, Some(&dir_document_entry(&theme_dir))).unwrap();

        assert!(themed.contains("<header>hi</header>"));
        assert!(themed.contains("<style>body{color:red}</style>"));
        assert!(themed.contains("<script>console.log(1)</script>"));
    }

    #[test]
    fn theme_template_error_names_the_theme() {
        let dir = tempfile::tempdir().unwrap();
        let theme_dir = write_theme(
            dir.path(),
            "broken",
            "document.html",
            "{% include \"partials/missing.html\" %}",
        );

        let err = apply_html_theme(SAMPLE_HTML, Some(&dir_document_entry(&theme_dir)))
            .unwrap_err()
            .to_string();

        assert!(err.contains("broken"));
    }

    #[test]
    fn bundled_website_theme_uses_configured_logo() {
        let site_context = SiteContextInput {
            sidebar: Vec::new(),
            sidebar_fold: false,
            sidebar_sections: Vec::new(),
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
            favicon: None,
            current_url: None,
            page_title: None,
            stylesheet: None,
            scripts: Vec::new(),
            pagefind: None,
        };
        let entry = entry_for(&ThemeSelection::Default, HtmlScope::Site);

        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some(&entry),
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
    fn bundled_themes_render_navbar_links_theme_toggle_and_language_picker() {
        let site_context = SiteContextInput {
            sidebar: Vec::new(),
            sidebar_fold: false,
            sidebar_sections: Vec::new(),
            navbar_left: vec![SiteNavEntry {
                href: "about.html".to_string(),
                label: "About".to_string(),
                label_html: "About".to_string(),
                active: true,
            }],
            navbar_center: Vec::new(),
            navbar_right: Vec::new(),
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
            favicon: None,
            current_url: None,
            page_title: None,
            stylesheet: None,
            scripts: Vec::new(),
            pagefind: None,
        };

        for selection in [ThemeSelection::Default, ThemeSelection::Builtin("academic")] {
            let entry = entry_for(&selection, HtmlScope::Site);
            let theme_name = entry.theme_name.clone();
            let themed = theme::apply_html_theme(
                SAMPLE_HTML,
                Some(&entry),
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
                "{theme_name}: missing theme toggle"
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
    fn default_website_theme_renders_output_picker_directly() {
        let site_context = SiteContextInput {
            title: Some("Example".to_string()),
            home_url: Some("index.html".to_string()),
            ..SiteContextInput::default()
        };
        let entry = entry_for(&ThemeSelection::Default, HtmlScope::Site);
        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some(&entry),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            Some(&site_context),
        )
        .unwrap();

        assert!(themed.contains(r#"id="calepin-website-view-mode""#));
        assert!(themed.contains(r#"<option value="source">Source</option>"#));
    }

    #[test]
    fn bundled_themes_hide_language_picker_for_single_language_sites() {
        let site_context = SiteContextInput {
            sidebar: Vec::new(),
            sidebar_fold: false,
            sidebar_sections: Vec::new(),
            navbar_left: Vec::new(),
            navbar_center: Vec::new(),
            navbar_right: vec![SiteNavEntry {
                href: String::new(),
                label: "Language".to_string(),
                label_html: "Language".to_string(),
                active: false,
            }],
            languages: vec![SiteLanguageEntry {
                code: "en".to_string(),
                label: "English".to_string(),
                href: "index.html".to_string(),
                active: true,
            }],
            translations: Vec::new(),
            language: Some("en".to_string()),
            title: Some("Example".to_string()),
            description: None,
            base_url: None,
            logo: None,
            logo_alt: None,
            home_url: Some("index.html".to_string()),
            favicon: None,
            current_url: None,
            page_title: None,
            stylesheet: None,
            scripts: Vec::new(),
            pagefind: None,
        };

        for selection in [ThemeSelection::Default, ThemeSelection::Builtin("academic")] {
            let entry = entry_for(&selection, HtmlScope::Site);
            let theme_name = entry.theme_name.clone();
            let themed = theme::apply_html_theme(
                SAMPLE_HTML,
                Some(&entry),
                &HtmlSyntaxTheme::builtin(),
                None,
                None,
                Some(&site_context),
            )
            .unwrap();

            assert!(
                !themed.contains(r#"<select class="calepin-website-view-mode" aria-label="Language" data-calepin-language-picker"#)
                    && !themed.contains(r#"<select class="academic-language-picker" aria-label="Language" data-calepin-language-picker"#),
                "{theme_name}: language picker should require multiple languages"
            );
        }
    }

    fn sidebar_section(title: &str, href: &str, active: bool) -> SiteNavSection {
        SiteNavSection {
            title: Some(title.to_string()),
            active,
            items: vec![SiteNavEntry {
                href: href.to_string(),
                label: title.to_string(),
                label_html: title.to_string(),
                active,
            }],
        }
    }

    fn details_is_open(themed: &str, title: &str) -> bool {
        let summary = themed
            .find(&format!("<summary>{title}</summary>"))
            .unwrap_or_else(|| panic!("no foldable section titled {title}"));
        let details = themed[..summary]
            .rfind("<details")
            .expect("summary outside <details>");
        themed[details..summary].contains(" open")
    }

    #[test]
    fn bundled_website_theme_folds_inactive_sidebar_sections() {
        let site_context = SiteContextInput {
            sidebar_fold: true,
            sidebar_sections: vec![
                sidebar_section("Guide", "guide.html", false),
                sidebar_section("Reference", "reference.html", true),
            ],
            ..Default::default()
        };
        let entry = entry_for(&ThemeSelection::Default, HtmlScope::Site);

        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some(&entry),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            Some(&site_context),
        )
        .unwrap();

        assert!(details_is_open(&themed, "Reference"));
        assert!(!details_is_open(&themed, "Guide"));
    }

    #[test]
    fn bundled_website_theme_keeps_sections_unfolded_when_fold_disabled() {
        let site_context = SiteContextInput {
            sidebar_fold: false,
            sidebar_sections: vec![
                sidebar_section("Guide", "guide.html", false),
                sidebar_section("Reference", "reference.html", true),
            ],
            ..Default::default()
        };
        let entry = entry_for(&ThemeSelection::Default, HtmlScope::Site);

        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some(&entry),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            Some(&site_context),
        )
        .unwrap();

        assert!(!themed.contains("<summary>Guide</summary>"));
        assert!(!themed.contains("<summary>Reference</summary>"));
        assert!(themed.contains("<p><strong>Guide</strong></p>"));
        assert!(themed.contains("<p><strong>Reference</strong></p>"));
    }

    #[test]
    fn bundled_website_theme_can_link_external_stylesheet() {
        let site_context = SiteContextInput {
            sidebar: Vec::new(),
            sidebar_fold: false,
            sidebar_sections: Vec::new(),
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
            favicon: None,
            current_url: None,
            page_title: None,
            stylesheet: Some("../.calepin/calepin-website.css".to_string()),
            scripts: Vec::new(),
            pagefind: None,
        };
        let entry = entry_for(&ThemeSelection::Default, HtmlScope::Site);

        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some(&entry),
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
    fn bundled_website_theme_can_link_external_scripts() {
        let site_context = SiteContextInput {
            title: Some("Example".to_string()),
            home_url: Some("index.html".to_string()),
            scripts: vec!["../.calepin/calepin-website.0123456789abcdef.js".to_string()],
            ..SiteContextInput::default()
        };
        let entry = entry_for(&ThemeSelection::Default, HtmlScope::Site);

        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some(&entry),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            Some(&site_context),
        )
        .unwrap();

        assert!(themed.contains(
            r#"<script src="../.calepin/calepin-website.0123456789abcdef.js"></script>"#
        ));
        assert!(!themed.contains("data-calepin-theme-toggle], #calepin-theme-button"));
    }

    #[test]
    fn writes_bundled_website_stylesheet() {
        let dir = tempfile::tempdir().unwrap();
        let rel = Path::new(".calepin/calepin-website.css");
        let entry = entry_for(&ThemeSelection::Default, HtmlScope::Site);

        let wrote = write_html_theme_stylesheet(&entry, dir.path(), rel).unwrap();

        assert!(wrote);
        let css = std::fs::read_to_string(dir.path().join(rel)).unwrap();
        assert!(css.contains(".cell-output,"));
        assert!(css.contains(".calepin-website-shell"));
        assert!(css.contains("--calepin-syntax-foreground"));
    }

    #[test]
    fn title_is_not_double_escaped() {
        let dir = tempfile::tempdir().unwrap();
        let theme_dir = write_theme(
            dir.path(),
            "title-only",
            "document.html",
            "<h1>{{ doc.title }}</h1>{{ doc.body_open }}{{ doc.body }}{{ doc.body_close }}",
        );
        let html = "<html><head><title>Foo &amp; Bar</title></head><body><p>x</p></body></html>";

        let themed = apply_html_theme(html, Some(&dir_document_entry(&theme_dir))).unwrap();

        assert!(themed.contains("<h1>Foo &amp; Bar</h1>"));
        assert!(!themed.contains("&amp;amp;"));
    }
}
