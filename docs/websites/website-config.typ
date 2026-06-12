= Site configuration

Website settings live in `calepin.toml` at the root of your website source directory. A small site can start with:

```toml
title = "My Site"
description = "A static website built from Typst documents."
base_url = "https://example.com"
theme = "academic"
pdf = false

[sidebar]

[[sidebar.section]]
title = "Guide"

  [[sidebar.section.item]]
  path = "index.typ"
  label = "Home"

  [[sidebar.section.item]]
  path = "about.typ"
  label = "About"
```

Use `--config <path>` only when the config file lives somewhere else.

== Basic settings

#table(
  columns: (1fr, 0.8fr, 2.6fr),
  stroke: none,
  inset: 0.55em,
  [*Key*], [*Default*], [*Use it for*],
  [`title`], [`none`], [The site name shown by the bundled themes and in page metadata.],
  [`description`], [`none`], [A short site description for metadata and previews.],
  [`base_url`], [`none`], [The public site URL. Set this to generate `sitemap.xml` and canonical URLs.],
  [`theme`], [`"calepin"`], [`"calepin"`, `"academic"`, a local theme directory, or `false` for raw output.],
  [`pdf`], [`false`], [When `true`, also render `.pdf` files. Individual pages can override this with `pdf` in their `<website-metadata>` metadata.],
)

Use `--format html` for a one-time HTML-only build, regardless of `pdf`.

== Branding

Set a logo and links for the top bar:

```toml
logo = "assets/logo.svg"
logo_alt = "My Site"
home = "index.html"
github_url = "https://github.com/user/repo"
```

Relative logo paths are resolved from the output directory, so the same logo works from nested pages. If no logo is set, the bundled theme uses `title` as the site name.

== Navigation

The sidebar is the main navigation for documentation-style sites. A section can list pages one by one:

```toml
[[sidebar.section]]
title = "Guide"

  [[sidebar.section.item]]
  path = "install.typ"
  label = "Install"
```

or include several pages with a glob:

```toml
[[sidebar.section.item]]
glob = "guide/*.typ"
```

If you do not configure a sidebar, _Calepin_ builds one from `.typ` files in the source directory. Hidden files are skipped.

Labels can include icons:

```toml
label = "{icon:lucide:download} Install"
```

If the prefix is omitted, _Calepin_ uses `lucide`, so `{icon:home}` means `{icon:lucide:home}`. Icons are downloaded during the build and cached under `.calepin/icons/`.

== Top navigation

Use `[navbar]` for a small top navigation bar:

```toml
[navbar]

[[navbar.item]]
position = "left"
path = "index.typ"
label = "Home"

[[navbar.item]]
position = "right"
url = "https://github.com/user/repo"
label = "{icon:simple-icons:github} GitHub"
```

Navbar items can use `path`, `glob`, `url`, or a theme widget such as `widget = "theme"` or `widget = "language"`. `position` can be `left`, `center`, or `right`.

== Page titles and hidden pages

Set a page title in the Typst file when you want something clearer than the file name:

```typ
#metadata((title: "Getting started")) <website-metadata>
```

Navigation labels are chosen in this order:

1. the `label` in `calepin.toml`
2. the page's `title` metadata
3. the file name

Use `[pages]` for pages that should be built but should not appear in navigation, such as blog posts or legal pages:

```toml
[pages]
include = ["blog/*.typ", "legal/privacy.typ"]
exclude = ["drafts/**"]
```

Then hide individual pages from generated navigation with page metadata:

```typ
#metadata((title: "First post", hidden: true)) <website-metadata>
```

`index.typ` and `404.typ` are always built when present. If `404.typ` exists, _Calepin_ writes `404.html`; if PDF rendering is enabled for that page, it also writes `404.pdf`.

== Page listings

Pages can list other pages in the site. This is useful for a blog index, project index, or publication list.

```typ
#import "@preview/calepin:0.0.1" as calepin

#for post in calepin.pages()
  .filter(p => p.path.starts-with("blog/"))
  .sorted(key: p => p.meta.at("date", default: ""))
  .rev() [
  - #link(post.href)[#post.title]
]
```

Each page entry includes `path`, `href`, `title`, `language`, `translations`, `pdf`, and `meta`. The `meta` field contains the page's `<website-metadata>` values, so you can sort or filter on your own fields such as dates, tags, authors, or categories.

== Multilingual sites

Configure languages with one content directory per language:

```toml
default_language = "en"

[languages.en]
label = "English"
content_dir = "."

[languages.fr]
label = "Français"
content_dir = "fr"
```

With this layout, `about.typ` and `fr/about.typ` are treated as translations of the same page. The default language keeps root URLs like `about.html`; other languages use their language code as a URL prefix, such as `fr/about.html`.

Use page metadata when translations move or need different slugs:

```typ
#metadata((
  title: "À propos",
  translation_key: "about",
  slug: "a-propos",
)) <website-metadata>
```

When more than one language is configured, the bundled themes show a language picker. Local navigation links are shown only for the current language; external links stay global.
