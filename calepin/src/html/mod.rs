mod assets;
mod minify;
mod syntax;
mod theme;

use anyhow::{Context, Result};
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

pub(crate) use minify::minify_html_file;
pub(crate) use syntax::HtmlSyntaxTheme;
#[cfg(test)]
pub(crate) use theme::theme_css;
pub(crate) use theme::{
    SiteContextInput, SiteLanguageEntry, SiteNavEntry, SiteNavSection, SitePagefindEntry,
};

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

    #[test]
    fn minify_leaves_mathml_documents_unchanged() {
        let html = r#"<!doctype html><html><head></head><body><main>
<math display="block"><mi>𝒜︀</mi><mo>≔</mo><mrow><mo>{</mo><mrow><mi>𝑥</mi><mo>∈</mo><mi>ℝ</mi><mspace width="0.2222em"/><mo lspace="0em" rspace="0em">|</mo><mspace width="0.2222em"/><mi>𝑥</mi><mspace width="0.2222em"/><mtext>is natural</mtext></mrow><mo>}</mo></mrow></math>
<p>after math</p></main><aside>toc</aside></body></html>"#;

        let minified = minify_html(html);

        assert_eq!(minified, html);
        assert_eq!(minified.matches("<math").count(), 1, "{minified}");
        assert_eq!(minified.matches("</math>").count(), 1, "{minified}");
        assert_eq!(minified.matches("<mrow").count(), 2, "{minified}");
        assert_eq!(minified.matches("</mrow>").count(), 2, "{minified}");
        assert!(minified.contains("</main><aside>"), "{minified}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{resolve_html_entry, HtmlEntry, HtmlScope, ThemeSelection};
    use std::collections::BTreeMap;

    const SAMPLE_HTML: &str = "<html><head><title>Standard Title</title></head><body><h1>Standard Title</h1></body></html>";

    fn entry_for(selection: &ThemeSelection, scope: HtmlScope) -> HtmlEntry {
        resolve_html_entry(selection, scope).unwrap().unwrap()
    }

    fn apply_html_theme(html: &str, entry: Option<&HtmlEntry>) -> Result<String> {
        theme::apply_html_theme(html, entry, &HtmlSyntaxTheme::builtin(), None, None, None)
    }

    fn theme_stylesheet(entry: &HtmlEntry, syntax_theme: &HtmlSyntaxTheme) -> String {
        entry
            .styles
            .iter()
            .map(|(_, css)| theme_css(css, syntax_theme))
            .filter(|css| !css.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn theme_scripts(entry: &HtmlEntry) -> String {
        entry
            .scripts
            .iter()
            .map(|(_, script)| script.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Writes a minimal theme bundle directory holding one entry file.
    fn write_theme(parent: &Path, name: &str, entry_file: &str, layout: &str) -> PathBuf {
        let dir = parent.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(entry_file);
        let manifest_path = dir.join("theme.toml");
        if !manifest_path.exists() {
            std::fs::write(manifest_path, "extends = \"typst\"\n").unwrap();
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, layout).unwrap();
        dir
    }

    fn dir_document_entry(dir: &Path) -> HtmlEntry {
        entry_for(&ThemeSelection::Dir(dir.to_path_buf()), HtmlScope::Document)
    }

    fn dir_site_entry(dir: &Path) -> HtmlEntry {
        entry_for(&ThemeSelection::Dir(dir.to_path_buf()), HtmlScope::Site)
    }

    fn root_custom_property(css: &str, theme: &str, property: &str) -> Option<String> {
        let mut winner: Option<((usize, usize, usize), usize, String)> = None;
        for (order, (selectors, declarations)) in top_level_css_rules(css).into_iter().enumerate() {
            let Some(value) = declaration_value(declarations, property) else {
                continue;
            };
            for selector in selectors.split(',').map(str::trim) {
                if !selector_matches_root_theme(selector, theme) {
                    continue;
                }
                let specificity = selector_specificity(selector);
                if winner.as_ref().is_none_or(|(current, current_order, _)| {
                    (specificity, order) >= (*current, *current_order)
                }) {
                    winner = Some((specificity, order, value.clone()));
                }
            }
        }
        winner.map(|(_, _, value)| value)
    }

    fn top_level_css_rules(css: &str) -> Vec<(&str, &str)> {
        let mut rules = Vec::new();
        let bytes = css.as_bytes();
        let mut selector_start = 0;
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b';' && css[selector_start..i].trim_start().starts_with('@') {
                selector_start = i + 1;
                i += 1;
                continue;
            }
            if bytes[i] != b'{' {
                i += 1;
                continue;
            }
            let selector = css[selector_start..i].trim();
            let body_start = i + 1;
            let mut depth = 1;
            let mut j = body_start;
            while j < bytes.len() && depth > 0 {
                match bytes[j] {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    _ => {}
                }
                j += 1;
            }
            if depth == 0 && !selector.starts_with('@') {
                rules.push((selector, css[body_start..j - 1].trim()));
            }
            selector_start = j;
            i = j;
        }
        rules
    }

    fn declaration_value(declarations: &str, property: &str) -> Option<String> {
        declarations.split(';').find_map(|declaration| {
            let (name, value) = declaration.split_once(':')?;
            (name.trim() == property).then(|| value.trim().to_string())
        })
    }

    fn element_property_with_inline(
        css: &str,
        selector_to_match: &str,
        property: &str,
        inline_value: &str,
    ) -> String {
        let mut winner = (false, (1, 0, 0, 0), 0, inline_value.trim().to_string());
        for (order, (selectors, declarations)) in top_level_css_rules(css).into_iter().enumerate() {
            let Some((value, important)) = declaration_value_with_priority(declarations, property)
            else {
                continue;
            };
            for selector in selectors.split(',').map(str::trim) {
                if selector != selector_to_match {
                    continue;
                }
                let candidate = (
                    important,
                    selector_specificity_for_element(selector),
                    order + 1,
                    value.clone(),
                );
                if (candidate.0, candidate.1, candidate.2) >= (winner.0, winner.1, winner.2) {
                    winner = candidate;
                }
            }
        }
        winner.3
    }

    fn declaration_value_with_priority(
        declarations: &str,
        property: &str,
    ) -> Option<(String, bool)> {
        declarations.split(';').find_map(|declaration| {
            let (name, value) = declaration.split_once(':')?;
            if name.trim() != property {
                return None;
            }
            let value = value.trim();
            let Some(stripped) = value.strip_suffix("!important") else {
                return Some((value.to_string(), false));
            };
            Some((stripped.trim().to_string(), true))
        })
    }

    fn selector_matches_root_theme(selector: &str, theme: &str) -> bool {
        selector == ":root"
            || selector == format!("html[data-theme=\"{theme}\"]")
            || selector == format!("html[data-theme='{theme}']")
    }

    fn selector_specificity(selector: &str) -> (usize, usize, usize) {
        match selector {
            ":root" => (0, 1, 0),
            selector if selector.starts_with("html[data-theme=") => (0, 1, 1),
            _ => (0, 0, 0),
        }
    }

    fn selector_specificity_for_element(selector: &str) -> (usize, usize, usize, usize) {
        let class_count = selector.matches('.').count();
        let element_count = selector
            .split(|ch: char| !ch.is_ascii_alphabetic())
            .filter(|part| !part.is_empty() && !part.starts_with('.'))
            .count();
        (0, 0, class_count, element_count)
    }

    #[test]
    fn default_document_theme_preserves_title_and_wraps_body() {
        let entry = entry_for(&ThemeSelection::Default, HtmlScope::Document);
        let themed = apply_html_theme(SAMPLE_HTML, Some(&entry)).unwrap();

        assert!(themed.contains("<title>Standard Title</title>"));
        assert!(!themed.contains("https://cdn.jsdelivr.net/npm/@picocss/pico@2/css/pico.min.css"));
        assert!(themed.contains("<main class=\"container calepin-content calepin-document-main\">"));
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
    fn default_document_theme_constrains_text_width() {
        let entry = entry_for(&ThemeSelection::Default, HtmlScope::Document);
        let css = theme_stylesheet(&entry, &HtmlSyntaxTheme::builtin());

        assert!(
            css.contains(
                r#".calepin-document-main {
  width: min(100% - 2rem, var(--calepin-content-width));"#
            ),
            "default document pages should keep text at reading width"
        );
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
            "layouts/document.html",
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
            "layouts/document.html",
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
            "layouts/site.html",
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
        // Typst emits a fragment (no <html>/<head>/<body>) for normal document
        // content. The theme must still apply.
        let fragment = "<h1>Hello</h1><p>World</p>";
        let entry = entry_for(&ThemeSelection::Default, HtmlScope::Document);
        let themed = apply_html_theme(fragment, Some(&entry)).unwrap();

        assert!(!themed.contains("https://cdn.jsdelivr.net/npm/@picocss/pico@2/css/pico.min.css"));
        assert!(themed.contains("<main class=\"container calepin-content calepin-document-main\">"));
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
    fn user_theme_directory_can_be_referenced_directly() {
        let dir = tempfile::tempdir().unwrap();
        let theme_dir = write_theme(
            dir.path(),
            "custom",
            "layouts/document.html",
            "<custom-shell>{{ doc.body }}</custom-shell>",
        );

        let themed = apply_html_theme(SAMPLE_HTML, Some(&dir_document_entry(&theme_dir))).unwrap();

        assert!(themed.contains("<custom-shell>"));
    }

    #[test]
    fn user_theme_exposes_custom_vars() {
        let entry = HtmlEntry {
            theme_name: "vars".to_string(),
            layout: "{{ vars.course }} — {{ vars.semester }}".to_string(),
            partials: Vec::new(),
            styles: Vec::new(),
            scripts: Vec::new(),
        };
        let site_context = SiteContextInput {
            vars: serde_json::json!({
                "course": "Econ 101",
                "semester": "Fall 2026",
            }),
            ..SiteContextInput::default()
        };

        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some(&entry),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            Some(&site_context),
        )
        .unwrap();

        assert_eq!(themed, "Econ 101 — Fall 2026");
    }

    #[test]
    fn user_theme_loops_css_js_and_includes_partials() {
        let dir = tempfile::tempdir().unwrap();
        let theme_dir = dir.path().join("mini");
        std::fs::create_dir_all(theme_dir.join("layouts")).unwrap();
        std::fs::create_dir_all(theme_dir.join("partials")).unwrap();
        std::fs::create_dir_all(theme_dir.join("css")).unwrap();
        std::fs::create_dir_all(theme_dir.join("js")).unwrap();
        std::fs::write(
            theme_dir.join("layouts/document.html"),
            "{{ doc.head }}{% for s in css %}<style>{{ s.content }}</style>{% endfor %}{{ doc.body_open }}{% include \"partials/banner.html\" %}{{ doc.body }}{% for s in js %}<script>{{ s.content }}</script>{% endfor %}{{ doc.body_close }}",
        )
        .unwrap();
        std::fs::write(
            theme_dir.join("partials/banner.html"),
            "<header>hi</header>",
        )
        .unwrap();
        std::fs::write(theme_dir.join("css/main.css"), "body{color:red}").unwrap();
        std::fs::write(theme_dir.join("js/main.js"), "console.log(1)").unwrap();

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
            "layouts/document.html",
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
            title: Some("Example".to_string()),
            logo: Some("assets/logo.svg".to_string()),
            logo_alt: Some("Example".to_string()),
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

        assert!(themed.contains(r#"<img src="assets/logo.svg" alt="Example""#));
        assert!(themed.contains(r#"aria-label="Example home""#));
        assert!(!themed.contains("logo_short.svg"));
        assert!(!themed.contains("Calepin home"));
    }

    #[test]
    fn bundled_themes_render_menu_links_theme_toggle_and_language_picker() {
        let site_context = SiteContextInput {
            menus: BTreeMap::from([(
                "main".to_string(),
                vec![SiteNavEntry {
                    href: "about.html".to_string(),
                    label: "About".to_string(),
                    label_html: "About".to_string(),
                    active: true,
                }],
            )]),
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
            language: Some("en".to_string()),
            title: Some("Example".to_string()),
            home_url: Some("index.html".to_string()),
            ..SiteContextInput::default()
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
                "{theme_name}: missing menu page link"
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
    fn bundled_website_themes_place_pagefind_config_before_component_script() {
        let site_context = SiteContextInput {
            title: Some("Example".to_string()),
            home_url: Some("index.html".to_string()),
            pagefind: Some(SitePagefindEntry {
                css: "pagefind/pagefind-component-ui.css".to_string(),
                js: "pagefind/pagefind-component-ui.js".to_string(),
                bundle: "pagefind/".to_string(),
            }),
            ..SiteContextInput::default()
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

            let config = themed
                .find(r#"<pagefind-config bundle-path="pagefind/"></pagefind-config>"#)
                .unwrap();
            let normalizer = themed
                .find(r#"new URL(bundlePath, window.location.href)"#)
                .unwrap();
            let script = themed
                .find(r#"<script src="pagefind/pagefind-component-ui.js" type="module"></script>"#)
                .unwrap();
            let modal = themed.find("<pagefind-modal></pagefind-modal>").unwrap();

            assert!(
                config < normalizer && normalizer < script,
                "{theme_name}: Pagefind config and bundle path setup must precede component script"
            );
            assert!(
                script < modal,
                "{theme_name}: Pagefind modal should remain in the body"
            );
            assert_eq!(
                themed.matches("<pagefind-config").count(),
                1,
                "{theme_name}: duplicate Pagefind config"
            );
        }
    }

    #[test]
    fn default_website_theme_places_main_menu_on_right_before_social_menu() {
        let site_context = SiteContextInput {
            menus: BTreeMap::from([
                (
                    "main".to_string(),
                    vec![SiteNavEntry {
                        href: "docs.html".to_string(),
                        label: "Documentation".to_string(),
                        label_html: "Documentation".to_string(),
                        active: false,
                    }],
                ),
                (
                    "social".to_string(),
                    vec![SiteNavEntry {
                        href: "https://github.com/example/project".to_string(),
                        label: "GitHub".to_string(),
                        label_html: "GitHub".to_string(),
                        active: false,
                    }],
                ),
            ]),
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

        let brand = themed.find("calepin-site-brand").unwrap();
        let right_menu = themed.find("</ul>\n            <ul>").unwrap();
        let documentation = themed.find("Documentation").unwrap();
        let github = themed.find("GitHub").unwrap();

        assert!(brand < right_menu);
        assert!(right_menu < documentation);
        assert!(documentation < github);
    }

    #[test]
    fn bundled_themes_render_footer_menu_items() {
        let site_context = SiteContextInput {
            menus: BTreeMap::from([(
                "footer".to_string(),
                vec![
                    SiteNavEntry {
                        href: "about.html".to_string(),
                        label: "About".to_string(),
                        label_html: "About".to_string(),
                        active: false,
                    },
                    SiteNavEntry {
                        href: String::new(),
                        label: "© 2026 Example".to_string(),
                        label_html: "© 2026 Example".to_string(),
                        active: false,
                    },
                ],
            )]),
            title: Some("Example".to_string()),
            home_url: Some("index.html".to_string()),
            ..SiteContextInput::default()
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

            assert!(themed.contains("calepin-site-footer"), "{theme_name}");
            assert!(
                themed.contains(r#"<a href="about.html" aria-label="About">About</a>"#),
                "{theme_name}: missing footer link"
            );
            assert!(
                themed.contains(r#"<span>© 2026 Example</span>"#),
                "{theme_name}: missing footer text"
            );
        }
    }

    #[test]
    fn bundled_themes_render_default_footer_when_not_configured() {
        let site_context = SiteContextInput {
            title: Some("Example".to_string()),
            home_url: Some("index.html".to_string()),
            ..SiteContextInput::default()
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
                !themed.contains("Made with Calepin"),
                "{theme_name}: footer should not be auto-injected"
            );
            assert!(
                !themed.contains("© 2026"),
                "{theme_name}: footer should not include default copyright"
            );
            assert!(
                !themed.contains(r#"<footer class="calepin-site-footer""#),
                "{theme_name}: footer wrapper should not render without footer config"
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
    fn default_website_theme_allows_wider_landing_hero_headline() {
        let entry = entry_for(&ThemeSelection::Default, HtmlScope::Site);
        let css = theme_stylesheet(&entry, &HtmlSyntaxTheme::builtin());

        assert!(css.contains(
            r#".landing-hero h2 {
  max-width: 42rem;"#
        ));
        assert!(css.contains(r#"@media (max-width: 56rem)"#));
        assert!(css.contains(
            r#".landing-hero h2 {
    max-width: 34rem;"#
        ));
    }

    #[test]
    fn bundled_themes_hide_language_picker_for_single_language_sites() {
        let site_context = SiteContextInput {
            menus: BTreeMap::from([(
                "social".to_string(),
                vec![SiteNavEntry {
                    href: String::new(),
                    label: "Language".to_string(),
                    label_html: "Language".to_string(),
                    active: false,
                }],
            )]),
            languages: vec![SiteLanguageEntry {
                code: "en".to_string(),
                label: "English".to_string(),
                href: "index.html".to_string(),
                active: true,
            }],
            language: Some("en".to_string()),
            title: Some("Example".to_string()),
            home_url: Some("index.html".to_string()),
            ..SiteContextInput::default()
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
                !themed.contains(r#"aria-label="Language" data-calepin-language-picker"#),
                "{theme_name}: language picker should require multiple languages"
            );
        }
    }

    #[test]
    fn bundled_themes_render_borderless_language_picker_button() {
        for selection in [ThemeSelection::Default, ThemeSelection::Builtin("academic")] {
            let entry = entry_for(&selection, HtmlScope::Site);
            let theme_name = entry.theme_name.clone();
            let css = theme_stylesheet(&entry, &HtmlSyntaxTheme::builtin());
            let script = theme_scripts(&entry);

            assert!(
                css.contains(
                    r#".calepin-language-picker-button {
  display: inline-flex;"#
                ),
                "{theme_name}: missing language picker button styles"
            );
            assert!(
                css.contains("border: 0;"),
                "{theme_name}: language picker button should not have a border"
            );
            assert!(
                css.contains("background: transparent;"),
                "{theme_name}: language picker button should not use the primary button color"
            );
            assert!(
                css.contains("color: inherit;"),
                "{theme_name}: language picker button should inherit text color"
            );
            assert!(
                css.contains("text-decoration: none;"),
                "{theme_name}: themed links should not be underlined"
            );
            assert!(
                script.contains(r#"summary.className = "calepin-language-picker-button";"#),
                "{theme_name}: language picker button should not use outline classes"
            );
            assert!(
                !script.contains("calepin-language-picker-button outline secondary"),
                "{theme_name}: language picker button should not use outline classes"
            );
        }
    }

    #[test]
    fn bundled_themes_expose_public_light_and_dark_theme_tokens() {
        for selection in [ThemeSelection::Default, ThemeSelection::Builtin("academic")] {
            let entry = entry_for(&selection, HtmlScope::Site);
            let theme_name = entry.theme_name.clone();
            let css = theme_stylesheet(&entry, &HtmlSyntaxTheme::builtin());

            for token in [
                "--calepin-color-background",
                "--calepin-color-text",
                "--calepin-color-muted",
                "--calepin-color-border",
                "--calepin-color-accent",
                "--calepin-color-link",
                "--calepin-color-info",
                "--calepin-color-success",
                "--calepin-color-warning",
                "--calepin-color-danger",
                "--calepin-color-important",
                "--calepin-callout-note-color",
                "--calepin-callout-tip-color",
                "--calepin-callout-warning-color",
                "--calepin-callout-caution-color",
                "--calepin-callout-important-color",
                "--calepin-surface",
                "--calepin-surface-raised",
                "--calepin-surface-inset",
                "--calepin-font-body",
                "--calepin-font-size-aside",
                "--calepin-space-md",
                "--calepin-block-gap",
                "--calepin-margin-width",
                "--calepin-focus-ring",
            ] {
                assert!(css.contains(token), "{theme_name}: missing {token}");
            }

            assert!(
                css.contains("html[data-theme=\"dark\"] {\n  color-scheme: dark;"),
                "{theme_name}: missing explicit dark token block"
            );
            assert!(
                css.contains("@media (prefers-color-scheme: dark)")
                    && css.contains("html:not([data-theme])"),
                "{theme_name}: missing system dark token block"
            );
        }
    }

    #[test]
    fn academic_document_page_width_token_is_theme_invariant() {
        let entry = entry_for(&ThemeSelection::Builtin("academic"), HtmlScope::Document);
        let css = theme_stylesheet(&entry, &HtmlSyntaxTheme::builtin());

        let light = root_custom_property(&css, "light", "--calepin-page-width").unwrap();
        let dark = root_custom_property(&css, "dark", "--calepin-page-width").unwrap();

        assert_eq!(
            light, dark,
            "academic document margins should not change between explicit light and dark themes"
        );
        assert_eq!(
            light,
            "calc(var(--calepin-content-width) + var(--calepin-margin-gap) + var(--calepin-margin-width))"
        );
    }

    #[test]
    fn academic_document_reserves_margin_only_for_margin_content() {
        let entry = entry_for(&ThemeSelection::Builtin("academic"), HtmlScope::Document);
        let css = theme_stylesheet(&entry, &HtmlSyntaxTheme::builtin());

        assert!(
            css.contains(
                r#".academic-document-main {
  width: min(100% - 2rem, var(--calepin-content-width));"#
            ),
            "academic document should collapse to the text column by default"
        );
        assert!(
            css.contains(
                r#".academic-document-controls {
  display: flex;
  justify-content: flex-end;
  width: min(100% - 2rem, var(--calepin-content-width));"#
            ),
            "academic document controls should stay aligned to the text column"
        );
        assert!(
            css.contains(
                "body:has(.academic-document-main .calepin-sidenote) .academic-document-main"
            ) && css.contains(
                "body:has(.academic-document-main .calepin-sidefigure) .academic-document-main"
            ),
            "academic side content should reserve a real margin column"
        );
        assert!(
            css.contains(
                r#".calepin-sidefigure {
  margin: 0.4rem 0 1rem var(--calepin-margin-gap);"#
            ),
            "block side figures should sit in the reserved margin instead of using negative sidenote margins"
        );
    }

    #[test]
    fn academic_margin_content_stacks_before_it_can_overlap_text() {
        let entry = entry_for(&ThemeSelection::Builtin("academic"), HtmlScope::Document);
        let css = theme_stylesheet(&entry, &HtmlSyntaxTheme::builtin());

        assert!(
            css.contains("@media (max-width: calc(39rem + 16rem + 2rem + 2rem))"),
            "academic margin content should stack before the 16rem margin can overlap text"
        );
    }

    #[test]
    fn academic_document_caps_sized_figures_to_text_column() {
        let entry = entry_for(&ThemeSelection::Builtin("academic"), HtmlScope::Document);
        let css = theme_stylesheet(&entry, &HtmlSyntaxTheme::builtin());

        let max_width = element_property_with_inline(
            &css,
            ".academic-document-main > .calepin-figure-width",
            "max-width",
            "100%",
        );

        assert_eq!(max_width, "var(--calepin-content-width)");
    }

    #[test]
    fn academic_document_aligns_sized_figures_with_text_column() {
        let entry = entry_for(&ThemeSelection::Builtin("academic"), HtmlScope::Document);
        let css = theme_stylesheet(&entry, &HtmlSyntaxTheme::builtin());

        let margin_inline = element_property_with_inline(
            &css,
            ".academic-document-main > .calepin-figure-width",
            "margin-inline",
            "auto",
        );

        assert_eq!(margin_inline, "0 auto");
    }

    #[test]
    fn academic_site_theme_omits_toc_and_sidebar_nav_links() {
        let site_context = SiteContextInput {
            menus: BTreeMap::from([(
                "main".to_string(),
                vec![SiteNavEntry {
                    href: "about.html".to_string(),
                    label: "About".to_string(),
                    label_html: "About".to_string(),
                    active: false,
                }],
            )]),
            sidebar_sections: vec![sidebar_section("Guide", "guide.html", false)],
            title: Some("Example".to_string()),
            home_url: Some("index.html".to_string()),
            // Academic's built-in default is opt-in (off); the TOC behavior
            // itself is covered by toc_* tests, this test is about menu/sidebar
            // gating only.
            toc_depth: Some(0),
            ..SiteContextInput::default()
        };
        let html = "<html><head><title>Standard Title</title></head><body><h1>Standard Title</h1><h2>Section</h2></body></html>";
        let entry = entry_for(&ThemeSelection::Builtin("academic"), HtmlScope::Site);

        let themed = theme::apply_html_theme(
            html,
            Some(&entry),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            Some(&site_context),
        )
        .unwrap();

        assert!(themed.contains(r#"href="about.html" aria-label="About""#));
        assert!(!themed.contains(r#"href="guide.html" aria-label="Guide""#));
        assert!(!themed.contains("academic-toc"));
        assert!(!themed.contains("On this page"));
    }

    #[test]
    fn academic_site_theme_shows_toc_when_enabled() {
        let site_context = SiteContextInput {
            title: Some("Example".to_string()),
            home_url: Some("index.html".to_string()),
            toc_depth: Some(3),
            ..SiteContextInput::default()
        };
        let html = "<html><head><title>Standard Title</title></head><body><h1>Standard Title</h1><h2>Section</h2></body></html>";
        let entry = entry_for(&ThemeSelection::Builtin("academic"), HtmlScope::Site);

        let themed = theme::apply_html_theme(
            html,
            Some(&entry),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            Some(&site_context),
        )
        .unwrap();

        assert!(themed.contains("On this page"));
        assert!(themed.contains(r##"<li class="level-2"><a href="#section">Section</a></li>"##));
    }

    #[test]
    fn academic_site_theme_centers_text_and_inherits_shared_code_styles() {
        let entry = entry_for(&ThemeSelection::Builtin("academic"), HtmlScope::Site);
        let css = theme_stylesheet(&entry, &HtmlSyntaxTheme::builtin());

        assert!(css.contains(
            r#".academic-page {
  width: min(100% - 2rem, var(--calepin-content-width));"#
        ));
        assert!(css.contains("justify-content: space-evenly;"));
        assert!(css.contains(
            r#".academic-menu {
  display: contents;"#
        ));
        assert!(css.contains(
            r#".academic-nav {
  display: flex;"#
        ));
        assert!(css.contains("pre > code"));
        assert!(css.contains("calepin-copy-code"));
        assert!(!css.contains(".academic-main pre"));
        assert!(!css.contains(".academic-main code"));
        assert!(!css.contains(".academic-landing .landing-command-row code"));
        assert!(!css.contains(".academic-toc"));
    }

    #[test]
    fn academic_site_theme_does_not_rewire_footnotes() {
        let entry = entry_for(&ThemeSelection::Builtin("academic"), HtmlScope::Site);
        let script = theme_scripts(&entry);

        // Footnotes are left as native footnotes; the theme no longer moves
        // endnotes into the margin. Margin placement is an explicit author
        // choice via the sidenote/sidefigure elements instead.
        assert!(!script.contains(r#"section[role="doc-endnotes"]"#));
        assert!(!script.contains("enhanceFootnotes"));
        // The unrelated navigation toggle is retained.
        assert!(script.contains("academic-nav-toggle"));
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

    fn is_details_open(themed: &str, title: &str) -> bool {
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

        assert!(is_details_open(&themed, "Reference"));
        assert!(!is_details_open(&themed, "Guide"));
    }

    #[test]
    fn bundled_website_theme_renders_sidebar_subheadings_without_links() {
        let site_context = SiteContextInput {
            sidebar_fold: false,
            sidebar_sections: vec![SiteNavSection {
                title: Some("Reference".to_string()),
                active: true,
                items: vec![
                    SiteNavEntry {
                        href: String::new(),
                        label: "Language".to_string(),
                        label_html: "Language".to_string(),
                        active: false,
                    },
                    SiteNavEntry {
                        href: "syntax.html".to_string(),
                        label: "Syntax".to_string(),
                        label_html: "Syntax".to_string(),
                        active: true,
                    },
                ],
            }],
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

        assert!(themed.contains(r#"<li class="calepin-website-sidebar-subheading">"#));
        assert!(themed.contains(r#"<span>Language</span>"#));
        assert!(!themed.contains(r#"<a href="" aria-label="Language""#));
        assert!(themed.contains(r#"<a href="syntax.html" aria-label="Syntax""#));
    }

    #[test]
    fn bundled_website_theme_script_closes_other_sidebar_sections() {
        let entry = entry_for(&ThemeSelection::Default, HtmlScope::Site);
        let script = theme_scripts(&entry);

        assert!(script.contains("initSidebarSections"));
        assert!(script.contains(".calepin-website-sidebar-section"));
        assert!(script.contains(r#"removeAttribute("open")"#));
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
    fn bundled_website_theme_inlines_theme_css() {
        let entry = entry_for(&ThemeSelection::Default, HtmlScope::Site);
        let site_context = SiteContextInput {
            title: Some("Example".to_string()),
            home_url: Some("index.html".to_string()),
            ..SiteContextInput::default()
        };

        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some(&entry),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            Some(&site_context),
        )
        .unwrap();

        assert!(!themed.contains(r#"<link rel="stylesheet" href="../.calepin/10_pico.css">"#));
        assert!(themed.contains("<style>"));
        assert!(themed.contains("--calepin-syntax-border"));
        assert!(themed.contains("--calepin-topbar-height"));
    }

    #[test]
    fn bundled_website_theme_inlines_theme_js() {
        let entry = entry_for(&ThemeSelection::Default, HtmlScope::Site);
        let themed = theme::apply_html_theme(
            SAMPLE_HTML,
            Some(&entry),
            &HtmlSyntaxTheme::builtin(),
            None,
            None,
            None,
        )
        .unwrap();

        assert!(!themed.contains(r#"<script src="../.calepin/theme-toggle.js"></script>"#));
        assert!(themed.contains("data-calepin-theme-toggle], #calepin-theme-button"));
    }

    #[test]
    fn title_is_not_double_escaped() {
        let dir = tempfile::tempdir().unwrap();
        let theme_dir = write_theme(
            dir.path(),
            "title-only",
            "layouts/document.html",
            "<h1>{{ doc.title }}</h1>{{ doc.body_open }}{{ doc.body }}{{ doc.body_close }}",
        );
        let html = "<html><head><title>Foo &amp; Bar</title></head><body><p>x</p></body></html>";

        let themed = apply_html_theme(html, Some(&dir_document_entry(&theme_dir))).unwrap();

        assert!(themed.contains("<h1>Foo &amp; Bar</h1>"));
        assert!(!themed.contains("&amp;amp;"));
    }
}
