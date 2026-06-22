use super::icons::nav_label_html;
use super::navigation::NavItemModel;
use super::*;
use crate::utils::html::escape as html_escape;
use crate::utils::testutil::{command_available, tempdir_in_manifest};

fn test_build_result(root: &Path, pages: &[PathBuf]) -> WebsiteBuildResult {
    WebsiteBuildResult {
        src_dir: root.to_path_buf(),
        out_dir: root.to_path_buf(),
        asset_dir: PathBuf::from(".calepin"),
        config_path: root.join("calepin.toml"),
        theme_dirs: Vec::new(),
        page_fingerprints: fingerprint_files(pages).unwrap(),
        nav_signature: 0,
        pages_signature: 0,
    }
}

fn test_page_info(src: &Path, files: &[PathBuf], pdf_files: &BTreeSet<PathBuf>) -> PageInfoMap {
    build_page_info(src, files, &PageMetaMap::new(), pdf_files, &None).unwrap()
}

fn website_config_from_toml(toml: &str) -> WebsiteConfig {
    try_website_config_from_toml(toml).unwrap()
}

fn try_website_config_from_toml(toml: &str) -> Result<WebsiteConfig, toml::de::Error> {
    toml::from_str(toml)
}

#[test]
fn theme_key_parses_builtin_name() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("calepin.toml"), r#"theme = "academic""#).unwrap();

    let config =
        crate::config::CalepinConfig::load(dir.path(), Some(&dir.path().join("calepin.toml")))
            .unwrap();

    assert_eq!(
        config.theme_selection().unwrap(),
        Some(crate::theme::ThemeSelection::Builtin("academic"))
    );
}

#[test]
fn theme_key_typst_selects_raw_typst_output() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("calepin.toml"), r#"theme = "typst""#).unwrap();

    let config =
        crate::config::CalepinConfig::load(dir.path(), Some(&dir.path().join("calepin.toml")))
            .unwrap();

    assert_eq!(
        config.theme_selection().unwrap(),
        Some(crate::theme::ThemeSelection::Typst)
    );
}

#[test]
fn missing_theme_key_is_default() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("calepin.toml"), "").unwrap();

    let config =
        crate::config::CalepinConfig::load(dir.path(), Some(&dir.path().join("calepin.toml")))
            .unwrap();

    assert_eq!(config.theme_selection().unwrap(), None);
}

#[test]
fn website_config_defaults_asset_dir_to_dot_calepin() {
    let config = website_config_from_toml("");

    assert_eq!(
        resolve_website_asset_dir(&config).unwrap(),
        PathBuf::from(".calepin")
    );
}

