//! Theme bundles. A theme is a directory named by family; the target and
//! scope dimensions are well-known filenames inside it: `layouts/pdf.typ`,
//! `layouts/document.html`, `layouts/site.html`. See specs/2026-06-10-theme-bundles-design.md.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

mod bundle;
mod html;
mod notebook;

pub use bundle::{builtin_names, eject_builtin_to};
pub use html::{resolve_explicit_site_html_entry, resolve_html_entry, HtmlEntry, HtmlScope};
pub use notebook::{notebook_source, NotebookSource, NotebookTemplateContext};

pub const DEFAULT_THEME_NAME: &str = "calepin";

/// Where a render gets its theme from. `Default` means "no selection made",
/// which resolves to the builtin `calepin` bundle.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ThemeSelection {
    #[default]
    Default,
    /// `theme = "typst"`: raw Typst output, no Calepin theming for any target.
    Typst,
    Builtin(&'static str),
    Dir(PathBuf),
}

impl ThemeSelection {
    /// Parse a config or setup value. Relative path-like values resolve
    /// against `base_dir` (the config file's directory or document root).
    pub fn parse(value: &str, base_dir: &Path) -> Result<Self> {
        if value == "typst" {
            return Ok(Self::Typst);
        }
        if let Some(name) = builtin_names().into_iter().find(|name| *name == value) {
            return Ok(Self::Builtin(name));
        }
        if is_path_like(value) {
            let path = Path::new(value);
            let path = if path.is_absolute() {
                path.to_path_buf()
            } else {
                base_dir.join(path)
            };
            if !path.is_dir() {
                return Err(anyhow!("theme directory not found: {}", path.display()));
            }
            return Ok(Self::Dir(path));
        }
        Err(anyhow!(
            "unknown theme `{value}`; use `typst`, one of {}, or a path to a theme directory",
            builtin_names().join(", ")
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ThemeLayer {
    Builtin(&'static str),
    Dir(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThemeChain {
    /// Parent-most to child-most. Empty means terminal `typst` with no local layers.
    pub(crate) layers: Vec<ThemeLayer>,
    pub(crate) terminal_typst: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct LocalThemeManifest {
    pub(crate) extends: Option<String>,
    pub(crate) shared: LocalThemeSharedImports,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct LocalThemeSharedImports {
    pub(crate) partials: Vec<String>,
    pub(crate) css: Vec<String>,
    pub(crate) js: Vec<String>,
}

pub(crate) fn resolve_theme_chain(selection: &ThemeSelection) -> Result<ThemeChain> {
    match selection {
        ThemeSelection::Typst => Ok(ThemeChain {
            layers: Vec::new(),
            terminal_typst: true,
        }),
        ThemeSelection::Default => Ok(ThemeChain {
            layers: vec![ThemeLayer::Builtin(DEFAULT_THEME_NAME)],
            terminal_typst: false,
        }),
        ThemeSelection::Builtin(name) => Ok(ThemeChain {
            layers: vec![ThemeLayer::Builtin(name)],
            terminal_typst: false,
        }),
        ThemeSelection::Dir(dir) => {
            validate_theme_dir(dir)?;
            resolve_dir_theme_chain(dir)
        }
    }
}

fn resolve_dir_theme_chain(dir: &Path) -> Result<ThemeChain> {
    let manifest = read_local_theme_manifest(dir)?;
    let mut chain = match manifest.extends.as_deref() {
        None => {
            return Err(anyhow!(
                "local theme {} must declare `extends` in theme.toml; use `extends = \"typst\"` for a bare theme or a built-in theme: {}",
                dir.display(),
                bundle::builtin_names().join(", ")
            ))
        }
        Some("typst") => ThemeChain {
            layers: Vec::new(),
            terminal_typst: true,
        },
        Some(value) => match bundle::builtin_names()
            .into_iter()
            .find(|name| *name == value)
        {
            Some(name) => ThemeChain {
                layers: vec![ThemeLayer::Builtin(name)],
                terminal_typst: false,
            },
            None => {
                return Err(anyhow!(
                    "unknown theme `{value}` in extends; use `typst` or a built-in theme: {}",
                    bundle::builtin_names().join(", ")
                ))
            }
        },
    };
    chain.layers.push(ThemeLayer::Dir(dir.to_path_buf()));
    Ok(chain)
}

pub(crate) fn read_local_theme_manifest(dir: &Path) -> Result<LocalThemeManifest> {
    let path = dir.join("theme.toml");
    if !path.is_file() {
        return Ok(LocalThemeManifest::default());
    }
    let source = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&source).with_context(|| format!("failed to parse {}", path.display()))
}

/// Same heuristic the old html theme used: anything that looks like a path
/// (separator, leading dot, absolute) is treated as a directory reference.
fn is_path_like(value: &str) -> bool {
    let path = Path::new(value);
    path.is_absolute()
        || path.components().count() > 1
        || value.starts_with('.')
        || value.contains('\\')
        || value.ends_with('/')
}

fn validate_theme_dir(dir: &Path) -> Result<()> {
    let has_entry = dir.join("theme.toml").is_file()
        || [
            "layouts/pdf.typ",
            "notebook.typ.jinja",
            "paged.typ.jinja",
            "layouts/document.html",
            "layouts/site.html",
        ]
        .iter()
        .any(|file| dir.join(file).is_file());
    if !has_entry {
        return Err(anyhow!(
            "theme directory {} contains none of layouts/pdf.typ, layouts/document.html, layouts/site.html",
            dir.display()
        ));
    }
    Ok(())
}

fn dir_theme_name(dir: &Path) -> String {
    dir.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| dir.display().to_string())
}

/// Read every `*.<ext>` file in `dir`, sorted by filename for deterministic
/// order. A missing directory yields no files.
fn read_theme_files(dir: &Path, ext: &str) -> Result<Vec<(String, String)>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("failed to read {}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some(ext))
        .collect();
    paths.sort();

    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        files.push((name, contents));
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn parse_builtin_names() {
        let base = Path::new("/tmp");
        assert_eq!(
            ThemeSelection::parse("calepin", base).unwrap(),
            ThemeSelection::Builtin("calepin")
        );
        assert_eq!(
            ThemeSelection::parse("academic", base).unwrap(),
            ThemeSelection::Builtin("academic")
        );
    }

    #[test]
    fn parse_typst_selects_raw_typst_output() {
        let base = Path::new("/tmp");
        assert_eq!(
            ThemeSelection::parse("typst", base).unwrap(),
            ThemeSelection::Typst
        );
    }

    #[test]
    fn parse_false_and_none_are_unknown_theme_names() {
        let base = Path::new("/tmp");
        assert!(ThemeSelection::parse("false", base).is_err());
        assert!(ThemeSelection::parse("none", base).is_err());
    }

    #[test]
    fn parse_existing_dir_resolves_relative_to_base() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("mytheme")).unwrap();
        let sel = ThemeSelection::parse("mytheme/", dir.path()).unwrap();
        assert_eq!(sel, ThemeSelection::Dir(dir.path().join("mytheme/")));
    }

    #[test]
    fn parse_unknown_name_errors_with_builtin_list() {
        let err = ThemeSelection::parse("zensical", Path::new("/tmp")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("calepin"), "{msg}");
        assert!(msg.contains("academic"), "{msg}");
    }

    #[test]
    fn parse_missing_dir_errors() {
        let dir = tempfile::tempdir().unwrap();
        let err = ThemeSelection::parse("does/not/exist", dir.path()).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn academic_has_site_document_and_landing_entries() {
        let sel = ThemeSelection::Builtin("academic");
        let site = resolve_html_entry(&sel, HtmlScope::Site).unwrap().unwrap();
        assert_eq!(site.theme_name, "academic");
        assert!(site.layout.contains("academic-page"));

        let document = resolve_html_entry(&sel, HtmlScope::Document)
            .unwrap()
            .unwrap();
        assert_eq!(document.theme_name, "academic");
        assert!(document.layout.contains("academic-document-main"));

        let landing = resolve_explicit_site_html_entry(&sel, "layouts/site-landing.html")
            .unwrap()
            .unwrap();
        assert_eq!(landing.theme_name, "academic");
        assert!(landing.layout.contains("academic-landing"));
    }

    #[test]
    fn academic_assets_hide_topbar_on_down_scroll() {
        let sel = ThemeSelection::Builtin("academic");
        let site = resolve_html_entry(&sel, HtmlScope::Site).unwrap().unwrap();
        let css = site
            .styles
            .iter()
            .find(|(name, _)| name == "50_main.css")
            .map(|(_, source)| source)
            .expect("academic 50_main.css should be included");
        let js = site
            .scripts
            .iter()
            .find(|(name, _)| name == "main.js")
            .map(|(_, source)| source)
            .expect("academic main.js should be included");

        assert!(css.contains(".academic-topbar.is-hidden"), "{css}");
        assert!(css.contains("transform: translateY(-100%)"), "{css}");
        assert!(js.contains("initScrollAwareTopbar"), "{js}");
        assert!(js.contains("lastScrollY"), "{js}");
    }

    #[test]
    fn academic_document_uses_static_theme_controls_without_topbar() {
        let sel = ThemeSelection::Builtin("academic");
        let document = resolve_html_entry(&sel, HtmlScope::Document)
            .unwrap()
            .unwrap();
        let js = document
            .scripts
            .iter()
            .find(|(name, _)| name == "main.js")
            .map(|(_, source)| source)
            .expect("academic main.js should be included");

        assert!(document.layout.contains("academic-document-controls"));
        assert!(!document.layout.contains("calepin-site-topbar"));
        assert!(!document.layout.contains("academic-document-topbar"));
        assert!(
            !js.contains(".academic-document-topbar"),
            "document topbar should not participate in scroll hiding:\n{js}"
        );
    }

    #[test]
    fn typst_selection_resolves_to_no_entry_and_no_notebook_source() {
        assert!(resolve_html_entry(&ThemeSelection::Typst, HtmlScope::Site)
            .unwrap()
            .is_none());
        assert!(
            notebook_source(&ThemeSelection::Typst, &NotebookTemplateContext::default())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn dir_theme_without_extends_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("layouts")).unwrap();
        std::fs::write(
            dir.path().join("layouts/site.html"),
            "<html><head></head><body>X</body></html>",
        )
        .unwrap();

        let err = resolve_html_entry(
            &ThemeSelection::Dir(dir.path().to_path_buf()),
            HtmlScope::Site,
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("must declare `extends`"), "{err}");
        assert!(err.contains("extends = \"typst\""), "{err}");
    }

    #[test]
    fn dir_theme_rejects_local_extends_path() {
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().join("base");
        let child = dir.path().join("child");
        std::fs::create_dir_all(parent.join("layouts")).unwrap();
        std::fs::create_dir_all(&child).unwrap();
        std::fs::write(parent.join("layouts/site.html"), "{{ doc.body }}").unwrap();
        std::fs::write(child.join("theme.toml"), "extends = \"../base\"\n").unwrap();

        let err = resolve_html_entry(&ThemeSelection::Dir(child), HtmlScope::Site)
            .unwrap_err()
            .to_string();

        assert!(err.contains("unknown theme `../base` in extends"), "{err}");
        assert!(err.contains("built-in theme"), "{err}");
    }

    #[test]
    fn dir_theme_extends_builtin_for_missing_entries_and_overrides_css_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let theme = dir.path().join("child");
        std::fs::create_dir_all(theme.join("css")).unwrap();
        std::fs::write(theme.join("theme.toml"), "extends = \"academic\"\n").unwrap();
        std::fs::write(theme.join("css/20_theme.css"), "/* child theme */").unwrap();

        let sel = ThemeSelection::Dir(theme);
        let site = resolve_html_entry(&sel, HtmlScope::Site).unwrap().unwrap();

        assert_eq!(site.theme_name, "child");
        assert!(site.layout.contains("academic-page"));
        let theme_css = site
            .styles
            .iter()
            .find(|(name, _)| name == "20_theme.css")
            .map(|(_, css)| css)
            .unwrap();
        assert_eq!(theme_css, "/* child theme */");
        assert_eq!(
            site.styles
                .iter()
                .filter(|(name, _)| name == "20_theme.css")
                .count(),
            1
        );
    }

    #[test]
    fn dir_theme_imports_shared_css_and_js_in_manifest_order() {
        let dir = tempfile::tempdir().unwrap();
        let theme = dir.path().join("custom");
        std::fs::create_dir_all(theme.join("css")).unwrap();
        std::fs::create_dir_all(theme.join("js")).unwrap();
        std::fs::create_dir_all(theme.join("layouts")).unwrap();
        std::fs::create_dir_all(dir.path().join("shared/css")).unwrap();
        std::fs::write(theme.join("layouts/document.html"), "{{ doc.body }}").unwrap();
        std::fs::write(
            theme.join("theme.toml"),
            r#"extends = "typst"

[shared]
css = ["theme.css", "widgets.css"]
js = ["copy-code.js"]
"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("shared/css/theme.css"),
            "/* sibling shared theme */",
        )
        .unwrap();
        std::fs::write(theme.join("css/widgets.css"), "/* local override */").unwrap();
        std::fs::write(theme.join("css/local.css"), "/* local extra */").unwrap();
        std::fs::write(theme.join("js/local.js"), "console.log('local')").unwrap();

        let entry = resolve_html_entry(&ThemeSelection::Dir(theme), HtmlScope::Document)
            .unwrap()
            .unwrap();

        assert_eq!(
            entry
                .styles
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["theme.css", "widgets.css", "local.css"]
        );
        assert!(entry.styles[0].1.contains("sibling shared theme"));
        assert!(entry.styles[1].1.contains("local override"));
        assert_eq!(
            entry
                .scripts
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            ["copy-code.js", "local.js"]
        );
        assert!(entry.scripts[0].1.contains("window.CalepinCopyCode"));
    }

    #[test]
    fn local_theme_ignores_legacy_styles_directory() {
        let dir = tempfile::tempdir().unwrap();
        let theme = dir.path().join("child");
        std::fs::create_dir_all(theme.join("styles")).unwrap();
        std::fs::write(theme.join("theme.toml"), "extends = \"academic\"\n").unwrap();
        std::fs::write(theme.join("styles/20_theme.css"), "/* legacy styles dir */").unwrap();

        let entry = resolve_html_entry(&ThemeSelection::Dir(theme), HtmlScope::Site)
            .unwrap()
            .unwrap();

        assert!(!entry
            .styles
            .iter()
            .any(|(_, css)| css.contains("legacy styles dir")));
    }

    #[test]
    fn shared_imports_reject_legacy_alias_keys() {
        let dir = tempfile::tempdir().unwrap();
        let theme = dir.path().join("custom");
        std::fs::create_dir_all(theme.join("layouts")).unwrap();
        std::fs::write(theme.join("layouts/document.html"), "{{ doc.body }}").unwrap();
        std::fs::write(
            theme.join("theme.toml"),
            r#"extends = "typst"

[shared]
styles = ["theme.css"]
scripts = ["copy-code.js"]
"#,
        )
        .unwrap();

        let err = resolve_html_entry(&ThemeSelection::Dir(theme), HtmlScope::Document)
            .unwrap_err()
            .to_string();

        assert!(err.contains("unknown field"), "{err}");
        assert!(err.contains("styles"), "{err}");
    }

    #[test]
    fn shared_imports_reject_assets_key() {
        let dir = tempfile::tempdir().unwrap();
        let theme = dir.path().join("custom");
        std::fs::create_dir_all(theme.join("layouts")).unwrap();
        std::fs::write(theme.join("layouts/document.html"), "{{ doc.body }}").unwrap();
        std::fs::write(
            theme.join("theme.toml"),
            r#"extends = "typst"

[shared]
assets = ["font.woff2"]
"#,
        )
        .unwrap();

        let err = resolve_html_entry(&ThemeSelection::Dir(theme), HtmlScope::Document)
            .unwrap_err()
            .to_string();

        assert!(err.contains("unknown field"), "{err}");
        assert!(err.contains("assets"), "{err}");
    }

    #[test]
    fn shared_imports_reject_duplicate_entries() {
        let dir = tempfile::tempdir().unwrap();
        let theme = dir.path().join("custom");
        std::fs::create_dir_all(theme.join("css")).unwrap();
        std::fs::create_dir_all(theme.join("layouts")).unwrap();
        std::fs::write(theme.join("layouts/document.html"), "{{ doc.body }}").unwrap();
        std::fs::write(
            theme.join("theme.toml"),
            r#"extends = "typst"

[shared]
css = ["site.css", "site.css"]
"#,
        )
        .unwrap();
        std::fs::write(theme.join("css/site.css"), "/* local */").unwrap();

        let err = resolve_html_entry(&ThemeSelection::Dir(theme), HtmlScope::Document).unwrap_err();

        assert!(err.to_string().contains("listed more than once"));
    }

    #[test]
    fn shared_manifest_rejects_path_imports() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("layouts")).unwrap();
        std::fs::write(dir.path().join("layouts/document.html"), "{{ doc.body }}").unwrap();
        std::fs::write(
            dir.path().join("theme.toml"),
            r#"extends = "typst"

[shared]
css = ["../theme.css"]
"#,
        )
        .unwrap();

        let err = match resolve_html_entry(
            &ThemeSelection::Dir(dir.path().to_path_buf()),
            HtmlScope::Document,
        ) {
            Ok(_) => panic!("expected shared path import to be rejected"),
            Err(error) => error,
        };

        assert!(err.to_string().contains("must be a filename"));
    }

    #[test]
    fn explicit_site_layout_uses_exact_theme_relative_path() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("layouts")).unwrap();
        std::fs::write(dir.path().join("layouts/site.html"), "{{ doc.body }}").unwrap();
        std::fs::write(dir.path().join("layouts/site-landing.html"), "landing").unwrap();
        std::fs::write(dir.path().join("theme.toml"), "extends = \"typst\"\n").unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());

        let entry = resolve_explicit_site_html_entry(&sel, "layouts/site-landing.html")
            .unwrap()
            .unwrap();

        assert_eq!(entry.layout, "landing");
    }

