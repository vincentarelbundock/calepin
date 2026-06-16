//! Theme bundles. A theme is a directory named by family; the target and
//! scope dimensions are well-known filenames inside it: `paged.typ.jinja`,
//! `layouts/notebook.html`, `layouts/webpage.html`. See specs/2026-06-10-theme-bundles-design.md.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

mod bundle;
mod html;
mod paged;

pub use bundle::{builtin_names, eject_builtin, eject_builtin_to};
pub use html::{resolve_explicit_site_html_entry, resolve_html_entry, HtmlEntry, HtmlScope};
pub use paged::{paged_source, PagedSource, PagedTemplateContext};

pub const DEFAULT_THEME_NAME: &str = "calepin";

/// Where a render gets its theme from. `Default` means "no selection made",
/// which resolves to the builtin `calepin` bundle.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ThemeSelection {
    #[default]
    Default,
    /// `theme = false`: raw output, no theming for any target.
    Disabled,
    Builtin(&'static str),
    Dir(PathBuf),
}

impl ThemeSelection {
    /// Parse a CLI or config value. Relative path-like values resolve
    /// against `base_dir` (the config file's directory, or the CLI's cwd).
    pub fn parse(value: &str, base_dir: &Path) -> Result<Self> {
        if value == "false" || value == "none" {
            return Ok(Self::Disabled);
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
            "unknown theme `{value}`; use one of {} or a path to a theme directory",
            builtin_names().join(", ")
        ))
    }
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
    let has_entry = [
        "paged.typ.jinja",
        "layouts/notebook.html",
        "layouts/webpage.html",
    ]
    .iter()
    .any(|file| dir.join(file).is_file());
    if !has_entry {
        return Err(anyhow!(
            "theme directory {} contains none of paged.typ.jinja, layouts/notebook.html, layouts/webpage.html",
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
    use std::path::Path;

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
        assert_eq!(
            ThemeSelection::parse("tufte", base).unwrap(),
            ThemeSelection::Builtin("tufte")
        );
    }

    #[test]
    fn parse_false_and_none_disable() {
        let base = Path::new("/tmp");
        assert_eq!(
            ThemeSelection::parse("false", base).unwrap(),
            ThemeSelection::Disabled
        );
        assert_eq!(
            ThemeSelection::parse("none", base).unwrap(),
            ThemeSelection::Disabled
        );
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
        assert!(msg.contains("tufte"), "{msg}");
    }

    #[test]
    fn parse_missing_dir_errors() {
        let dir = tempfile::tempdir().unwrap();
        let err = ThemeSelection::parse("does/not/exist", dir.path()).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn academic_falls_back_to_default_for_document_scope() {
        let sel = ThemeSelection::Builtin("academic");
        let entry = resolve_html_entry(&sel, HtmlScope::Document)
            .unwrap()
            .unwrap();
        assert_eq!(entry.theme_name, "calepin");
        assert!(entry.is_default);
        let site = resolve_html_entry(&sel, HtmlScope::Site).unwrap().unwrap();
        assert_eq!(site.theme_name, "academic");
        assert!(!site.is_default);
    }

    #[test]
    fn tufte_has_site_document_and_landing_entries() {
        let sel = ThemeSelection::Builtin("tufte");
        let site = resolve_html_entry(&sel, HtmlScope::Site).unwrap().unwrap();
        assert_eq!(site.theme_name, "tufte");
        assert!(!site.is_default);
        assert!(site.layout.contains("tufte-page"));

        let document = resolve_html_entry(&sel, HtmlScope::Document)
            .unwrap()
            .unwrap();
        assert_eq!(document.theme_name, "tufte");
        assert!(!document.is_default);
        assert!(document.layout.contains("tufte-document-main"));

        let landing = resolve_explicit_site_html_entry(&sel, "layouts/landing.html")
            .unwrap()
            .unwrap();
        assert_eq!(landing.theme_name, "tufte");
        assert!(!landing.is_default);
        assert!(landing.layout.contains("tufte-landing"));
    }

    #[test]
    fn disabled_selection_resolves_to_no_entry_and_no_paged_source() {
        assert!(
            resolve_html_entry(&ThemeSelection::Disabled, HtmlScope::Site)
                .unwrap()
                .is_none()
        );
        assert!(
            paged_source(&ThemeSelection::Disabled, &PagedTemplateContext::default())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn dir_theme_uses_own_entry_and_falls_back_per_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("layouts")).unwrap();
        std::fs::write(
            dir.path().join("layouts/webpage.html"),
            "<html><head></head><body>X</body></html>",
        )
        .unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());
        let site = resolve_html_entry(&sel, HtmlScope::Site).unwrap().unwrap();
        assert!(!site.is_default);
        let document = resolve_html_entry(&sel, HtmlScope::Document)
            .unwrap()
            .unwrap();
        assert!(document.is_default);
        // paged.typ.jinja absent: default paged styling
        assert_eq!(
            paged_source(&sel, &PagedTemplateContext::default()).unwrap(),
            paged_source(&ThemeSelection::Default, &PagedTemplateContext::default()).unwrap()
        );
    }

    #[test]
    fn dir_theme_imports_shared_assets_in_manifest_order() {
        let dir = tempfile::tempdir().unwrap();
        let theme = dir.path().join("custom");
        std::fs::create_dir_all(theme.join("styles")).unwrap();
        std::fs::create_dir_all(theme.join("scripts")).unwrap();
        std::fs::create_dir_all(theme.join("layouts")).unwrap();
        std::fs::create_dir_all(dir.path().join("shared/styles")).unwrap();
        std::fs::write(theme.join("layouts/notebook.html"), "{{ doc.body }}").unwrap();
        std::fs::write(
            theme.join("theme.toml"),
            r#"[shared]
styles = ["theme.css", "widgets.css"]
scripts = ["copy-code.js"]
"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("shared/styles/theme.css"),
            "/* sibling shared theme */",
        )
        .unwrap();
        std::fs::write(theme.join("styles/widgets.css"), "/* local override */").unwrap();
        std::fs::write(theme.join("styles/local.css"), "/* local extra */").unwrap();
        std::fs::write(theme.join("scripts/local.js"), "console.log('local')").unwrap();

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
    fn shared_manifest_rejects_path_imports() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("layouts")).unwrap();
        std::fs::write(dir.path().join("layouts/notebook.html"), "{{ doc.body }}").unwrap();
        std::fs::write(
            dir.path().join("theme.toml"),
            r#"[shared]
styles = ["../theme.css"]
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
        std::fs::write(dir.path().join("layouts/webpage.html"), "{{ doc.body }}").unwrap();
        std::fs::write(dir.path().join("layouts/landing.html"), "landing").unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());

        let entry = resolve_explicit_site_html_entry(&sel, "layouts/landing.html")
            .unwrap()
            .unwrap();

        assert_eq!(entry.layout, "landing");
        assert!(!entry.is_default);
    }

    #[test]
    fn explicit_site_layout_falls_back_for_paged_only_local_theme() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("paged.typ.jinja"), "{{ document.body }}").unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());

        let entry = resolve_explicit_site_html_entry(&sel, "layouts/landing.html")
            .unwrap()
            .unwrap();

        assert!(entry.layout.contains("calepin-website-main--landing"));
        assert!(entry.is_default);
    }

    #[test]
    fn explicit_builtin_landing_layout_resolves() {
        let entry =
            resolve_explicit_site_html_entry(&ThemeSelection::Default, "layouts/landing.html")
                .unwrap()
                .unwrap();

        assert!(entry.layout.contains("calepin-website-main--landing"));
        assert!(entry.is_default);
    }

    #[test]
    fn explicit_site_layout_rejects_sugar_and_escape_paths() {
        for value in [
            "landing",
            "landing.typ",
            "../landing.html",
            "/tmp/landing.html",
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
            "layouts/notebook.html",
        ) {
            Ok(_) => panic!("expected missing explicit layout to be rejected"),
            Err(error) => error.to_string(),
        };

        assert!(err.contains("does not contain page layout"), "{err}");
    }

    #[test]
    fn empty_paged_typ_jinja_means_no_styling_not_fallback() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("paged.typ.jinja"), "").unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());
        assert_eq!(
            paged_source(&sel, &PagedTemplateContext::default()).unwrap(),
            Some(PagedSource {
                source: String::new(),
                owns_body: false,
            })
        );
    }

    #[test]
    fn paged_typ_jinja_can_use_calepin_context() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("paged.typ.jinja"),
            r#"#let title = "{{ document.meta.title }}"