#[test]
fn website_config_allows_custom_asset_dir() {
    let config = website_config_from_toml(r#"asset-dir = "_calepin""#);

    assert_eq!(
        resolve_website_asset_dir(&config).unwrap(),
        PathBuf::from("_calepin")
    );
}

#[test]
fn website_config_rejects_invalid_asset_dir() {
    let parent = website_config_from_toml(r#"asset-dir = "../_calepin""#);
    assert!(resolve_website_asset_dir(&parent).is_err());

    let absolute = website_config_from_toml(&format!(
        "asset-dir = \"{}\"",
        Path::new("/tmp/assets").display()
    ));
    assert!(resolve_website_asset_dir(&absolute).is_err());
}

#[test]
fn website_build_result_canonicalizes_config_theme_dir() {
    if !command_available("typst") {
        return;
    }

    let dir = tempdir_in_manifest("calepin-website-test-");
    let root = dir.path();
    let src = root.join("docs");
    let config = root.join("config");
    let theme = root.join("theme");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(&config).unwrap();
    std::fs::create_dir_all(theme.join("layouts")).unwrap();
    std::fs::write(theme.join("layouts/webpage.html"), "{{ doc.body }}").unwrap();
    std::fs::write(config.join("calepin.toml"), r#"theme = "../theme""#).unwrap();
    std::fs::write(src.join("index.typ"), "#set document(title: [Home])\nHome").unwrap();

    let result = build_site(WebsiteBuildOptions {
        config: config.join("calepin.toml"),
        src: Some(src),
        out: Some(root.join("out")),
        parallelism: Some(1),
        render_pdf: Some(false),
        quiet: true,
        timeout: None,
        params: Vec::new(),
        typst_args: Vec::new(),
        incremental_inputs: None,
        clean: true,
        minify_html: false,
    })
    .unwrap();

    let canonical_theme = theme.canonicalize().unwrap();
    let canonical_theme_file = theme.join("layouts/webpage.html").canonicalize().unwrap();
    assert_eq!(result.theme_dirs, vec![canonical_theme]);
    assert!(should_rebuild_for_path(&result, &canonical_theme_file));
}

#[test]
fn website_build_writes_runtime_to_custom_asset_dir() {
    if !command_available("typst") {
        return;
    }

    let dir = tempdir_in_manifest("calepin-website-test-");
    let root = dir.path();
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(
        root.join("calepin.toml"),
        r#"
asset-dir = "_runtime"
theme = "calepin"
"#,
    )
    .unwrap();
    std::fs::write(root.join("index.typ"), "#set document(title: [Home])\nHome").unwrap();

    let result = build_site(WebsiteBuildOptions {
        config: root.join("calepin.toml"),
        src: Some(root.to_path_buf()),
        out: Some(root.join("out")),
        parallelism: Some(1),
        render_pdf: Some(false),
        quiet: true,
        timeout: None,
        params: Vec::new(),
        typst_args: Vec::new(),
        incremental_inputs: None,
        clean: true,
        minify_html: false,
    })
    .unwrap();

    let runtime = root.join("_runtime/calepin.typ");
    let wrapper = root
        .join("_runtime")
        .join("index")
        .join("calepin-wrapper.typ");

    assert_eq!(result.asset_dir, PathBuf::from("_runtime"));
    assert!(
        runtime.exists(),
        "runtime should be written to asset-dir: {}",
        runtime.display()
    );
    assert!(
        wrapper.exists(),
        "render wrapper should be generated: {}",
        wrapper.display()
    );

    let wrapper_source = std::fs::read_to_string(wrapper).unwrap();
    assert!(wrapper_source.contains("#import \"/_runtime/calepin.typ\""));
}

#[test]
fn website_build_result_normalizes_created_output_dir_inside_source() {
    if !command_available("typst") {
        return;
    }

    let dir = tempdir_in_manifest("calepin-website-test-");
    let root = dir.path();
    let src = root.join("docs");
    std::fs::create_dir_all(src.join("tmp")).unwrap();
    std::fs::write(src.join("calepin.toml"), r#"theme = "calepin""#).unwrap();
    std::fs::write(src.join("index.typ"), "#set document(title: [Home])\nHome").unwrap();

    let result = build_site(WebsiteBuildOptions {
        config: src.join("calepin.toml"),
        src: Some(src.clone()),
        out: Some(src.join("tmp/../_site")),
        parallelism: Some(1),
        render_pdf: Some(false),
        quiet: true,
        timeout: None,
        params: Vec::new(),
        typst_args: Vec::new(),
        incremental_inputs: None,
        clean: true,
        minify_html: false,
    })
    .unwrap();

    let canonical_out = src.join("_site").canonicalize().unwrap();
    assert_eq!(result.out_dir, canonical_out);
    assert!(!should_rebuild_for_path(
        &result,
        &canonical_out.join("index.html")
    ));
}

#[test]
fn watch_roots_include_configured_theme_dirs() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("docs");
    let theme = temp.path().join("theme");
    let mut current = test_build_result(&src, &[]);
    current.theme_dirs = vec![theme.clone()];

    assert_eq!(
        watch_roots(&current),
        vec![
            (src, RecursiveMode::Recursive),
            (
                temp.path().join("docs/calepin.toml"),
                RecursiveMode::NonRecursive
            ),
            (theme, RecursiveMode::Recursive),
        ]
    );
}

#[test]
fn watch_roots_changed_detects_theme_updates() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("docs");
    let theme = temp.path().join("theme");
    let current = test_build_result(&src, &[]);

    let mut theme_next = current.clone();
    theme_next.theme_dirs = vec![theme];
    assert!(watch_roots_changed(&current, &theme_next));

    assert!(!watch_roots_changed(&theme_next, &theme_next));
}

#[test]
fn html_theme_key_is_rejected() {
    assert!(try_website_config_from_toml(r#"html_theme = "academic""#).is_err());
}

#[test]
fn home_config_key_is_rejected() {
    assert!(try_website_config_from_toml(r#"home = "index.html""#).is_err());
}

#[test]
fn static_config_parses_include_exclude_and_rejects_unknown_fields() {
    let config = website_config_from_toml(
        r#"
[static]
include = ["assets/**", "robots.txt"]
exclude = ["assets/private/**"]
"#,
    );
    let static_files = config.static_files.unwrap();

    assert_eq!(
        static_files.include,
        vec!["assets/**".to_string(), "robots.txt".to_string()]
    );
    assert_eq!(static_files.exclude, vec!["assets/private/**".to_string()]);
    assert!(try_website_config_from_toml(
        r#"
[static]
copy = ["assets/**"]
"#
    )
    .is_err());
}

#[test]
fn wildcard_match_keeps_single_star_within_path_segment() {
    assert!(wildcard_match("*.png", "logo.png"));
    assert!(!wildcard_match("*.png", "assets/logo.png"));
    assert!(wildcard_match("assets/*.png", "assets/logo.png"));
    assert!(!wildcard_match("assets/*.png", "assets/icons/logo.png"));
    assert!(wildcard_match("assets/**", "assets/icons/logo.png"));
    assert!(wildcard_match(
        "assets/**/logo.png",
        "assets/icons/ui/logo.png"
    ));
    assert!(wildcard_match("assets/?.png", "assets/a.png"));
    assert!(!wildcard_match("assets/?.png", "assets/a/b.png"));
}

#[test]
fn robots_config_defaults_enabled_and_accepts_toggle_or_table() {
    let config = website_config_from_toml("");
    assert!(config.robots_enabled());

    let config = website_config_from_toml("robots = false");
    assert!(!config.robots_enabled());

    let config = website_config_from_toml(
        r#"
[robots]
enabled = false
"#,
    );
    assert!(!config.robots_enabled());

    assert!(try_website_config_from_toml(
        r#"
[robots]
allow = false
"#
    )
    .is_err());
}

#[test]
fn minify_config_defaults_disabled_and_accepts_toggle() {
    let config = website_config_from_toml("");
    assert_eq!(config.minify, None);

    let config = website_config_from_toml("minify = true");
    assert_eq!(config.minify, Some(true));
}

#[test]
fn search_config_accepts_pagefind_and_rejects_unknown_engines() {
    let config = website_config_from_toml("");
    assert_eq!(config.search, None);

    let config = website_config_from_toml(r#"search = "pagefind""#);
    assert_eq!(config.search, Some(SearchEngine::Pagefind));

    assert!(try_website_config_from_toml(r#"search = "lunr""#).is_err());
}

#[test]
fn feed_config_defaults_to_atom_and_accepts_explicit_targets() {
    let config = website_config_from_toml(
        r#"
generate_feeds = true

[feeds]
limit = 10
filenames = ["atom.xml", "rss.xml"]

[[feeds.file]]
filename = "updates.xml"
format = "rss"
template = "feeds/custom-rss.xml"
"#,
    );
    assert!(config.feeds_enabled());
    let targets = feed_targets(&config).unwrap();

    assert_eq!(
        targets,
        vec![
            FeedTarget {
                filename: "atom.xml".to_string(),
                format: FeedFormat::Atom,
                template: None,
            },
            FeedTarget {
                filename: "rss.xml".to_string(),
                format: FeedFormat::Rss,
                template: None,
            },
            FeedTarget {
                filename: "updates.xml".to_string(),
                format: FeedFormat::Rss,
                template: Some("feeds/custom-rss.xml".to_string()),
            },
        ]
    );

    let default = website_config_from_toml("generate_feeds = true");
    assert_eq!(
        feed_targets(&default).unwrap(),
        vec![FeedTarget {
            filename: "atom.xml".to_string(),
            format: FeedFormat::Atom,
            template: None,
        }]
    );
}

#[test]
fn infer_feed_format_only_treats_rss_names_as_rss() {
    assert_eq!(infer_feed_format("rss.xml"), FeedFormat::Rss);
    assert_eq!(infer_feed_format("feeds/updates.rss"), FeedFormat::Rss);
    assert_eq!(infer_feed_format("myrss.xml"), FeedFormat::Atom);
}

#[test]
fn feed_config_rejects_unsafe_or_duplicate_filenames() {
    assert!(feed_targets(&website_config_from_toml(
        r#"
generate_feeds = true
[feeds]
filenames = ["../atom.xml"]
"#
    ))
    .is_err());

    assert!(feed_targets(&website_config_from_toml(
        r#"
generate_feeds = true
[feeds]
filenames = ["atom.xml"]
[[feeds.file]]
filename = "atom.xml"
"#
    ))
    .is_err());
}

#[test]
fn rss_feed_date_rejects_impossible_iso_dates() {
    assert_eq!(rss_feed_date("2024-02-29"), "Thu, 29 Feb 2024 00:00:00 GMT");
    assert_eq!(rss_feed_date("2023-02-29"), "2023-02-29");
    assert_eq!(rss_feed_date("2024-04-31"), "2024-04-31");
    assert_eq!(rss_feed_date("2024-13-01"), "2024-13-01");
}

#[test]
fn favicon_config_parses_and_defaults_to_generated_asset() {
    let src = Path::new("/site/docs");
    let config = website_config_from_toml(r#"favicon = "assets/favicon.ico""#);
    let metadata = SiteMetadata::from_config(&config, src, ".calepin/favicon.svg").unwrap();
    assert_eq!(metadata.favicon.as_deref(), Some("assets/favicon.ico"));

    let config = website_config_from_toml("");
    let metadata = SiteMetadata::from_config(&config, src, ".calepin/favicon.svg").unwrap();
    assert_eq!(metadata.favicon.as_deref(), Some(".calepin/favicon.svg"));
}

#[test]
fn favicon_default_respects_custom_asset_dir() {
    let src = Path::new("/site/docs");
    let config = website_config_from_toml(r#"asset-dir = "_calepin""#);
    let metadata = SiteMetadata::from_config(&config, src, "_calepin/favicon.svg").unwrap();

    assert_eq!(metadata.favicon.as_deref(), Some("_calepin/favicon.svg"));
}

#[test]
fn icon_cache_respects_custom_asset_dir() {
    assert_eq!(
        website_icon_cache_dir(Path::new("_calepin")),
        PathBuf::from("_calepin/icons")
    );
}

#[test]
fn logo_and_favicon_paths_resolve_from_source_directory() {
    let src = Path::new("/site/docs");
    let config = website_config_from_toml(
        r#"
logo = "./assets/logo.svg"
favicon = "assets/favicon.ico"
"#,
    );

    let metadata = SiteMetadata::from_config(&config, src, ".calepin/favicon.svg").unwrap();

    assert_eq!(metadata.logo.as_deref(), Some("assets/logo.svg"));
    assert_eq!(metadata.favicon.as_deref(), Some("assets/favicon.ico"));

    let config = website_config_from_toml(r#"logo = "../logo.svg""#);
    let err = SiteMetadata::from_config(&config, src, ".calepin/favicon.svg").unwrap_err();
    assert!(err.to_string().contains("source directory"));

    let config = website_config_from_toml(r#"logo = ".""#);
    let err = SiteMetadata::from_config(&config, src, ".calepin/favicon.svg").unwrap_err();
    assert!(err.to_string().contains("source directory"));
}

#[test]
fn configured_languages_defaults_to_directory_per_language() {
    let src = Path::new("/site/docs");
    let config = WebsiteConfig {
        default_language: Some("en".to_string()),
        languages: BTreeMap::from([
            (
                "en".to_string(),
                LanguageConfig {
                    label: Some("English".to_string()),
                    content_dir: Some(PathBuf::from(".")),
                    ..LanguageConfig::default()
                },
            ),
            (
                "fr".to_string(),
                LanguageConfig {
                    label: Some("Français".to_string()),
                    ..LanguageConfig::default()
                },
            ),
        ]),
        ..WebsiteConfig::default()
    };

    let languages = configured_languages(src, &config).unwrap().unwrap();

    assert_eq!(languages[0].code, "en");
    assert_eq!(languages[0].content_dir, src);
    assert_eq!(languages[0].url_prefix, "");
    assert!(languages[0].is_default);
    assert_eq!(languages[1].code, "fr");
    assert_eq!(languages[1].content_dir, src.join("fr"));
    assert_eq!(languages[1].url_prefix, "fr");
    assert_eq!(languages[1].label, "Français");
}

#[test]
fn configured_languages_requires_explicit_default_for_multiple_languages() {
    let src = Path::new("/site/docs");
    let config = WebsiteConfig {
        languages: BTreeMap::from([
            ("en".to_string(), LanguageConfig::default()),
            ("fr".to_string(), LanguageConfig::default()),
        ]),
        ..WebsiteConfig::default()
    };

    let error = configured_languages(src, &config).unwrap_err();
    assert!(error.to_string().contains("default_language"));

    let single = WebsiteConfig {
        languages: BTreeMap::from([("fr".to_string(), LanguageConfig::default())]),
        ..WebsiteConfig::default()
    };
    let languages = configured_languages(src, &single).unwrap().unwrap();
    assert!(languages[0].is_default);
}

#[test]
fn configured_languages_rejects_url_prefix_escaping_output_directory() {
    let src = Path::new("/site/docs");
    let config = WebsiteConfig {
        default_language: Some("en".to_string()),
        languages: BTreeMap::from([(
            "en".to_string(),
            LanguageConfig {
                url_prefix: Some("../outside".to_string()),
                ..LanguageConfig::default()
            },
        )]),
        ..WebsiteConfig::default()
    };

    let error = configured_languages(src, &config).unwrap_err();
    assert!(error.to_string().contains("url_prefix"));
}

#[test]
fn configured_languages_rejects_content_dir_outside_source_directory() {
    let src = Path::new("/site/docs");
    for content_dir in [
        PathBuf::from("../outside"),
        std::env::current_dir().unwrap().join("outside"),
    ] {
        let config = WebsiteConfig {
            default_language: Some("en".to_string()),
            languages: BTreeMap::from([(
                "en".to_string(),
                LanguageConfig {
                    content_dir: Some(content_dir),
                    ..LanguageConfig::default()
                },
            )]),
            ..WebsiteConfig::default()
        };

        let error = configured_languages(src, &config).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("content_dir"));
        assert!(message.contains("source directory"));
    }
}

#[test]
fn configured_languages_rejects_duplicate_url_prefixes_after_cleaning() {
    let src = Path::new("/site/docs");
    let config = WebsiteConfig {
        default_language: Some("en".to_string()),
        languages: BTreeMap::from([
            ("en".to_string(), LanguageConfig::default()),
            (
                "fr".to_string(),
                LanguageConfig {
                    url_prefix: Some("/".to_string()),
                    ..LanguageConfig::default()
                },
            ),
        ]),
        ..WebsiteConfig::default()
    };

    let error = configured_languages(src, &config).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("url_prefix"));
    assert!(message.contains("en"));
    assert!(message.contains("fr"));
}

#[test]
fn configured_languages_rejects_duplicate_content_dirs() {
    let src = Path::new("/site/docs");
    let config = WebsiteConfig {
        default_language: Some("en".to_string()),
        languages: BTreeMap::from([
            ("en".to_string(), LanguageConfig::default()),
            (
                "fr".to_string(),
                LanguageConfig {
                    content_dir: Some(PathBuf::from(".")),
                    ..LanguageConfig::default()
                },
            ),
        ]),
        ..WebsiteConfig::default()
    };

    let error = configured_languages(src, &config).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("content_dir"));
    assert!(message.contains("duplicates"));
}

#[test]
fn configured_languages_validates_language_codes_and_trims_default_language() {
    let src = Path::new("/site/docs");
    let invalid = WebsiteConfig {
        default_language: Some(String::new()),
        languages: BTreeMap::from([(String::new(), LanguageConfig::default())]),
        ..WebsiteConfig::default()
    };

    let error = configured_languages(src, &invalid).unwrap_err();
    assert!(error.to_string().contains("language code"));

    let config = WebsiteConfig {
        default_language: Some(" en ".to_string()),
        languages: BTreeMap::from([
            ("en".to_string(), LanguageConfig::default()),
            ("fr".to_string(), LanguageConfig::default()),
        ]),
        ..WebsiteConfig::default()
    };

    let languages = configured_languages(src, &config).unwrap().unwrap();
    assert!(languages[0].is_default);
    assert_eq!(languages[0].code, "en");
}

#[test]
fn language_config_rejects_unknown_fields() {
    let error = try_website_config_from_toml(
        r#"
[languages.en]
content-dir = "docs"
"#,
    )
    .unwrap_err();

    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn discover_site_pages_does_not_treat_language_dirs_as_default_pages() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    fs::write(src.join("index.typ"), "= Home\n").unwrap();
    fs::write(src.join("about.typ"), "= About\n").unwrap();
    fs::create_dir_all(src.join("fr")).unwrap();
    fs::write(src.join("fr").join("index.typ"), "= Accueil\n").unwrap();
    fs::write(src.join("fr").join("about.typ"), "= À propos\n").unwrap();
    let languages = Some(vec![
        LanguageInfo {
            code: "en".to_string(),
            label: "English".to_string(),
            content_dir: src.to_path_buf(),
            url_prefix: String::new(),
            is_default: true,
        },
        LanguageInfo {
            code: "fr".to_string(),
            label: "Français".to_string(),
            content_dir: src.join("fr"),
            url_prefix: "fr".to_string(),
            is_default: false,
        },
    ]);

    let (_sections, files) = discover_site_pages(src, None, None, &languages).unwrap();
    let mut rel = files
        .iter()
        .map(|path| rel_posix(src, path))
        .collect::<Vec<_>>();
    rel.sort();

    assert_eq!(
        rel,
        vec!["about.typ", "fr/about.typ", "fr/index.typ", "index.typ"]
    );
}

#[test]
fn implicit_build_pages_include_root_home_and_fallback_outside_configured_navigation() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    fs::write(src.join("index.typ"), "= Home\n").unwrap();
    fs::write(src.join("404.typ"), "= Not Found\n").unwrap();
    fs::write(src.join("about.typ"), "= About\n").unwrap();
    let sidebar = SidebarConfig {
        section: vec![SidebarSectionConfig {
            item: vec![SidebarItemConfig {
                target: Some("about.typ".to_string()),
                ..SidebarItemConfig::default()
            }],
            ..SidebarSectionConfig::default()
        }],
        ..SidebarConfig::default()
    };

    let (_sections, files) = discover_site_pages(src, Some(&sidebar), None, &None).unwrap();
    assert_eq!(
        files
            .iter()
            .map(|path| rel_posix(src, path))
            .collect::<Vec<_>>(),
        vec!["about.typ"]
    );

    let mut build_files = files;
    build_files.extend(implicit_build_pages(src, &None));
    build_files.sort_by_key(|path| rel_posix(src, path));

    assert_eq!(
        build_files
            .iter()
            .map(|path| rel_posix(src, path))
            .collect::<Vec<_>>(),
        vec!["404.typ", "about.typ", "index.typ"]
    );
}

#[test]
fn implicit_build_pages_include_each_language_home_and_fallback() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    fs::write(src.join("index.typ"), "= Home\n").unwrap();
    fs::write(src.join("404.typ"), "= Not Found\n").unwrap();
    fs::create_dir_all(src.join("fr")).unwrap();
    fs::write(src.join("fr").join("index.typ"), "= Accueil\n").unwrap();
    fs::write(src.join("fr").join("404.typ"), "= Introuvable\n").unwrap();
    let languages = Some(vec![
        LanguageInfo {
            code: "en".to_string(),
            label: "English".to_string(),
            content_dir: src.to_path_buf(),
            url_prefix: String::new(),
            is_default: true,
        },
        LanguageInfo {
            code: "fr".to_string(),
            label: "Français".to_string(),
            content_dir: src.join("fr"),
            url_prefix: "fr".to_string(),
            is_default: false,
        },
    ]);

    let mut pages = implicit_build_pages(src, &languages);
    pages.sort_by_key(|path| rel_posix(src, path));

    assert_eq!(
        pages
            .iter()
            .map(|path| rel_posix(src, path))
            .collect::<Vec<_>>(),
        vec!["404.typ", "fr/404.typ", "fr/index.typ", "index.typ"]
    );
}

#[test]
fn pages_include_adds_build_only_pages() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    fs::write(src.join("index.typ"), "= Home\n").unwrap();
    fs::write(src.join("about.typ"), "= About\n").unwrap();
    fs::create_dir_all(src.join("landing")).unwrap();
    fs::write(src.join("landing").join("campaign.typ"), "= Campaign\n").unwrap();
    let pages = PagesConfig {
        include: vec!["landing/*.typ".to_string()],
        ..PagesConfig::default()
    };
    let sidebar = SidebarConfig {
        section: vec![SidebarSectionConfig {
            item: vec![SidebarItemConfig {
                target: Some("about.typ".to_string()),
                ..SidebarItemConfig::default()
            }],
            ..SidebarSectionConfig::default()
        }],
        ..SidebarConfig::default()
    };

    let (_sections, nav_files) =
        discover_site_pages(src, Some(&sidebar), Some(&pages), &None).unwrap();
    let include_files = discover_site_build_pages(src, Some(&pages), &None).unwrap();

    assert_eq!(
        nav_files
            .iter()
            .map(|path| rel_posix(src, path))
            .collect::<Vec<_>>(),
        vec!["about.typ"]
    );
    assert_eq!(
        include_files
            .iter()
            .map(|path| rel_posix(src, path))
            .collect::<Vec<_>>(),
        vec!["landing/campaign.typ"]
    );
}

#[test]
fn pages_exclude_removes_pages_from_navigation_and_includes() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    fs::write(src.join("index.typ"), "= Home\n").unwrap();
    fs::write(src.join("about.typ"), "= About\n").unwrap();
    fs::create_dir_all(src.join("drafts")).unwrap();
    fs::write(src.join("drafts").join("idea.typ"), "= Draft\n").unwrap();
    let pages = PagesConfig {
        include: vec!["drafts/*.typ".to_string()],
        exclude: vec!["drafts/**".to_string(), "about.typ".to_string()],
    };

    let (_sections, nav_files) = discover_site_pages(src, None, Some(&pages), &None).unwrap();
    let include_files = discover_site_build_pages(src, Some(&pages), &None).unwrap();
    let required = implicit_build_pages(src, &None);

    assert_eq!(
        nav_files
            .iter()
            .map(|path| rel_posix(src, path))
            .collect::<Vec<_>>(),
        vec!["index.typ"]
    );
    assert!(include_files.is_empty());
    assert_eq!(
        required
            .iter()
            .map(|path| rel_posix(src, path))
            .collect::<Vec<_>>(),
        vec!["index.typ"]
    );
}

#[test]
fn pages_exclude_prevents_typ_sources_from_being_copied_to_output() {
    if !command_available("typst") {
        return;
    }

    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    fs::create_dir_all(src.join("lib")).unwrap();
    std::fs::write(
        src.join("calepin.toml"),
        r#"
[pages]
exclude = ["lib/**"]
"#,
    )
    .unwrap();
    std::fs::write(
        src.join("lib/helpers.typ"),
        "#let greeting() = [Hello from helper]\n",
    )
    .unwrap();
    std::fs::write(
        src.join("index.typ"),
        "#set document(title: [Home])\n#import \"/lib/helpers.typ\": greeting\n#heading[Home]\n#greeting()\n",
    )
    .unwrap();

    let result = build_site(WebsiteBuildOptions {
        config: src.join("calepin.toml"),
        src: Some(src.to_path_buf()),
        out: Some(src.join("public")),
        parallelism: Some(1),
        render_pdf: Some(false),
        quiet: true,
        timeout: None,
        params: Vec::new(),
        typst_args: Vec::new(),
        incremental_inputs: None,
        clean: true,
        minify_html: false,
    })
    .unwrap();

    assert!(result.out_dir.join("index.html").is_file());
    assert!(!result.out_dir.join("lib/helpers.typ").exists());
}

#[test]
fn discover_static_files_includes_files_dirs_and_globs_then_excludes() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    fs::create_dir_all(src.join("assets/private")).unwrap();
    fs::create_dir_all(src.join("downloads")).unwrap();
    fs::write(src.join("assets/logo.svg"), "<svg></svg>").unwrap();
    fs::write(src.join("assets/private/draft.svg"), "<svg></svg>").unwrap();
    fs::write(src.join("downloads/manual.pdf"), "pdf").unwrap();
    fs::write(src.join("downloads/notes.txt"), "notes").unwrap();
    fs::write(src.join("robots.txt"), "User-agent: *").unwrap();
    fs::create_dir_all(src.join(".calepin")).unwrap();
    fs::write(src.join(".calepin/generated.css"), "body {}").unwrap();
    let config = StaticConfig {
        include: vec![
            "assets".to_string(),
            "downloads/*.pdf".to_string(),
            "robots.txt".to_string(),
            ".calepin/**".to_string(),
        ],
        exclude: vec!["assets/private/**".to_string()],
    };

    let files = discover_static_files(src, Some(&config)).unwrap();
    let rels = files
        .iter()
        .map(|path| rel_posix(src, path))
        .collect::<Vec<_>>();

    assert_eq!(
        rels,
        vec![
            "assets/logo.svg".to_string(),
            "downloads/manual.pdf".to_string(),
            "robots.txt".to_string()
        ]
    );
}

#[test]
fn discover_static_files_rejects_paths_outside_source_directory() {
    let config = StaticConfig {
        include: vec!["../secret.txt".to_string()],
        exclude: Vec::new(),
    };

    let err = discover_static_files(Path::new("/site/docs"), Some(&config)).unwrap_err();

    assert!(err.to_string().contains("source directory"));
}

#[test]
fn copy_static_files_preserves_source_relative_paths() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("docs");
    let out = temp.path().join("public");
    fs::create_dir_all(src.join("assets")).unwrap();
    fs::write(src.join("assets/logo.svg"), "<svg></svg>").unwrap();
    fs::write(src.join("robots.txt"), "User-agent: *").unwrap();
    let files = vec![src.join("assets/logo.svg"), src.join("robots.txt")];

    copy_static_files(&src, &out, &files).unwrap();

    assert_eq!(
        fs::read_to_string(out.join("assets/logo.svg")).unwrap(),
        "<svg></svg>"
    );
    assert_eq!(
        fs::read_to_string(out.join("robots.txt")).unwrap(),
        "User-agent: *"
    );
}

#[test]
fn build_page_info_uses_language_prefixes_slugs_and_translation_keys() {
    let src = Path::new("/site/docs");
    let en = PathBuf::from("/site/docs/about.typ");
    let fr = PathBuf::from("/site/docs/fr/about.typ");
    let languages = Some(vec![
        LanguageInfo {
            code: "en".to_string(),
            label: "English".to_string(),
            content_dir: src.to_path_buf(),
            url_prefix: String::new(),
            is_default: true,
        },
        LanguageInfo {
            code: "fr".to_string(),
            label: "Français".to_string(),
            content_dir: src.join("fr"),
            url_prefix: "fr".to_string(),
            is_default: false,
        },
    ]);
    let meta = PageMetaMap::from([(
        fr.clone(),
        page_meta_from_value(&serde_json::json!({"translation_key": "about", "slug": "a-propos"})),
    )]);
    let pdf_files = BTreeSet::from([fr.clone()]);

    let info = build_page_info(
        src,
        &[en.clone(), fr.clone()],
        &meta,
        &pdf_files,
        &languages,
    )
    .unwrap();

    assert_eq!(info[&en].language.as_deref(), Some("en"));
    assert_eq!(info[&en].translation_key, "about");
    assert_eq!(info[&en].href, "about.html");
    assert_eq!(info[&fr].language.as_deref(), Some("fr"));
    assert_eq!(info[&fr].translation_key, "about");
    assert_eq!(info[&fr].href, "fr/a-propos.html");
    assert_eq!(info[&fr].pdf_href.as_deref(), Some("fr/a-propos.pdf"));
}

#[test]
fn build_page_info_keeps_pdf_distinct_from_custom_url_with_extension() {
    let src = Path::new("/site/docs");
    let page = PathBuf::from("/site/docs/about.typ");
    let meta = PageMetaMap::from([(
        page.clone(),
        page_meta_from_value(&serde_json::json!({"url": "info/about.html"})),
    )]);
    let pdf_files = BTreeSet::from([page.clone()]);

    let info = build_page_info(src, std::slice::from_ref(&page), &meta, &pdf_files, &None).unwrap();

    assert_eq!(info[&page].href, "info/about.html");
    assert_eq!(info[&page].pdf_href.as_deref(), Some("info/about.pdf"));
}

#[test]
fn build_page_info_rejects_slug_and_url_escaping_output_directory() {
    let src = Path::new("/site/docs");
    let page = PathBuf::from("/site/docs/about.typ");

    for value in [
        serde_json::json!({"slug": "../escape"}),
        serde_json::json!({"slug": "/absolute"}),
        serde_json::json!({"url": "../escape.html"}),
    ] {
        let meta = PageMetaMap::from([(page.clone(), page_meta_from_value(&value))]);
        let error = build_page_info(
            src,
            std::slice::from_ref(&page),
            &meta,
            &BTreeSet::new(),
            &None,
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("output directory"),
            "expected rejection for {value}: {error}"
        );
    }
}

#[test]
fn sanitize_icon_svg_accepts_plain_icons_and_rejects_scripting_vectors() {
    let plain = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M3 12L12 3l9 9"/></svg>"#;
    assert_eq!(sanitize_icon_svg(plain, "home").unwrap(), plain);

    for bad in [
        r#"<svg><script>alert(1)</script></svg>"#,
        r#"<svg/onload=alert(1)></svg>"#,
        r#"<svg onclick="alert(1)"></svg>"#,
        r#"<svg ONLOAD = "alert(1)"></svg>"#,
        r#"<svg><a href="javascript:alert(1)">x</a></svg>"#,
        r#"<svg><use href="java&#x73;cript:alert(1)"></use></svg>"#,
        r#"<svg><use href="https://example.com/icon.svg#x"></use></svg>"#,
        r#"<svg><image href="https://example.com/icon.png"/></svg>"#,
        r#"<svg><path></svg>"#,
        r#"<svg><foreignObject></foreignObject></svg>"#,
        "not svg at all",
    ] {
        assert!(sanitize_icon_svg(bad, "home").is_err(), "accepted: {bad}");
    }
}

#[test]
fn translation_entries_are_relative_to_current_page() {
    let en = PathBuf::from("/site/docs/about.typ");
    let fr = PathBuf::from("/site/docs/fr/about.typ");
    let page_info = PageInfoMap::from([
        (
            en,
            PageInfo {
                language: Some("en".to_string()),
                translation_key: "about".to_string(),
                href: "about.html".to_string(),
                pdf_href: None,
            },
        ),
        (
            fr.clone(),
            PageInfo {
                language: Some("fr".to_string()),
                translation_key: "about".to_string(),
                href: "fr/a-propos.html".to_string(),
                pdf_href: None,
            },
        ),
    ]);
    let languages = vec![
        LanguageInfo {
            code: "en".to_string(),
            label: "English".to_string(),
            content_dir: PathBuf::from("/site/docs"),
            url_prefix: String::new(),
            is_default: true,
        },
        LanguageInfo {
            code: "fr".to_string(),
            label: "Français".to_string(),
            content_dir: PathBuf::from("/site/docs/fr"),
            url_prefix: "fr".to_string(),
            is_default: false,
        },
    ];

    let entries = translation_entries(
        "fr/a-propos.html",
        page_info.get(&fr).unwrap(),
        &page_info,
        &languages,
    );

    assert_eq!(entries[0].href, "../about.html");
    assert_eq!(entries[0].label, "English");
    assert!(!entries[0].active);
    assert_eq!(entries[1].href, "a-propos.html");
    assert!(entries[1].active);
}

#[test]
fn language_entries_include_all_languages_with_home_fallbacks() {
    let en = PathBuf::from("/site/docs/about.typ");
    let page_info = PageInfoMap::from([(
        en.clone(),
        PageInfo {
            language: Some("en".to_string()),
            translation_key: "about".to_string(),
            href: "about.html".to_string(),
            pdf_href: None,
        },
    )]);
    let languages = vec![
        LanguageInfo {
            code: "en".to_string(),
            label: "English".to_string(),
            content_dir: PathBuf::from("/site/docs"),
            url_prefix: String::new(),
            is_default: true,
        },
        LanguageInfo {
            code: "fr".to_string(),
            label: "Français".to_string(),
            content_dir: PathBuf::from("/site/docs/fr"),
            url_prefix: "fr".to_string(),
            is_default: false,
        },
    ];

    let entries = language_entries("about.html", page_info.get(&en), &page_info, &languages);

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].href, "about.html");
    assert!(entries[0].active);
    assert_eq!(entries[1].href, "fr/index.html");
    assert!(!entries[1].active);
}

#[test]
fn changed_typ_pages_skips_unchanged_hashes() {
    let temp = tempfile::tempdir().unwrap();
    let page = temp.path().join("index.typ");
    fs::write(&page, "= Home\n").unwrap();
    let current = test_build_result(temp.path(), std::slice::from_ref(&page));

    let changed = changed_typ_pages(&current, std::slice::from_ref(&page)).unwrap();

    assert_eq!(changed, Some(Vec::new()));
}

#[test]
fn changed_typ_pages_returns_modified_known_pages() {
    let temp = tempfile::tempdir().unwrap();
    let page = temp.path().join("index.typ");
    fs::write(&page, "= Home\n").unwrap();
    let current = test_build_result(temp.path(), std::slice::from_ref(&page));

    fs::write(&page, "= Updated\n").unwrap();
    let changed = changed_typ_pages(&current, std::slice::from_ref(&page)).unwrap();

    assert_eq!(changed, Some(vec![page]));
}

#[test]
fn changed_typ_pages_falls_back_for_structural_changes() {
    let temp = tempfile::tempdir().unwrap();
    let page = temp.path().join("index.typ");
    let asset = temp.path().join("assets").join("site.css");
    fs::create_dir_all(asset.parent().unwrap()).unwrap();
    fs::write(&page, "= Home\n").unwrap();
    fs::write(&asset, "body {}\n").unwrap();
    let current = test_build_result(temp.path(), std::slice::from_ref(&page));

    let changed = changed_typ_pages(&current, std::slice::from_ref(&asset)).unwrap();

    assert_eq!(changed, None);
}

#[test]
fn changed_typ_pages_falls_back_for_new_or_removed_pages() {
    let temp = tempfile::tempdir().unwrap();
    let page = temp.path().join("index.typ");
    let new_page = temp.path().join("new.typ");
    fs::write(&page, "= Home\n").unwrap();
    fs::write(&new_page, "= New\n").unwrap();
    let current = test_build_result(temp.path(), std::slice::from_ref(&page));

    let new_changed = changed_typ_pages(&current, std::slice::from_ref(&new_page)).unwrap();
    fs::remove_file(&page).unwrap();
    let removed_changed = changed_typ_pages(&current, std::slice::from_ref(&page)).unwrap();

    assert_eq!(new_changed, None);
    assert_eq!(removed_changed, None);
}

#[test]
fn reconcile_manifest_outputs_removes_only_stale_generated_files() {
    let temp = tempfile::tempdir().unwrap();
    let stale = temp.path().join("old.html");
    let current = temp.path().join("index.html");
    fs::write(&stale, "old").unwrap();
    fs::write(&current, "current").unwrap();
    let manifest = WebsiteManifest {
        outputs: vec!["old.html".to_string(), "index.html".to_string()],
        pagefind: None,
    };
    let expected = BTreeSet::from([current.clone()]);

    reconcile_manifest_outputs(temp.path(), &manifest, &expected).unwrap();

    assert!(!stale.exists());
    assert!(current.exists());
}

#[test]
fn write_sitemap_uses_absolute_page_urls() {
    let temp = tempfile::tempdir().unwrap();
    let hrefs = BTreeSet::from([
        "index.html".to_string(),
        "guide/index.html".to_string(),
        "guide/usage.html".to_string(),
    ]);

    write_sitemap(temp.path(), Some("https://example.com/project/"), &hrefs).unwrap();

    let sitemap = fs::read_to_string(temp.path().join("sitemap.xml")).unwrap();
    assert!(sitemap.contains("<loc>https://example.com/project/</loc>"));
    assert!(sitemap.contains("<loc>https://example.com/project/guide/</loc>"));
    assert!(sitemap.contains("<loc>https://example.com/project/guide/usage.html</loc>"));
}

#[test]
fn site_context_page_url_uses_directory_style_for_index_routes() {
    let site = SiteModel::new(
        vec![],
        MenusModel::default(),
        SiteMetadata {
            title: Some("Name".to_string()),
            description: None,
            base_url: Some("https://example.com/project".to_string()),
            logo: None,
            logo_alt: None,
            favicon: None,
        },
        true,
    );
    let empty_page_info = PageInfoMap::new();

    let home = site.theme_context("index.html", None, &empty_page_info, None, None);
    assert_eq!(home.page_url.as_deref(), Some("https://example.com/project/"));

    let section = site.theme_context("guide/index.html", None, &empty_page_info, None, None);
    assert_eq!(
        section.page_url.as_deref(),
        Some("https://example.com/project/guide/")
    );
}

#[test]
fn feed_items_include_only_dated_pages_sorted_newest_first() {
    let pages = serde_json::json!([
        {
            "href": "posts/old.html",
            "title": "Old",
            "meta": {"date": "2026-01-01", "summary": "Older"}
        },
        {
            "href": "about.html",
            "title": "About",
            "meta": {}
        },
        {
            "href": "posts/new.html",
            "title": "New",
            "meta": {"date": "2026-06-10", "authors": ["Ada", "Grace"]}
        }
    ]);

    let items = feed_items_from_pages(&pages, "https://example.com/site", None);

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].title, "New");
    assert_eq!(items[0].url, "https://example.com/site/posts/new.html");
    assert_eq!(items[0].author.as_deref(), Some("Ada, Grace"));
    assert_eq!(items[1].title, "Old");
}

