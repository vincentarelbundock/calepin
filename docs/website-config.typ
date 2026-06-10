= Site configuration

Website settings live in `calepin.toml`, at the root of the website source directory. `website.toml` is accepted as a deprecated fallback name, and `--config <path>` points a build at a config stored elsewhere.

```toml
html_theme = "calepin-website"
title = "My Site"
description = "A static website built from Typst documents."
base_url = "https://example.com"
logo = "assets/logo.svg"
logo_alt = "My Site"
home = "index.html"
github_url = "https://github.com/user/repo"
pdf = true
pdf_theme = "docs/assets/pdf-theme.typ"
```

== HTML theme

`html_theme` selects the HTML theme. It can be:

- `calepin-website` for the bundled website theme
- a path to a theme directory containing `layout.html`

If `html_theme` is omitted, website builds use `calepin-website`. `theme` and `template` are accepted as backward-compatible aliases.

```toml
html_theme = "calepin-website"
```

== PDF outputs

`pdf` controls whether website builds also render a `.pdf` for every page. It defaults to `true`. Pages can override the site setting through the `pdf` entry of their `<website-metadata>` metadata, and `--format html` disables PDF rendering for one build regardless of configuration.

== PDF theme

`pdf_theme` selects a Typst theme file for PDF and other paged output. If it is omitted, _Calepin_ uses the bundled `calepin-pdf` theme, which styles ordinary fenced source blocks as boxes to match rendered chunk source and output blocks. You can also write `pdf_theme = "calepin-pdf"` explicitly. Set `pdf_theme = false` to disable this default. Relative paths resolve from the config file, so a theme stored with website assets can be referenced as `pdf_theme = "assets/pdf-theme.typ"` from the `calepin.toml` in the source directory.

== Metadata

`title`, `description`, and `base_url` are optional. The bundled website theme uses them for:

- browser page titles
- description metadata
- Open Graph metadata
- canonical URLs

When `base_url` is set, _Calepin_ also writes `sitemap.xml`.

== Branding

`logo` is a path or URL for the top-bar brand image. Relative paths are interpreted from the website output root and rewritten relative to each generated page, so `logo = "assets/logo.svg"` works from nested pages too.

`logo_alt` controls the image alt text. If no logo is configured, the bundled theme uses `title` as a text brand. `home` controls the brand link destination and defaults to `index.html`. `github_url` adds a GitHub link to the top bar.

== Build page selection

Sidebar and navbar settings control navigation, not the complete build set. When a sidebar is configured, use `[pages]` to add pages that should be rendered even though they are not listed in the navigation:

```toml
[pages]
include = ["landing/*.typ", "legal/privacy.typ"]
exclude = ["drafts/**"]
```

`include` and `exclude` entries are paths or glob patterns relative to the website source directory. In multilingual sites, the same patterns apply relative to each language's `content_dir`. `exclude` removes matching pages from the build set and from generated navigation.

Each content root's `index.typ` and `404.typ` are always rendered when present, even if they are not listed in the sidebar or navbar. This applies to the site root in a single-language site, and to each configured language `content_dir` in a multilingual site. Nested files such as `blog/index.typ` are not implicit; add them through navigation or `[pages].include` when they should be built.

== Multilingual sites

Configure languages in `calepin.toml` with one content directory per language:

```toml
default_language = "en"

[languages.en]
label = "English"
content_dir = "."

[languages.fr]
label = "Français"
content_dir = "fr"
```

With this layout, `about.typ` and `fr/about.typ` are treated as translations of the same page. The default language keeps root URLs like `about.html`; non-default languages use their language code as the URL prefix by default, such as `fr/about.html`.

Use page metadata to keep translations linked when files move or slugs differ:

```typ
#metadata((
  title: "À propos",
  translation_key: "about",
  slug: "a-propos",
)) <website-metadata>
```

The bundled website themes render a language picker when more than one language is configured. The picker shows each configured `label`; write labels in the language itself, such as `English`, `Français`, or `Deutsch`. The picker links to the matching translated page when one exists, and otherwise falls back to that language's home page. Pages with translations also emit alternate links. `url_prefix` can override a language's URL prefix.

Navigation is language-aware. Sidebar and navbar links to local pages are shown only for the current page's language, while external navbar links remain global. This lets each language directory keep parallel navigation without showing both `about.html` and `fr/about.html` in the same bar.

== Sitemap

When `base_url` is configured, _Calepin_ writes `sitemap.xml` in the output directory:

```toml
base_url = "https://example.com/project"
```

The sitemap lists every built page except `404.typ`, including pages with `hidden: true` metadata. URLs are absolute and use `base_url` plus the page's generated `.html` path.

If `base_url` is removed from the config, a stale generated sitemap is removed on the next build.

== Special pages

If `404.typ` exists, _Calepin_ renders it as `404.html` and `404.pdf`.

`404.typ` is excluded from automatic navigation and from `sitemap.xml`. This keeps the error page available to GitHub Pages without presenting it as a normal documentation page.