#let species = "{{ params.Species }}"
"#,
        )
        .unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());
        let context = PagedTemplateContext {
            input_path: "reports/iris.typ".to_string(),
            input_dir: "reports".to_string(),
            input_stem: "iris".to_string(),
            body: "#include \"/.calepin/reports/iris/source.typ\"".to_string(),
            page_meta: serde_json::json!({"title": "Iris Report"}),
            params: serde_json::json!({"Species": "setosa"}),
        };

        let source = paged_source(&sel, &context).unwrap().unwrap();
        assert_eq!(
            source.source,
            "#let title = \"Iris Report\"\n#let species = \"setosa\""
        );
        assert!(!source.owns_body);
    }

    #[test]
    fn paged_typ_jinja_can_place_document_body() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("paged.typ.jinja"),
            r#"#set text(size: 11pt)
{{ document.body }}
[#emph[Generated footer]]
"#,
        )
        .unwrap();
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());
        let context = PagedTemplateContext {
            input_path: "paper.typ".to_string(),
            input_dir: String::new(),
            input_stem: "paper".to_string(),
            body: "#include \"/.calepin/paper/source.typ\"".to_string(),
            page_meta: serde_json::Value::Null,
            params: serde_json::json!({}),
        };

        let source = paged_source(&sel, &context).unwrap().unwrap();
        assert_eq!(
            source.source,
            "#set text(size: 11pt)\n#include \"/.calepin/paper/source.typ\"\n[#emph[Generated footer]]"
        );
        assert!(source.owns_body);
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
        let sel = ThemeSelection::Dir(dir.path().to_path_buf());

        assert!(paged_source(&sel, &PagedTemplateContext::default()).is_err());
    }

    #[test]
    fn eject_builtin_copies_default_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let dest = eject_builtin(DEFAULT_THEME_NAME, &dir.path().join("themes"), false).unwrap();

        assert_eq!(dest, dir.path().join("themes/calepin"));
        assert!(dest.join("layouts/notebook.html").is_file());
        assert!(dest.join("layouts/webpage.html").is_file());
        assert!(dest.join("layouts/landing.html").is_file());
        assert!(dest.join("paged.typ.jinja").is_file());
        assert!(!dest.join("paged.typ").exists());
        assert!(dest.join("partials/navbar-item.html").is_file());
        assert!(dest.join("partials/theme-switcher.html").is_file());
        assert!(dest.join("partials/site-topbar.html").is_file());
        assert!(!dest.join("partials/scripts.html").exists());
        assert!(!dest.join("partials/styles.html").exists());
        assert!(dest.join("theme.toml").is_file());
        assert!(dest.join("styles/site.css").is_file());
        assert!(dest.join("scripts/site.js").is_file());
        assert!(!dest.join("styles/00-theme.css").exists());
        assert!(!dest.join("scripts/02-copy-code.js").exists());

        let shared = dest.parent().unwrap().join("shared");
        assert!(shared.join("partials/site-meta.html").is_file());
        assert!(shared.join("partials/theme-init.html").is_file());
        assert!(shared.join("partials/styles.html").is_file());
        assert!(shared.join("partials/scripts.html").is_file());
        assert!(shared.join("partials/pagefind-modal.html").is_file());
        assert!(shared.join("partials/theme-toggle.html").is_file());
        assert!(shared.join("styles/theme.css").is_file());
        assert!(shared.join("styles/code.css").is_file());
        assert!(shared.join("styles/widgets.css").is_file());
        assert!(shared.join("scripts/theme-toggle.js").is_file());
        assert!(shared.join("scripts/language-picker.js").is_file());
        assert!(shared.join("scripts/copy-code.js").is_file());
        assert!(std::fs::read_to_string(shared.join("styles/widgets.css"))
            .unwrap()
            .contains("[data-calepin-theme-toggle]"));
        assert!(std::fs::read_to_string(shared.join("scripts/copy-code.js"))
            .unwrap()
            .contains("window.CalepinCopyCode"));
    }

    #[test]
    fn eject_builtin_to_copies_into_requested_directory() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("custom-theme");
        let wrote = eject_builtin_to("academic", &dest, false).unwrap();

        assert_eq!(wrote, dest);
        assert!(wrote.join("layouts/webpage.html").is_file());
        assert!(wrote.join("partials/navbar-item.html").is_file());
        assert!(wrote.join("theme.toml").is_file());
        assert!(wrote.join("styles/main.css").is_file());
        assert!(wrote.join("scripts/main.js").is_file());
        let shared = wrote.parent().unwrap().join("shared");
        assert!(shared.join("styles/theme.css").is_file());
        assert!(shared.join("scripts/copy-code.js").is_file());
    }

    #[test]
    fn eject_builtin_refuses_existing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let themes = dir.path().join("themes");
        std::fs::create_dir_all(themes.join("calepin")).unwrap();

        let err = eject_builtin(DEFAULT_THEME_NAME, &themes, false).unwrap_err();

        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn eject_builtin_unknown_name_lists_builtins() {
        let dir = tempfile::tempdir().unwrap();
        let err = eject_builtin("zensical", &dir.path().join("themes"), false).unwrap_err();

        assert!(err.to_string().contains("academic"));
        assert!(err.to_string().contains("tufte"));
    }
}