#[test]
fn write_feeds_generates_atom_and_rss_from_dated_pages() {
    let temp = tempfile::tempdir().unwrap();
    let config = website_config_from_toml(
        r#"
title = "Example Site"
description = "Research updates"
base_url = "https://example.com/project"
generate_feeds = true

[feeds]
filenames = ["atom.xml", "rss.xml"]
"#,
    );
    let metadata = SiteMetadata::from_config(&config, temp.path(), ".calepin/favicon.svg").unwrap();
    let pages = serde_json::json!([
        {
            "href": "posts/first.html",
            "title": "First & Best",
            "meta": {
                "date": "2026-06-10",
                "summary": "A <short> update.",
                "author": "Ada Lovelace"
            }
        },
        {
            "href": "about.html",
            "title": "About",
            "meta": {}
        }
    ]);
    let targets = feed_targets(&config).unwrap();

    write_feeds(
        temp.path(),
        temp.path(),
        &config,
        metadata.base_url.as_deref(),
        &metadata,
        &pages,
        &targets,
    )
    .unwrap();

    let atom = fs::read_to_string(temp.path().join("atom.xml")).unwrap();
    assert!(atom.contains("<feed xmlns=\"http://www.w3.org/2005/Atom\">"));
    assert!(atom.contains("<title>First &amp; Best</title>"));
    assert!(atom.contains("https://example.com/project/posts/first.html"));
    assert!(atom.contains("A &lt;short&gt; update."));
    assert!(!atom.contains("about.html"));

    let rss = fs::read_to_string(temp.path().join("rss.xml")).unwrap();
    assert!(rss.contains("<rss version=\"2.0\">"));
    assert!(rss.contains("<title>First &amp; Best</title>"));
    assert!(rss.contains("<pubDate>Wed, 10 Jun 2026 00:00:00 GMT</pubDate>"));
    assert!(rss.contains("Ada Lovelace"));
}