    #[test]
    fn explicit_site_layout_requires_local_or_inherited_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("layouts")).unwrap();
        std::fs::write(dir.path().join("layouts/pdf.typ"), "{{ document.body }}").unwrap();
        std::fs::write(dir.path().join("theme.toml"), "extends = \"typst\"\n").unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());

        let err = resolve_explicit_site_html_entry(&sel, "layouts/site-landing.html")
            .unwrap_err()
            .to_string();

        assert!(err.contains("layouts/site-landing.html"), "{err}");
    }

    #[test]
    fn explicit_builtin_landing_layout_resolves() {
        let entry =
            resolve_explicit_site_html_entry(&ThemeSelection::Default, "layouts/site-landing.html")
                .unwrap()
                .unwrap();

        assert!(entry.layout.contains("calepin-website-main--landing"));
    }

    #[test]
    fn explicit_site_layout_rejects_sugar_and_escape_paths() {
        for value in [
            "landing",
            "landing.typ",
            "../site-landing.html",
            "/tmp/site-landing.html",
        ] {
            let err = match resolve_explicit_site_html_entry(&ThemeSelection::Default, value) {
                Ok(_) => panic!("expected `{value}` to be rejected"),
                Err(error) => error.to_string(),
            };
            assert!(
                err.contains("page layout path") || err.contains("inside the active theme"),
                "{err}"
            );
        }
    }

    #[test]
    fn explicit_site_layout_does_not_fallback_to_default_theme() {
        let err = match resolve_explicit_site_html_entry(
            &ThemeSelection::Builtin("academic"),
            "layouts/missing.html",
        ) {
            Ok(_) => panic!("expected missing explicit layout to be rejected"),
            Err(error) => error.to_string(),
        };

        assert!(err.contains("does not contain page layout"), "{err}");
    }

    #[test]
    fn empty_notebook_typ_means_no_styling_not_fallback() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("layouts")).unwrap();
        std::fs::write(dir.path().join("layouts/pdf.typ"), "").unwrap();
        std::fs::write(dir.path().join("theme.toml"), "extends = \"typst\"\n").unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());
        assert_eq!(
            notebook_source(&sel, &NotebookTemplateContext::default()).unwrap(),
            Some(NotebookSource {
                source: String::new(),
                owns_body: false,
            })
        );
    }

    #[test]
    fn notebook_typ_can_use_calepin_context() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("layouts")).unwrap();
        std::fs::write(
            dir.path().join("layouts/pdf.typ"),
            r#"#let title = "{{ document.meta.title }}"
#let species = "{{ params.Species }}"
"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("theme.toml"), "extends = \"typst\"\n").unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());
        let context = NotebookTemplateContext {
            input_path: "reports/iris.typ".to_string(),
            input_dir: "reports".to_string(),
            input_stem: "iris".to_string(),
            body: "#include \"/.calepin/reports/iris/source.typ\"".to_string(),
            page_meta: serde_json::json!({"title": "Iris Report"}),
            params: serde_json::json!({"Species": "setosa"}),
        };

        let source = notebook_source(&sel, &context).unwrap().unwrap();
        assert_eq!(
            source.source,
            "#let title = \"Iris Report\"\n#let species = \"setosa\""
        );
        assert!(!source.owns_body);
    }

    #[test]
    fn notebook_typ_can_place_document_body() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("layouts")).unwrap();
        std::fs::write(
            dir.path().join("layouts/pdf.typ"),
            r#"#set text(size: 11pt)
{{ document.body }}
[#emph[Generated footer]]
"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("theme.toml"), "extends = \"typst\"\n").unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());
        let context = NotebookTemplateContext {
            input_path: "paper.typ".to_string(),
            input_dir: String::new(),
            input_stem: "paper".to_string(),
            body: "#include \"/.calepin/paper/source.typ\"".to_string(),
            page_meta: serde_json::Value::Null,
            params: serde_json::json!({}),
        };

        let source = notebook_source(&sel, &context).unwrap().unwrap();
        assert_eq!(
            source.source,
            "#set text(size: 11pt)\n#include \"/.calepin/paper/source.typ\"\n[#emph[Generated footer]]"
        );
        assert!(source.owns_body);
    }

    #[test]
    fn academic_notebook_installs_margin_elements_without_page_setup() {
        let source = notebook_source(
            &ThemeSelection::Builtin("academic"),
            &NotebookTemplateContext::default(),
        )
        .unwrap()
        .unwrap();

        assert!(
            source.source.contains("set-margin-impl"),
            "academic paged output should keep marginalia-backed margin elements"
        );
        assert!(
            !source.source.contains("#show: marginalia.setup"),
            "academic paged output should not reserve margin space automatically"
        );
    }

    #[test]
    fn academic_notebook_maps_auto_sidenote_numbering_to_numbers() {
        let source = notebook_source(
            &ThemeSelection::Builtin("academic"),
            &NotebookTemplateContext::default(),
        )
        .unwrap()
        .unwrap();

        assert!(
            source
                .source
                .contains(r#"if numbering == auto { "1" } else { numbering }"#),
            "academic paged output should map Calepin's default numbering to marginalia's numeric numbering"
        );
    }

    #[test]
    fn old_notebook_typ_jinja_is_rejected_for_local_themes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("notebook.typ.jinja"),
            "{{ target }} {{ document.body }}",
        )
        .unwrap();
        std::fs::write(dir.path().join("theme.toml"), "extends = \"typst\"\n").unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());
        let err = notebook_source(&sel, &NotebookTemplateContext::default())
            .unwrap_err()
            .to_string();
        assert!(err.contains("notebook.typ.jinja"), "{err}");
        assert!(err.contains("layouts/pdf.typ"), "{err}");
    }

    #[test]
    fn paged_typ_jinja_is_rejected_for_local_themes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("paged.typ.jinja"),
            "{{ target }} {{ document.body }}",
        )
        .unwrap();
        std::fs::write(dir.path().join("theme.toml"), "extends = \"typst\"\n").unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());
        let err = notebook_source(&sel, &NotebookTemplateContext::default())
            .unwrap_err()
            .to_string();
        assert!(err.contains("paged.typ.jinja"), "{err}");
        assert!(err.contains("layouts/pdf.typ"), "{err}");
    }

    #[test]
    fn theme_dir_without_any_entry_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());
        assert!(resolve_html_entry(&sel, HtmlScope::Site).is_err());
    }

    #[test]
    fn theme_dir_with_only_legacy_paged_typ_errors() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("paged.typ"), "#set text(size: 10pt)").unwrap();
        std::fs::write(dir.path().join("theme.toml"), "extends = \"typst\"\n").unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());

        assert!(notebook_source(&sel, &NotebookTemplateContext::default()).is_err());
    }

    #[test]
    fn eject_builtin_copies_default_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let dest =
            bundle::eject_builtin(DEFAULT_THEME_NAME, &dir.path().join("themes"), false).unwrap();

        assert_eq!(dest, dir.path().join("themes/calepin"));
        assert!(dest.join("layouts/document.html").is_file());
        assert!(dest.join("layouts/site.html").is_file());
        assert!(dest.join("layouts/site-landing.html").is_file());
        assert!(dest.join("layouts/pdf.typ").is_file());
        assert!(!dest.join("notebook.typ.jinja").exists());
        assert!(!dest.join("paged.typ.jinja").exists());
        assert!(!dest.join("paged.typ").exists());
        assert!(dest.join("partials/site-nav-item.html").is_file());
        assert!(dest.join("partials/document-theme-switcher.html").is_file());
        assert!(dest.join("partials/site-nav.html").is_file());
        assert!(dest.join("partials/document-head.html").is_file());
        assert!(dest.join("partials/site-head.html").is_file());
        assert!(dest.join("partials/site-head-meta.html").is_file());
        assert!(dest.join("partials/site-brand.html").is_file());
        assert!(dest.join("partials/site-language-picker.html").is_file());
        assert!(dest.join("partials/site-nav-prev-next.html").is_file());
        assert!(dest.join("partials/site-sidebar-item.html").is_file());
        assert!(dest.join("partials/theme-init-script.html").is_file());
        assert!(dest.join("partials/theme-styles.html").is_file());
        assert!(dest.join("partials/theme-scripts.html").is_file());
        assert!(dest.join("partials/site-search.html").is_file());
        assert!(dest.join("theme.toml").is_file());
        assert!(dest.join("css/20_theme.css").is_file());
        assert!(dest.join("css/30_code.css").is_file());
        assert!(dest.join("css/40_widgets.css").is_file());
        assert!(dest.join("css/50_site.css").is_file());
        assert!(dest.join("css/60_document.css").is_file());
        assert!(dest.join("js/theme-toggle.js").is_file());
        assert!(dest.join("js/language-picker.js").is_file());
        assert!(dest.join("js/copy-code.js").is_file());
        assert!(dest.join("js/site.js").is_file());
        assert!(!dest.parent().unwrap().join("shared").exists());

        assert!(std::fs::read_to_string(dest.join("css/40_widgets.css"))
            .unwrap()
            .contains("[data-calepin-theme-toggle]"));
        assert!(std::fs::read_to_string(dest.join("js/copy-code.js"))
            .unwrap()
            .contains("window.CalepinCopyCode"));
    }

    #[test]
    fn eject_builtin_to_copies_into_requested_directory() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("custom-theme");
        let wrote = eject_builtin_to("academic", &dest, false).unwrap();

        assert_eq!(wrote, dest);
        assert!(wrote.join("layouts/site.html").is_file());
        assert!(wrote.join("partials/site-nav-item.html").is_file());
        assert!(wrote.join("theme.toml").is_file());
        assert!(wrote.join("css/50_main.css").is_file());
        assert!(wrote.join("js/main.js").is_file());
        assert!(wrote.join("partials/theme-toggle-button.html").is_file());
        assert!(wrote.join("partials/document-head.html").is_file());
        assert!(wrote.join("partials/site-head.html").is_file());
        assert!(wrote.join("partials/site-brand.html").is_file());
        assert!(wrote.join("partials/site-language-picker.html").is_file());
        assert!(wrote.join("partials/site-nav-prev-next.html").is_file());
        assert!(wrote.join("partials/site-sidebar-item.html").is_file());
        assert!(wrote.join("css/20_theme.css").is_file());
        assert!(wrote.join("js/copy-code.js").is_file());
        assert!(!wrote.parent().unwrap().join("shared").exists());
    }

    #[test]
    fn eject_builtin_refuses_existing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let themes = dir.path().join("themes");
        std::fs::create_dir_all(themes.join("calepin")).unwrap();

        let err = bundle::eject_builtin(DEFAULT_THEME_NAME, &themes, false).unwrap_err();

        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn eject_builtin_unknown_name_lists_builtins() {
        let dir = tempfile::tempdir().unwrap();
        let err = bundle::eject_builtin("zensical", &dir.path().join("themes"), false).unwrap_err();

        assert!(err.to_string().contains("academic"));
    }
}