#[test]
fn pagefind_index_writes_bundle_files_and_pagefind_relative_urls() {
    let temp = tempfile::tempdir().unwrap();
    let page = temp.path().join("guide").join("usage.html");
    fs::create_dir_all(page.parent().unwrap()).unwrap();
    fs::write(
            &page,
            r#"<!doctype html><html><body><main data-pagefind-body><h1>Guide</h1><p>Searchable content.</p></main></body></html>"#,
        )
        .unwrap();

    let pages = vec![(page, "guide/usage.html".to_string())];
    let outputs = write_pagefind_index(temp.path(), &pages).unwrap();

    assert!(outputs
        .iter()
        .any(|path| path.ends_with(Path::new("pagefind/pagefind-component-ui.js"))));
    assert!(outputs
        .iter()
        .any(|path| path.ends_with(Path::new("pagefind/pagefind-component-ui.css"))));
    assert!(outputs.iter().any(|path| path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".pf_fragment"))));
}

#[test]
fn pagefind_signature_tracks_rendered_html_and_urls() {
    let temp = tempfile::tempdir().unwrap();
    let page = temp.path().join("index.html");
    fs::write(&page, "<main data-pagefind-body>one</main>").unwrap();
    let pages = vec![(page.clone(), "index.html".to_string())];

    let original = pagefind_signature(temp.path(), &pages).unwrap();
    fs::write(&page, "<main data-pagefind-body>two</main>").unwrap();
    let content_changed = pagefind_signature(temp.path(), &pages).unwrap();
    let url_changed =
        pagefind_signature(temp.path(), &[(page, "renamed.html".to_string())]).unwrap();

    assert_ne!(original, content_changed);
    assert_ne!(content_changed, url_changed);
}

#[test]
fn cached_pagefind_outputs_require_matching_signature_and_files() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("pagefind").join("pagefind.js");
    fs::create_dir_all(output.parent().unwrap()).unwrap();
    fs::write(&output, "bundle").unwrap();
    let manifest = WebsiteManifest {
        outputs: vec!["index.html".to_string()],
        pagefind: Some(PagefindManifest {
            signature: 42,
            outputs: vec!["pagefind/pagefind.js".to_string()],
        }),
    };

    assert!(cached_pagefind_outputs(temp.path(), &manifest, 42)
        .unwrap()
        .is_some());
    assert!(cached_pagefind_outputs(temp.path(), &manifest, 7)
        .unwrap()
        .is_none());
    fs::remove_file(output).unwrap();
    assert!(cached_pagefind_outputs(temp.path(), &manifest, 42)
        .unwrap()
        .is_none());
}

#[test]
fn manifest_output_paths_rejects_unsafe_paths() {
    let temp = tempfile::tempdir().unwrap();

    let error =
        manifest_output_paths(temp.path(), &["../evil/pagefind.js".to_string()]).unwrap_err();

    assert!(error.to_string().contains("invalid Pagefind output path"));
}

#[test]
fn pagefind_signature_rejects_paths_outside_output_directory() {
    let temp = tempfile::tempdir().unwrap();
    let out_dir = temp.path().join("out");
    fs::create_dir_all(&out_dir).unwrap();
    let outside_page = temp.path().join("outside.html");
    fs::write(&outside_page, "<main data-pagefind-body>external</main>").unwrap();

    let error =
        pagefind_signature(&out_dir, &[(outside_page, "outside.html".to_string())]).unwrap_err();

    assert!(error.to_string().contains("invalid pagefind input path"));
}

#[cfg(target_os = "windows")]
#[test]
fn manifest_output_paths_rejects_windows_prefixed_paths() {
    let temp = tempfile::tempdir().unwrap();

    let error = manifest_output_paths(
        temp.path(),
        &["C:\\Program Files\\pagefind\\pagefind.js".to_string()],
    )
    .unwrap_err();

    assert!(error.to_string().contains("invalid Pagefind output path"));
}

#[test]
fn write_robots_uses_default_template_and_sitemap_url() {
    let temp = tempfile::tempdir().unwrap();
    let config = website_config_from_toml(r#"base_url = "https://example.com/project""#);

    write_robots(
        temp.path(),
        temp.path(),
        &config,
        Some("https://example.com/project"),
    )
    .unwrap();

    assert_eq!(
        fs::read_to_string(temp.path().join("robots.txt")).unwrap(),
        "User-agent: *\nAllow: /\nSitemap: https://example.com/project/sitemap.xml\n"
    );
}

#[test]
fn write_robots_leaves_existing_file_when_disabled() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("robots.txt"), "old").unwrap();
    let config = website_config_from_toml("robots = false");

    write_robots(
        temp.path(),
        temp.path(),
        &config,
        Some("https://example.com"),
    )
    .unwrap();

    assert_eq!(
        fs::read_to_string(temp.path().join("robots.txt")).unwrap(),
        "old"
    );
}

#[test]
fn write_robots_uses_template_override_with_includes_and_config() {
    let temp = tempfile::tempdir().unwrap();
    let templates = temp.path().join("templates");
    fs::create_dir_all(templates.join("partials")).unwrap();
    fs::write(
        templates.join("base.txt"),
        "{% block body %}{% endblock %}{% include \"partials/footer.txt\" %}",
    )
    .unwrap();
    fs::write(
        templates.join("partials/footer.txt"),
        "Host: {{ config.base_url }}\n",
    )
    .unwrap();
    fs::write(
            templates.join("robots.txt"),
            "{% extends \"base.txt\" %}{% block body %}User-agent: *\nDisallow: /drafts/\n{% endblock %}",
        )
        .unwrap();
    let config = website_config_from_toml(r#"base_url = "https://example.com""#);

    write_robots(
        temp.path(),
        temp.path(),
        &config,
        Some("https://example.com"),
    )
    .unwrap();

    assert_eq!(
        fs::read_to_string(temp.path().join("robots.txt")).unwrap(),
        "User-agent: *\nDisallow: /drafts/\nHost: https://example.com"
    );
}

#[test]
fn theme_context_rewrites_brand_urls_relative_to_current_page() {
    let site = SiteModel::new(
        vec![NavSectionModel {
            language: None,
            title: Some("Guide".to_string()),
            items: vec![NavItemModel {
                language: None,
                href: "guide/usage.html".to_string(),
                label: "Usage".to_string(),
                label_html: html_escape("Usage"),
            }],
        }],
        MenusModel::default(),
        SiteMetadata {
            title: Some("Example".to_string()),
            description: None,
            base_url: None,
            logo: Some("assets/logo.svg".to_string()),
            logo_alt: Some("Example".to_string()),
            favicon: Some("assets/favicon.ico".to_string()),
        },
        true,
    );

    let context = site.theme_context("guide/usage.html", None, &PageInfoMap::new(), None, None);

    assert_eq!(context.logo.as_deref(), Some("../assets/logo.svg"));
    assert_eq!(context.home_url.as_deref(), Some("../index.html"));
    assert_eq!(context.favicon.as_deref(), Some("../assets/favicon.ico"));
    assert_eq!(context.logo_alt.as_deref(), Some("Example"));
}

#[test]
fn theme_context_includes_global_sidebar_sections_with_language_specific_current_page() {
    let en = PathBuf::from("/site/docs/en.typ");
    let site = SiteModel::new(
        vec![
            NavSectionModel {
                language: None,
                title: Some("Global".to_string()),
                items: vec![NavItemModel {
                    language: None,
                    href: "about.html".to_string(),
                    label: "About".to_string(),
                    label_html: html_escape("About"),
                }],
            },
            NavSectionModel {
                language: Some("en".to_string()),
                title: Some("English".to_string()),
                items: vec![NavItemModel {
                    language: Some("en".to_string()),
                    href: "guide/usage.html".to_string(),
                    label: "Usage".to_string(),
                    label_html: html_escape("Usage"),
                }],
            },
            NavSectionModel {
                language: Some("fr".to_string()),
                title: Some("Français".to_string()),
                items: vec![NavItemModel {
                    language: Some("fr".to_string()),
                    href: "fr/guide/usage.html".to_string(),
                    label: "Utilisation".to_string(),
                    label_html: html_escape("Utilisation"),
                }],
            },
        ],
        MenusModel::default(),
        SiteMetadata::default(),
        true,
    );
    let page_info = PageInfoMap::from([(
        en.clone(),
        PageInfo {
            language: Some("en".to_string()),
            translation_key: "guide-usage".to_string(),
            href: "guide/usage.html".to_string(),
            pdf_href: None,
        },
    )]);

    let context = site.theme_context(
        "guide/usage.html",
        page_info.get(&en),
        &page_info,
        None,
        None,
    );

    assert_eq!(context.sidebar_sections.len(), 2);
    assert_eq!(context.sidebar_sections[0].title.as_deref(), Some("Global"));
    assert_eq!(
        context.sidebar_sections[1].title.as_deref(),
        Some("English")
    );
    assert_eq!(context.sidebar.len(), 2);
    assert_eq!(context.sidebar[0].href, "../about.html");
    assert_eq!(context.sidebar[1].href, "guide/usage.html");
}

#[test]
fn theme_context_rewrites_nav_urls_relative_to_current_page() {
    let site = SiteModel::new(
        vec![NavSectionModel {
            language: None,
            title: None,
            items: vec![
                NavItemModel {
                    language: None,
                    href: "index.html".to_string(),
                    label: "Home".to_string(),
                    label_html: html_escape("Home"),
                },
                NavItemModel {
                    language: None,
                    href: "publications/index.html".to_string(),
                    label: "Publications".to_string(),
                    label_html: html_escape("Publications"),
                },
                NavItemModel {
                    language: None,
                    href: "posts/welcome.html".to_string(),
                    label: "Welcome".to_string(),
                    label_html: html_escape("Welcome"),
                },
            ],
        }],
        MenusModel::default(),
        SiteMetadata::default(),
        true,
    );

    let context = site.theme_context("posts/welcome.html", None, &PageInfoMap::new(), None, None);
    let hrefs = context
        .sidebar
        .iter()
        .map(|item| item.href.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        hrefs,
        vec![
            "../index.html",
            "../publications/index.html",
            "welcome.html"
        ]
    );
    assert!(context.sidebar[2].active);
}

#[test]
fn theme_context_marks_section_containing_current_page_active() {
    let section = |title: &str, href: &str| NavSectionModel {
        language: None,
        title: Some(title.to_string()),
        items: vec![NavItemModel {
            language: None,
            href: href.to_string(),
            label: title.to_string(),
            label_html: html_escape(title),
        }],
    };
    let site = SiteModel::new(
        vec![
            section("Guide", "guide/usage.html"),
            section("Reference", "reference/cli.html"),
        ],
        MenusModel::default(),
        SiteMetadata::default(),
        true,
    );

    let context = site.theme_context("reference/cli.html", None, &PageInfoMap::new(), None, None);

    assert!(context.sidebar_fold);
    assert!(!context.sidebar_sections[0].active);
    assert!(context.sidebar_sections[1].active);
}

#[test]
fn page_relative_url_rewrites_asset_paths_for_nested_pages() {
    let asset = ".calepin/pages.json";
    assert_eq!(
        page_relative_url("guide/usage.html", asset),
        "../.calepin/pages.json"
    );
    assert_eq!(
        page_relative_url("guide/usage.html", "guide/advanced.html"),
        "advanced.html"
    );
    assert_eq!(
        page_relative_url("posts/welcome.html", "publications/index.html"),
        "../publications/index.html"
    );
    assert_eq!(
        page_relative_url("index.html", asset),
        ".calepin/pages.json"
    );
}

#[test]
fn shared_theme_init_script_is_inlined() {
    if !command_available("typst") {
        return;
    }

    let dir = tempdir_in_manifest("calepin-website-test-");
    let root = dir.path();
    let src = root.join("docs");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::create_dir_all(src.join("sub")).unwrap();
    std::fs::write(
        root.join("calepin.toml"),
        r#"
        theme = "calepin"
        "#,
    )
    .unwrap();
    std::fs::write(src.join("index.typ"), "#set document(title: [Home])\nHome").unwrap();
    std::fs::write(
        src.join("sub").join("index.typ"),
        "#set document(title: [Sub])\nSub",
    )
    .unwrap();

    let result = build_site(WebsiteBuildOptions {
        config: root.join("calepin.toml"),
        src: Some(src),
        out: Some(root.join("out")),
        parallelism: Some(1),
        render_pdf: Some(false),
        quiet: true,
        timeout: None,
        params: Vec::new(),
        typst_args: Vec::new(),
        incremental_inputs: None,
        clean: true,
        minify_html: false,
    })
    .unwrap();

    let root_output = result.out_dir.join("index.html");
    let nested_output = result.out_dir.join("sub").join("index.html");
    let root_html = std::fs::read_to_string(root_output).unwrap();
    let nested_html = std::fs::read_to_string(nested_output).unwrap();

    assert!(root_html.contains("data-calepin-theme-storage-key"));
    assert!(nested_html.contains("data-calepin-theme-storage-key"));
    assert!(!root_html.contains("src=\".calepin/theme-init.js\""));
    assert!(!nested_html.contains("src=\"../.calepin/theme-init.js\""));
}

#[test]
fn theme_context_exposes_pagefind_assets_when_search_enabled() {
    let site = SiteModel::new(
        Vec::new(),
        MenusModel::default(),
        SiteMetadata::default(),
        true,
    );

    let context = site.theme_context(
        "guide/usage.html",
        None,
        &PageInfoMap::new(),
        None,
        Some(SearchEngine::Pagefind),
    );
    let pagefind = context.pagefind.expect("Pagefind search context");

    assert_eq!(pagefind.css, "../pagefind/pagefind-component-ui.css");
    assert_eq!(pagefind.js, "../pagefind/pagefind-component-ui.js");
    assert_eq!(pagefind.bundle, "../pagefind");
}

#[test]
fn theme_key_resolves_local_directory_against_config_dir() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("themes/my-theme/layouts")).unwrap();
    std::fs::write(
        temp.path().join("themes/my-theme/layouts/webpage.html"),
        "{{ doc.body }}",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("calepin.toml"),
        r#"theme = "themes/my-theme""#,
    )
    .unwrap();
    let config =
        crate::config::CalepinConfig::load(temp.path(), Some(&temp.path().join("calepin.toml")))
            .unwrap();

    assert_eq!(
        config.theme_selection().unwrap(),
        Some(crate::theme::ThemeSelection::Dir(
            temp.path().join("themes/my-theme")
        ))
    );
}

#[test]
fn sidebar_item_config_rejects_labels() {
    let err = try_website_config_from_toml(
        r#"
[sidebar]

[[sidebar.section]]
title = "Guide"

  [[sidebar.section.item]]
  target = "install.typ"
  label = "Install"
"#,
    )
    .unwrap_err();

    assert!(err.to_string().contains("unknown field `label`"));
}

#[test]
fn navigation_config_rejects_icon_fields() {
    let sidebar_err = try_website_config_from_toml(
        r#"
[sidebar]

[[sidebar.section]]
title = "Start"

  [[sidebar.section.item]]
  target = "index.typ"
  icon = "home"
"#,
    )
    .unwrap_err();
    assert!(sidebar_err.to_string().contains("unknown field `icon`"));

    let menu_err = try_website_config_from_toml(
        r#"
[menus]

[[menus.main]]
target = "index.typ"
label = "Home"
icon = "home"
"#,
    )
    .unwrap_err();
    assert!(menu_err.to_string().contains("unknown field `icon`"));
}

#[test]
fn nav_from_plans_uses_metadata_title_then_stem() {
    let src = Path::new("/site/docs");
    let titled = PathBuf::from("/site/docs/b-page.typ");
    let bare = PathBuf::from("/site/docs/c_page.typ");
    let sections = vec![NavSectionPlan {
        language: None,
        title: Some("Guide".to_string()),
        items: vec![
            NavItemPlan {
                path: Some(titled.clone()),
                url: None,
                configured_label: None,
                configured_aria_label: None,
                weight: None,
            },
            NavItemPlan {
                path: Some(bare.clone()),
                url: None,
                configured_label: None,
                configured_aria_label: None,
                weight: None,
            },
        ],
    }];
    let meta = PageMetaMap::from([(
        titled.clone(),
        PageMeta {
            title: Some("From Metadata".to_string()),
            ..PageMeta::default()
        },
    )]);

    let files = vec![titled.clone(), bare.clone()];
    let page_info = test_page_info(src, &files, &BTreeSet::new());
    let icon_temp = tempfile::tempdir().unwrap();
    let mut icon_cache = IconCache::new(icon_temp.path(), Path::new(ICON_CACHE_DIR));
    let nav = nav_from_plans(&sections, &meta, &page_info, &mut icon_cache).unwrap();

    let labels = nav[0]
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(labels, vec!["From Metadata", "c page"]);
}

#[test]
fn menu_config_parses_named_menus_and_rejects_navbar() {
    let config = website_config_from_toml(
        r#"
[menus]

[[menus.main]]
target = "index.typ"
label = "Home"
weight = 20

[[menus.social]]
target = "https://github.com/example/project"
label = "{icon:github} GitHub"
weight = 10
"#,
    );

    assert_eq!(config.menus["main"][0].target.as_deref(), Some("index.typ"));
    assert_eq!(config.menus["main"][0].label.as_deref(), Some("Home"));
    assert_eq!(config.menus["main"][0].weight, Some(20));
    assert_eq!(
        config.menus["social"][0].target.as_deref(),
        Some("https://github.com/example/project")
    );

    let err = try_website_config_from_toml(
        r#"
[navbar]

[[navbar.item]]
position = "right"
target = "index.typ"
"#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("unknown field `navbar`"));
}

#[test]
fn footer_config_parses_items() {
    let config = website_config_from_toml(
        r#"
[footer]

[[footer.item]]
label = "© 2026 Example"

[[footer.item]]
target = "https://example.com/privacy"
label = "Privacy"
aria-label = "Privacy policy"
weight = 10
"#,
    );

    let footer = config.footer.as_ref().unwrap();
    assert_eq!(footer.item[0].label.as_deref(), Some("© 2026 Example"));
    assert_eq!(
        footer.item[1].target.as_deref(),
        Some("https://example.com/privacy")
    );
    assert_eq!(footer.item[1].label.as_deref(), Some("Privacy"));
    assert_eq!(footer.item[1].aria_label.as_deref(), Some("Privacy policy"));
    assert_eq!(footer.item[1].weight, Some(10));
}

#[test]
fn discover_site_menus_resolves_footer_config_items() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    fs::write(src.join("index.typ"), "= Home\n").unwrap();

    let config = website_config_from_toml(
        r#"
[footer]

[[footer.item]]
target = "index.typ"
label = "Docs"

[[footer.item]]
label = "© 2026 Example"
"#,
    );

    let (plan, files) =
        discover_site_menus(src, &config.menus, config.footer.as_ref(), None, &None).unwrap();

    assert_eq!(
        plan.items["footer"][0].path.as_deref(),
        Some(src.join("index.typ").as_path())
    );
    assert_eq!(plan.items["footer"][1].path, None);
    assert_eq!(plan.items["footer"][1].url, None);
    assert_eq!(
        plan.items["footer"][1].configured_label.as_deref(),
        Some("© 2026 Example")
    );
    assert_eq!(files, vec![src.join("index.typ").as_path()]);
}

#[test]
fn discover_site_menus_rejects_legacy_footer_menu_config() {
    let config = website_config_from_toml(
        r#"
[[menus.footer]]
label = "© 2026 Example"
"#,
    );

    let err = discover_site_menus(
        Path::new("/site/docs"),
        &config.menus,
        config.footer.as_ref(),
        None,
        &None,
    )
    .unwrap_err();

    assert!(err
        .to_string()
        .contains("use `[[footer.item]]` instead of `[[menus.footer]]`"));
}

#[test]
fn discover_menus_resolves_named_menus_and_orders_by_weight() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    fs::write(src.join("index.typ"), "= Home\n").unwrap();
    fs::create_dir_all(src.join("guide")).unwrap();
    fs::write(src.join("guide").join("usage.typ"), "= Usage\n").unwrap();
    let menus = BTreeMap::from([
        (
            "main".to_string(),
            vec![
                MenuItemConfig {
                    target: Some("index.typ".to_string()),
                    label: Some("Home".to_string()),
                    weight: Some(20),
                    ..MenuItemConfig::default()
                },
                MenuItemConfig {
                    glob: Some("guide/*.typ".to_string()),
                    weight: Some(10),
                    ..MenuItemConfig::default()
                },
            ],
        ),
        (
            "social".to_string(),
            vec![MenuItemConfig {
                target: Some("https://github.com/example/project".to_string()),
                label: Some("GitHub".to_string()),
                ..MenuItemConfig::default()
            }],
        ),
    ]);

    let (plan, files) = discover_menus(src, &menus, None).unwrap();

    assert_eq!(
        plan.items["main"][0].path.as_deref(),
        Some(src.join("guide/usage.typ").as_path())
    );
    assert_eq!(
        plan.items["main"][1].path.as_deref(),
        Some(src.join("index.typ").as_path())
    );
    assert_eq!(
        plan.items["social"][0].url.as_deref(),
        Some("https://github.com/example/project")
    );
    assert_eq!(
        files
            .iter()
            .map(|path| rel_posix(src, path))
            .collect::<Vec<_>>(),
        vec!["guide/usage.typ", "index.typ"]
    );
}

#[test]
fn discover_menus_allows_footer_items_without_targets() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    fs::write(src.join("index.typ"), "= Home\n").unwrap();
    let menus = BTreeMap::from([(
        "footer".to_string(),
        vec![
            MenuItemConfig {
                target: Some("index.typ".to_string()),
                label: Some("Docs".to_string()),
                ..MenuItemConfig::default()
            },
            MenuItemConfig {
                label: Some("© 2026 Example".to_string()),
                ..MenuItemConfig::default()
            },
        ],
    )]);

    let (plan, files) = discover_menus(src, &menus, None).unwrap();

    assert_eq!(
        plan.items["footer"][0].path.as_deref(),
        Some(src.join("index.typ").as_path())
    );
    assert_eq!(plan.items["footer"][1].path, None);
    assert_eq!(plan.items["footer"][1].url, None);
    assert_eq!(
        plan.items["footer"][1].configured_label.as_deref(),
        Some("© 2026 Example")
    );
    assert_eq!(files, vec![src.join("index.typ").as_path()]);
}

#[test]
fn discover_menus_rejects_linkless_items_for_non_footer_menus() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    let menus = BTreeMap::from([(
        "main".to_string(),
        vec![MenuItemConfig {
            label: Some("Broken".to_string()),
            ..MenuItemConfig::default()
        }],
    )]);

    let err = discover_menus(src, &menus, None).unwrap_err();

    assert!(err.to_string().contains("item must set path, glob, or url"));
}

#[test]
fn discover_menus_rejects_invalid_menu_names() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    let menus = BTreeMap::from([("Main Nav".to_string(), Vec::new())]);

    let err = discover_menus(src, &menus, None).unwrap_err();

    assert!(err.to_string().contains("invalid menu name `Main Nav`"));
}

#[test]
fn menus_from_plan_expands_label_icon_tokens_and_local_svg_paths() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    let home = src.join("index.typ");
    fs::write(&home, "= Home\n").unwrap();
    let icon_path = src.join("assets/icons/home.svg");
    fs::create_dir_all(icon_path.parent().unwrap()).unwrap();
    fs::write(
            &icon_path,
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M3 12L12 3l9 9"/></svg>"#,
        )
        .unwrap();
    let plan = MenusPlan {
        items: BTreeMap::from([(
            "main".to_string(),
            vec![MenuItemPlan {
                path: Some(home.clone()),
                url: None,
                configured_label: Some("{icon:assets/icons/home.svg} Home".to_string()),
                configured_aria_label: None,
                weight: None,
            }],
        )]),
    };
    let meta = PageMetaMap::new();
    let page_info = test_page_info(src, std::slice::from_ref(&home), &BTreeSet::new());
    let mut icon_cache = IconCache::new(src, Path::new(ICON_CACHE_DIR));

    let menus = menus_from_plan(&plan, &meta, &page_info, &mut icon_cache).unwrap();

    assert_eq!(menus.items["main"][0].label, "Home");
    assert!(menus.items["main"][0]
        .label_html
        .contains(r#"<span class="calepin-nav-icon">"#));
    assert!(menus.items["main"][0]
        .label_html
        .contains("viewBox=\"0 0 24 24\""));
    assert!(menus.items["main"][0].label_html.ends_with(" Home"));
}

#[test]
fn local_icon_paths_must_stay_inside_source_directory_and_be_safe() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    let outside = temp.path().parent().unwrap().join("outside-icon.svg");
    fs::write(&outside, r#"<svg viewBox="0 0 1 1"></svg>"#).unwrap();
    let mut icon_cache = IconCache::new(src, Path::new(ICON_CACHE_DIR));

    let outside_err = nav_label_html("{icon:../outside-icon.svg}", &mut icon_cache)
        .unwrap_err()
        .to_string();
    assert!(outside_err.contains("must stay inside the source directory"));

    let missing_html = nav_label_html("{icon:missing.svg} Missing", &mut icon_cache).unwrap();
    assert_eq!(missing_html, " Missing");

    let unsafe_path = src.join("unsafe.svg");
    fs::write(&unsafe_path, r#"<svg onload="alert(1)"></svg>"#).unwrap();
    let unsafe_html = nav_label_html("{icon:unsafe.svg} Unsafe", &mut icon_cache).unwrap();
    assert_eq!(unsafe_html, " Unsafe");
}

#[cfg(unix)]
#[test]
fn local_icon_symlinks_must_stay_inside_source_directory() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("site");
    fs::create_dir_all(src.join("assets/icons")).unwrap();
    let outside = temp.path().join("outside.svg");
    fs::write(&outside, r#"<svg viewBox="0 0 1 1"></svg>"#).unwrap();
    symlink(&outside, src.join("assets/icons/outside.svg")).unwrap();
    let mut icon_cache = IconCache::new(&src, Path::new(ICON_CACHE_DIR));

    let error = nav_label_html("{icon:assets/icons/outside.svg} Escape", &mut icon_cache)
        .unwrap_err()
        .to_string();

    assert!(error.contains("must stay inside the source directory"));
}

#[test]
fn discover_pages_resolves_sidebar_page_targets() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    fs::write(src.join("index.typ"), "= Home\n").unwrap();
    let sidebar = SidebarConfig {
        section: vec![SidebarSectionConfig {
            item: vec![SidebarItemConfig {
                target: Some("index.typ".to_string()),
                ..SidebarItemConfig::default()
            }],
            ..SidebarSectionConfig::default()
        }],
        ..SidebarConfig::default()
    };

    let (sections, files) = discover_pages(src, Some(&sidebar), None, None).unwrap();

    assert_eq!(
        files
            .iter()
            .map(|path| rel_posix(src, path))
            .collect::<Vec<_>>(),
        vec!["index.typ"]
    );
    assert_eq!(
        sections[0].items[0].path.as_deref(),
        Some(src.join("index.typ").as_path())
    );
}

#[test]
fn discover_pages_rejects_sidebar_targets_outside_source_directory() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("site");
    fs::create_dir_all(&src).unwrap();
    fs::write(temp.path().join("outside.typ"), "= Outside\n").unwrap();
    let sidebar = SidebarConfig {
        section: vec![SidebarSectionConfig {
            item: vec![SidebarItemConfig {
                target: Some("../outside.typ".to_string()),
                ..SidebarItemConfig::default()
            }],
            ..SidebarSectionConfig::default()
        }],
        ..SidebarConfig::default()
    };

    let err = discover_pages(&src, Some(&sidebar), None, None).unwrap_err();

    assert!(err.to_string().contains("source directory"));
}

#[test]
fn discover_pages_applies_pages_exclude_to_explicit_sidebar_targets() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    fs::write(src.join("about.typ"), "= About\n").unwrap();
    let pages = PagesConfig {
        exclude: vec!["about.typ".to_string()],
        ..PagesConfig::default()
    };
    let sidebar = SidebarConfig {
        section: vec![SidebarSectionConfig {
            item: vec![SidebarItemConfig {
                target: Some("about.typ".to_string()),
                ..SidebarItemConfig::default()
            }],
            ..SidebarSectionConfig::default()
        }],
        ..SidebarConfig::default()
    };

    let (sections, files) = discover_pages(&src, Some(&sidebar), Some(&pages), None).unwrap();

    assert!(files.is_empty());
    assert!(sections[0].items.is_empty());
}

#[test]
fn discover_pages_rejects_sidebar_external_targets() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    let sidebar = SidebarConfig {
        section: vec![SidebarSectionConfig {
            item: vec![SidebarItemConfig {
                target: Some("https://example.com".to_string()),
                ..SidebarItemConfig::default()
            }],
            ..SidebarSectionConfig::default()
        }],
        ..SidebarConfig::default()
    };

    let err = discover_pages(src, Some(&sidebar), None, None).unwrap_err();

    assert!(err
        .to_string()
        .contains("sidebar target must point to a .typ source page"));
}

#[test]
fn discover_pages_rejects_target_combined_with_glob() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    fs::write(src.join("index.typ"), "= Home\n").unwrap();
    let sidebar = SidebarConfig {
        section: vec![SidebarSectionConfig {
            item: vec![SidebarItemConfig {
                target: Some("index.typ".to_string()),
                glob: Some("*.typ".to_string()),
            }],
            ..SidebarSectionConfig::default()
        }],
        ..SidebarConfig::default()
    };

    let err = discover_pages(src, Some(&sidebar), None, None).unwrap_err();

    assert!(err
        .to_string()
        .contains("sidebar target items cannot also set glob"));
}

#[test]
fn menu_config_rejects_unknown_item_fields() {
    let err = toml::from_str::<MenuItemConfig>(
        r#"
            behavior = "theme"
            "#,
    )
    .unwrap_err();

    assert!(err.to_string().contains("unknown field `behavior`"));
}

#[test]
fn discover_menus_rejects_target_combined_with_glob() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    fs::write(src.join("index.typ"), "= Home\n").unwrap();
    let menus = BTreeMap::from([(
        "main".to_string(),
        vec![MenuItemConfig {
            target: Some("https://example.com".to_string()),
            glob: Some("*.typ".to_string()),
            ..MenuItemConfig::default()
        }],
    )]);

    let err = discover_menus(src, &menus, None).unwrap_err();

    assert!(err
        .to_string()
        .contains("menu `main` target items cannot also set glob"));
}

#[test]
fn discover_menus_rejects_targets_outside_source_directory() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("site");
    fs::create_dir_all(&src).unwrap();
    fs::write(temp.path().join("outside.typ"), "= Outside\n").unwrap();
    let menus = BTreeMap::from([(
        "main".to_string(),
        vec![MenuItemConfig {
            target: Some("../outside.typ".to_string()),
            ..MenuItemConfig::default()
        }],
    )]);

    let err = discover_menus(&src, &menus, None).unwrap_err();

    assert!(err.to_string().contains("source directory"));
}

#[test]
fn discover_site_build_pages_rejects_include_paths_outside_source_directory() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("site");
    fs::create_dir_all(&src).unwrap();
    fs::write(temp.path().join("outside.typ"), "= Outside\n").unwrap();
    let pages = PagesConfig {
        include: vec!["../outside.typ".to_string()],
        ..PagesConfig::default()
    };

    let err = discover_site_build_pages(&src, Some(&pages), &None).unwrap_err();

    assert!(err.to_string().contains("source directory"));
}

#[test]
fn discover_menus_rejects_unsafe_literal_url_targets() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    let menus = BTreeMap::from([(
        "main".to_string(),
        vec![MenuItemConfig {
            target: Some("javascript:alert(1)".to_string()),
            label: Some("Bad".to_string()),
            ..MenuItemConfig::default()
        }],
    )]);

    let err = discover_menus(src, &menus, None).unwrap_err();

    assert!(err.to_string().contains("unsafe URL"));
}

#[test]
fn discover_menus_keeps_absolute_url_targets_ending_in_typ_as_urls() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path();
    let menus = BTreeMap::from([(
        "main".to_string(),
        vec![MenuItemConfig {
            target: Some("https://example.com/template.typ".to_string()),
            label: Some("Template".to_string()),
            ..MenuItemConfig::default()
        }],
    )]);

    let (plan, files) = discover_menus(src, &menus, None).unwrap();

    assert!(files.is_empty());
    assert_eq!(
        plan.items["main"][0].url.as_deref(),
        Some("https://example.com/template.typ")
    );
}

#[test]
fn menus_from_plan_uses_page_metadata_and_external_labels() {
    let src = Path::new("/site/docs");
    let home = PathBuf::from("/site/docs/index.typ");
    let usage = PathBuf::from("/site/docs/guide/usage.typ");
    let plan = MenusPlan {
        items: BTreeMap::from([
            (
                "main".to_string(),
                vec![
                    MenuItemPlan {
                        path: Some(home.clone()),
                        url: None,
                        configured_label: Some("Home".to_string()),
                        configured_aria_label: None,
                        weight: None,
                    },
                    MenuItemPlan {
                        path: Some(usage.clone()),
                        url: None,
                        configured_label: None,
                        configured_aria_label: None,
                        weight: None,
                    },
                ],
            ),
            (
                "social".to_string(),
                vec![MenuItemPlan {
                    path: None,
                    url: Some("https://example.com".to_string()),
                    configured_label: Some("External".to_string()),
                    configured_aria_label: None,
                    weight: None,
                }],
            ),
        ]),
    };
    let meta = PageMetaMap::from([(
        usage.clone(),
        PageMeta {
            title: Some("Usage Guide".to_string()),
            ..PageMeta::default()
        },
    )]);
    let page_info = test_page_info(src, &[home, usage], &BTreeSet::new());

    let icon_temp = tempfile::tempdir().unwrap();
    let mut icon_cache = IconCache::new(icon_temp.path(), Path::new(ICON_CACHE_DIR));
    let menus = menus_from_plan(&plan, &meta, &page_info, &mut icon_cache).unwrap();

    assert_eq!(menus.items["main"][0].href, "index.html");
    assert_eq!(menus.items["main"][0].label, "Home");
    assert_eq!(menus.items["main"][1].href, "guide/usage.html");
    assert_eq!(menus.items["main"][1].label, "Usage Guide");
    assert_eq!(menus.items["social"][0].href, "https://example.com");
    assert_eq!(menus.items["social"][0].label, "External");
}

#[test]
fn menus_from_plan_supports_footer_text_items_without_links() {
    let src = Path::new("/site/docs");
    let home = PathBuf::from("/site/docs/index.typ");
    let plan = MenusPlan {
        items: BTreeMap::from([(
            "footer".to_string(),
            vec![
                MenuItemPlan {
                    path: Some(home.clone()),
                    url: None,
                    configured_label: Some("Docs".to_string()),
                    configured_aria_label: Some("Documentation".to_string()),
                    weight: None,
                },
                MenuItemPlan {
                    path: None,
                    url: None,
                    configured_label: Some("© 2026 Example".to_string()),
                    configured_aria_label: None,
                    weight: None,
                },
            ],
        )]),
    };
    let page_info = test_page_info(src, std::slice::from_ref(&home), &BTreeSet::new());
    let mut icon_cache = IconCache::new(src, Path::new(ICON_CACHE_DIR));

    let menus = menus_from_plan(&plan, &PageMetaMap::new(), &page_info, &mut icon_cache).unwrap();

    assert_eq!(menus.items["footer"][0].href, "index.html");
    assert_eq!(menus.items["footer"][0].label, "Documentation");
    assert_eq!(menus.items["footer"][1].href, "");
    assert_eq!(menus.items["footer"][1].label, "© 2026 Example");
}

#[test]
fn menus_from_plan_skips_footer_items_without_content() {
    let plan = MenusPlan {
        items: BTreeMap::from([(
            "footer".to_string(),
            vec![
                MenuItemPlan {
                    path: None,
                    url: None,
                    configured_label: Some("   ".to_string()),
                    configured_aria_label: None,
                    weight: None,
                },
                MenuItemPlan {
                    path: None,
                    url: None,
                    configured_label: Some("© 2026 Example".to_string()),
                    configured_aria_label: None,
                    weight: None,
                },
            ],
        )]),
    };

    let icon_temp = tempfile::tempdir().unwrap();
    let mut icon_cache = IconCache::new(icon_temp.path(), Path::new(ICON_CACHE_DIR));

    let menus = menus_from_plan(
        &plan,
        &PageMetaMap::new(),
        &PageInfoMap::new(),
        &mut icon_cache,
    )
    .unwrap();

    let footer = menus.items.get("footer").unwrap();
    assert_eq!(footer.len(), 1);
    assert_eq!(footer[0].label, "© 2026 Example");
}

#[test]
fn menus_from_plan_errors_when_page_info_is_missing() {
    let src = Path::new("/site/docs");
    let missing = PathBuf::from("/site/docs/missing.typ");
    let plan = MenusPlan {
        items: BTreeMap::from([(
            "main".to_string(),
            vec![MenuItemPlan {
                path: Some(missing),
                url: None,
                configured_label: None,
                configured_aria_label: None,
                weight: None,
            }],
        )]),
    };
    let icon_temp = tempfile::tempdir().unwrap();
    let mut icon_cache = IconCache::new(icon_temp.path(), Path::new(ICON_CACHE_DIR));

    let err = menus_from_plan(
        &plan,
        &PageMetaMap::new(),
        &PageInfoMap::new(),
        &mut icon_cache,
    )
    .unwrap_err();

    assert!(err.to_string().contains("page output was not planned"));
    assert!(err.to_string().contains("menu `main`"));
    assert!(err.to_string().contains(src.display().to_string().as_str()));
}

#[test]
fn theme_context_exposes_relative_named_menus() {
    let site = SiteModel::new(
        Vec::new(),
        MenusModel {
            items: BTreeMap::from([
                (
                    "main".to_string(),
                    vec![
                        NavItemModel {
                            language: None,
                            href: "index.html".to_string(),
                            label: "Home".to_string(),
                            label_html: html_escape("Home"),
                        },
                        NavItemModel {
                            language: None,
                            href: "guide/usage.html".to_string(),
                            label: "Usage".to_string(),
                            label_html: html_escape("Usage"),
                        },
                    ],
                ),
                (
                    "social".to_string(),
                    vec![NavItemModel {
                        language: None,
                        href: "https://example.com".to_string(),
                        label: "External".to_string(),
                        label_html: html_escape("External"),
                    }],
                ),
                (
                    "footer".to_string(),
                    vec![NavItemModel {
                        language: None,
                        href: String::new(),
                        label: "© 2026 Example".to_string(),
                        label_html: html_escape("© 2026 Example"),
                    }],
                ),
            ]),
        },
        SiteMetadata::default(),
        true,
    );

    let context = site.theme_context("guide/usage.html", None, &PageInfoMap::new(), None, None);

    assert_eq!(context.menus["main"][0].href, "../index.html");
    assert_eq!(context.menus["main"][1].href, "usage.html");
    assert!(context.menus["main"][1].active);
    assert_eq!(context.menus["social"][0].href, "https://example.com");
    assert_eq!(context.menus["footer"][0].href, "");
}

#[test]
fn theme_context_filters_menu_page_links_by_language() {
    let en = PathBuf::from("/site/docs/index.typ");
    let fr = PathBuf::from("/site/docs/fr/index.typ");
    let page_info = PageInfoMap::from([
        (
            en.clone(),
            PageInfo {
                language: Some("en".to_string()),
                translation_key: "index".to_string(),
                href: "index.html".to_string(),
                pdf_href: None,
            },
        ),
        (
            fr.clone(),
            PageInfo {
                language: Some("fr".to_string()),
                translation_key: "index".to_string(),
                href: "fr/index.html".to_string(),
                pdf_href: None,
            },
        ),
    ]);
    let site = SiteModel::new(
        Vec::new(),
        MenusModel {
            items: BTreeMap::from([
                (
                    "main".to_string(),
                    vec![
                        NavItemModel {
                            language: Some("en".to_string()),
                            href: "index.html".to_string(),
                            label: "Home".to_string(),
                            label_html: html_escape("Home"),
                        },
                        NavItemModel {
                            language: Some("fr".to_string()),
                            href: "fr/index.html".to_string(),
                            label: "Accueil".to_string(),
                            label_html: html_escape("Accueil"),
                        },
                    ],
                ),
                (
                    "social".to_string(),
                    vec![NavItemModel {
                        language: None,
                        href: "https://example.com".to_string(),
                        label: "External".to_string(),
                        label_html: html_escape("External"),
                    }],
                ),
            ]),
        },
        SiteMetadata::default(),
        true,
    );

    let context = site.theme_context("fr/index.html", page_info.get(&fr), &page_info, None, None);

    assert_eq!(context.menus["main"].len(), 1);
    assert_eq!(context.menus["main"][0].label, "Accueil");
    assert_eq!(context.menus["main"][0].href, "index.html");
    assert_eq!(context.menus["social"].len(), 1);
    assert_eq!(context.menus["social"][0].href, "https://example.com");
}

#[test]
fn page_meta_from_value_reads_calepin_keys_and_keeps_raw_dict() {
    let value = serde_json::json!({
        "title": " My Page ",
        "pdf": false,
        "layout": "layouts/landing.html",
        "date": "2026-06-10"
    });
    let meta = page_meta_from_value(&value);
    assert_eq!(meta.title.as_deref(), Some("My Page"));
    assert_eq!(meta.pdf, Some(false));
    assert_eq!(meta.layout.as_deref(), Some("layouts/landing.html"));
    assert_eq!(meta.raw, value);

    let blank_title = page_meta_from_value(&serde_json::json!({"title": ""}));
    assert_eq!(blank_title.title, None);

    let not_a_dict = page_meta_from_value(&serde_json::json!("not a dict"));
    assert_eq!(not_a_dict.raw, serde_json::json!({}));
}

#[test]
fn extract_document_title_reads_set_document_title() {
    assert_eq!(
        extract_document_title(
            r#"
#set document(title: [Site configuration])

#title()
"#
        )
        .as_deref(),
        Some("Site configuration")
    );
    assert_eq!(
        extract_document_title(
            r#"
#set document(
  title: [#emph[Calepin]: Computational notebooks in Typst],
)
"#
        )
        .as_deref(),
        Some("Calepin: Computational notebooks in Typst")
    );
    assert_eq!(
        extract_document_title(r#"#set document(title: "CLI reference")"#).as_deref(),
        Some("CLI reference")
    );
}

#[test]
fn extract_document_title_ignores_title_inside_other_arguments() {
    assert_eq!(
        extract_document_title(r#"#set document(subtitle: "Sub", title: "Real")"#).as_deref(),
        Some("Real")
    );
    assert_eq!(
        extract_document_title(r#"#set document(description: "see title: intro", title: "Real")"#)
            .as_deref(),
        Some("Real")
    );
    assert_eq!(
        extract_document_title(r#"#set document(description: [see title: intro], title: "Real")"#)
            .as_deref(),
        Some("Real")
    );
}

#[test]
fn extract_document_title_skips_document_set_without_title() {
    assert_eq!(
        extract_document_title(
            r#"
#set document(author: "Calepin")
#set document(title: [Real])
"#
        )
        .as_deref(),
        Some("Real")
    );
}

#[test]
fn extract_document_title_ignores_comments_and_raw_spans() {
    assert_eq!(
        extract_document_title(
            r#"
// #set document(title: [Comment])
`#set document(title: [Inline raw])`
```typ
#set document(title: [Block raw])
```
#set document(title: [Real])
"#
        )
        .as_deref(),
        Some("Real")
    );

    assert_eq!(
        extract_document_title(
            r#"
/* #set document(title: [Block comment]) */
#set document(title: [Real])
"#
        )
        .as_deref(),
        Some("Real")
    );
}

#[test]
fn load_page_meta_falls_back_to_document_title() {
    let temp = tempfile::tempdir().unwrap();
    let page = temp.path().join("page.typ");
    fs::write(&page, "#set document(title: [From document])").unwrap();

    let meta = load_page_meta(temp.path(), std::slice::from_ref(&page));

    assert_eq!(meta[&page].title.as_deref(), Some("From document"));
}

#[test]
fn discover_website_config_prefers_explicit_then_calepin() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("site");
    fs::create_dir_all(&input).unwrap();

    let missing = discover_website_config(temp.path(), &input, None);
    assert!(missing.is_err());
    assert!(missing.unwrap_err().to_string().contains("calepin.toml"));

    fs::write(input.join("calepin.toml"), "").unwrap();
    assert_eq!(
        discover_website_config(temp.path(), &input, None).unwrap(),
        input.join("calepin.toml")
    );

    let explicit = discover_website_config(
        temp.path(),
        &input,
        Some(Path::new("elsewhere/custom.toml")),
    )
    .unwrap();
    assert_eq!(explicit, temp.path().join("elsewhere/custom.toml"));
}

#[test]
fn build_pages_index_resolves_titles_and_excludes_fallback_page() {
    let src = Path::new("/site/docs");
    let post = PathBuf::from("/site/docs/blog/first.typ");
    let home = PathBuf::from("/site/docs/index.typ");
    let fallback = PathBuf::from("/site/docs/404.typ");
    let typ_files = vec![fallback, post.clone(), home.clone()];
    let sections = Vec::new();
    let raw = serde_json::json!({"title": "First Post", "date": "2026-06-10"});
    let meta = PageMetaMap::from([(post.clone(), page_meta_from_value(&raw))]);
    let pdf_files = BTreeSet::from([post.clone()]);

    let page_info = build_page_info(src, &typ_files, &meta, &pdf_files, &None).unwrap();
    let index = build_pages_index(src, &typ_files, &sections, &meta, &page_info);

    let entries = index.as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["path"], "blog/first.typ");
    assert_eq!(entries[0]["href"], "blog/first.html");
    assert_eq!(entries[0]["title"], "First Post");
    assert_eq!(entries[0]["pdf"], "blog/first.pdf");
    assert_eq!(entries[0]["meta"], raw);
    assert_eq!(entries[1]["title"], "index");
    assert_eq!(entries[1]["pdf"], serde_json::Value::Null);
    assert_eq!(entries[1]["meta"], serde_json::json!({}));
}

#[test]
fn pdf_enabled_files_honors_per_page_override_over_site_default() {
    let on = PathBuf::from("on.typ");
    let off = PathBuf::from("off.typ");
    let plain = PathBuf::from("plain.typ");
    let files = vec![on.clone(), off.clone(), plain.clone()];
    let meta = PageMetaMap::from([
        (
            on.clone(),
            PageMeta {
                pdf: Some(true),
                ..PageMeta::default()
            },
        ),
        (
            off.clone(),
            PageMeta {
                pdf: Some(false),
                ..PageMeta::default()
            },
        ),
    ]);

    let with_site_off = pdf_enabled_files(&files, &meta, None, Some(false));
    assert_eq!(with_site_off, BTreeSet::from([on.clone()]));

    let with_default = pdf_enabled_files(&files, &meta, None, None);
    assert_eq!(with_default, BTreeSet::from([on.clone()]));

    let with_site_on = pdf_enabled_files(&files, &meta, None, Some(true));
    assert_eq!(with_site_on, BTreeSet::from([on.clone(), plain]));

    let with_cli_off = pdf_enabled_files(&files, &meta, Some(false), Some(true));
    assert!(with_cli_off.is_empty());
}

#[test]
fn should_rebuild_for_path_ignores_distinct_output_directory() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("docs");
    let out = src.join("_site");
    fs::create_dir_all(&out).unwrap();
    let mut current = test_build_result(&src, &[]);
    current.out_dir = out.clone();

    assert!(!should_rebuild_for_path(&current, &out.join("index.typ")));
    assert!(!should_rebuild_for_path(&current, &out.join("style.css")));
    assert!(should_rebuild_for_path(&current, &src.join("index.typ")));
}

#[test]
fn should_rebuild_for_path_tracks_template_text_files() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("docs");
    let template = src.join("templates").join("robots.txt");
    fs::create_dir_all(template.parent().unwrap()).unwrap();
    fs::write(&template, "User-agent: *").unwrap();

    let mut current = test_build_result(&src, &[]);
    current.out_dir = temp.path().join("site");

    assert!(should_rebuild_for_path(&current, &template));
}

#[test]
fn should_rebuild_for_path_tracks_static_files_with_arbitrary_extensions() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("docs");
    let asset = src.join("assets").join("site.webmanifest");
    fs::create_dir_all(asset.parent().unwrap()).unwrap();
    fs::write(&asset, r#"{"name":"Calepin"}"#).unwrap();

    let mut current = test_build_result(&src, &[]);
    current.out_dir = temp.path().join("site");

    assert!(should_rebuild_for_path(&current, &asset));
}

#[test]
fn should_rebuild_for_path_ignores_generated_calepin_directory() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("docs");
    let wrappers = [
        src.join(".calepin/index/calepin-wrapper.typ"),
        src.join("websites/.calepin/website-config/calepin-wrapper.typ"),
    ];
    for wrapper in &wrappers {
        fs::create_dir_all(wrapper.parent().unwrap()).unwrap();
        fs::write(wrapper, "#import \"/.calepin/calepin.typ\"").unwrap();
    }
    let current = test_build_result(&src, &[]);

    for wrapper in wrappers {
        assert!(!should_rebuild_for_path(&current, &wrapper));
    }
}

#[test]
fn should_rebuild_for_path_ignores_custom_asset_dir() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("docs");
    let asset = src.join("_calepin").join("20_theme.css");
    fs::create_dir_all(asset.parent().unwrap()).unwrap();
    fs::write(&asset, "/* generated */").unwrap();
    let mut current = test_build_result(&src, &[]);
    current.asset_dir = PathBuf::from("_calepin");

    assert!(!should_rebuild_for_path(&current, &asset));
}

#[test]
fn clear_previous_outputs_preserves_git_directory_in_output_dir() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("docs");
    let out = temp.path().join("site");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(out.join(".git")).unwrap();
    fs::create_dir_all(out.join(".calepin")).unwrap();
    fs::write(out.join(MANIFEST_PATH), "{}").unwrap();
    fs::write(out.join(".git").join("HEAD"), "ref: refs/heads/main").unwrap();
    fs::write(out.join("stale.html"), "old").unwrap();
    fs::write(out.join(".gitkeep"), "").unwrap();

    clear_previous_outputs(&src, &out, false).unwrap();

    assert!(out.join(".git").join("HEAD").exists());
    assert!(out.join(".gitkeep").exists());
    assert!(!out.join("stale.html").exists());
}

#[test]
fn clear_previous_outputs_can_preserve_pagefind_directory() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("docs");
    let out = temp.path().join("site");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(out.join(".calepin")).unwrap();
    fs::write(out.join(MANIFEST_PATH), "{}").unwrap();
    fs::create_dir_all(out.join(PAGEFIND_DIR)).unwrap();
    fs::write(out.join(PAGEFIND_DIR).join("pagefind.js"), "bundle").unwrap();
    fs::write(out.join("stale.html"), "old").unwrap();

    clear_previous_outputs(&src, &out, true).unwrap();

    assert!(out.join(PAGEFIND_DIR).join("pagefind.js").exists());
    assert!(!out.join("stale.html").exists());
}

#[test]
fn clear_previous_outputs_refuses_unknown_non_empty_output_dir() {
    let temp = tempfile::tempdir().unwrap();
    let src = temp.path().join("docs");
    let out = temp.path().join("site");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("notes.txt"), "keep me").unwrap();

    let error = clear_previous_outputs(&src, &out, false).unwrap_err();

    assert!(error.to_string().contains("refusing to clean non-empty"));
    assert!(out.join("notes.txt").exists());
}

#[test]
fn remove_unexpected_rendered_outputs_uses_expected_outputs_for_assets() {
    let temp = tempfile::tempdir().unwrap();
    let expected_page = temp.path().join("index.html");
    let stale_page = temp.path().join("old.html");
    let asset_pdf = temp.path().join("assets").join("manual.pdf");
    fs::create_dir_all(asset_pdf.parent().unwrap()).unwrap();
    fs::write(&expected_page, "index").unwrap();
    fs::write(&stale_page, "old").unwrap();
    fs::write(&asset_pdf, "asset").unwrap();
    let expected = BTreeSet::from([expected_page.clone(), asset_pdf.clone()]);

    remove_unexpected_rendered_outputs(temp.path(), &expected).unwrap();

    assert!(expected_page.exists());
    assert!(!stale_page.exists());
    assert!(asset_pdf.exists());
}

#[test]
fn clear_previous_outputs_preserves_in_place_rendered_files() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("index.typ"), "= Home\n").unwrap();
    fs::write(temp.path().join("index.html"), "previous html").unwrap();
    fs::write(temp.path().join("index.pdf"), "previous pdf").unwrap();
    fs::write(temp.path().join("notes.html"), "user html").unwrap();

    clear_previous_outputs(temp.path(), temp.path(), false).unwrap();

    assert!(temp.path().join("index.html").exists());
    assert!(temp.path().join("index.pdf").exists());
    assert!(temp.path().join("notes.html").exists());
}
