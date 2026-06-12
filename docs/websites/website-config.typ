#set document(title: [Site configuration])

#title()

Website settings live in `calepin.toml` at the root of your website source directory. Use `--config <path>` only when the config file lives somewhere else.

= Basic settings

A small site can start with:

```toml
# Site name shown by bundled themes and page metadata.
# Default: none.
title = "My Site"

# Short site description for metadata and previews.
# Default: none.
description = "A static website built from Typst documents."

# Public site URL, used for sitemap.xml and canonical URLs.
# Default: none.
base_url = "https://example.com"

# Theme: "calepin", "academic", a local theme directory, or false for raw output.
# Default: "calepin".
theme = "academic"

# Also render .pdf files. Pages can override this with `pdf` metadata.
# Default: false.
pdf = false

# Logo image path for the top bar.
# Default: none.
logo = "assets/logo.svg"

# Alternative text for the logo image. Defaults to `title` when omitted.
# Default: none.
logo_alt = "My Site"

# Browser favicon path. Omit this to use Calepin's generated default.
# Default: ".calepin/favicon.svg".
favicon = "assets/favicon.ico"
```

Use `--format html` for a one-time HTML-only build, regardless of `pdf`.

Relative `logo` and `favicon` paths are resolved from the website source directory, then rendered as page-relative URLs so the same assets work from nested pages. If no favicon is set, _Calepin_ writes a small default to `.calepin/favicon.svg`. If no logo is set, the bundled theme uses `title` as the site name.

= Navigation

== Side bar

The sidebar is the main navigation for documentation-style sites. Configure it with `[sidebar]`; a section can list pages one by one:

```toml
[sidebar]

[[sidebar.section]]
title = "Guide"

  [[sidebar.section.item]]
  target = "install.typ"
  label = "Install"
```

or include several pages with a glob:

```toml
[[sidebar.section.item]]
glob = "guide/*.typ"
```

Use `target` for one link and `glob` for a list of source pages. A `target` ending in `.typ` and all `glob` entries point to Typst source files, not rendered `.html` files. _Calepin_ resolves those pages and writes the right `.html` links in the generated site. Other `target` values are used as literal links, such as external URLs.

If you do not configure a sidebar, _Calepin_ builds one from `.typ` files in the source directory. Hidden files are skipped.

Titled sections are foldable: each page loads with the section that contains it open and the others folded, and readers can open more sections by hand. To keep every section expanded instead, disable folding:

```toml
[sidebar]
fold = false
```

Labels can include icons:

```toml
label = "{icon:lucide:download} Install"
```

If the prefix is omitted, _Calepin_ uses `lucide`, so `{icon:home}` means `{icon:lucide:home}`. Icons are downloaded during the build and cached under `.calepin/icons/`.

Icon prefixes are Iconify collection names. Search available icons in the #link("https://icon-sets.iconify.design/")[Iconify icon sets] browser. Common prefixes include `lucide`, `simple-icons`, `tabler`, `heroicons`, `material-symbols`, `carbon`, `ph`, and `bi`.

== Top bar

Use `[navbar]` for a small top navigation bar. External links, such as a GitHub repository, are regular navbar items:

```toml
[navbar]

[[navbar.item]]
position = "left"
target = "index.typ"
label = "Home"

[[navbar.item]]
position = "right"
target = "https://github.com/user/repo"
label = "{icon:github} GitHub"

[[navbar.item]]
position = "right"
widget = "theme"

[[navbar.item]]
position = "right"
widget = "language"
```

Navbar items can use `target`, `glob`, or a theme widget such as `widget = "theme"` or `widget = "language"`. Use a `.typ` `target` or `glob` for internal source pages; use any other `target` for external links or a literal already-rendered URL. `position` can be `left`, `center`, or `right`. If you configure `[navbar]`, include any default widgets you still want to show.

= Include or exclude pages

Use `[pages]` for pages that should be built but should not appear in navigation, such as blog posts or legal pages:

```toml
[pages]
include = ["blog/*.typ", "legal/privacy.typ"]
exclude = ["drafts/**"]
```

Put the page title in the document and keep website metadata for fields used by listings, routing, and output options:

```typ
#set document(title: [First post])

#metadata((
  date: "2026-06-10",
  tags: ("release", "website"),
)) <website-metadata>

#title()
```

`index.typ` and `404.typ` are always built when present. If `404.typ` exists, _Calepin_ writes `404.html`; if PDF rendering is enabled for that page, it also writes `404.pdf`.

= Working with pages

Use `calepin.pages()` to get structured information about every built page and process it with normal Typst code. This is useful for lists, indexes, feeds, publication pages, course pages, and any page that needs to organize other pages in the site.

```typ
#import "@preview/calepin:0.0.1" as calepin

#let posts = calepin.pages()
  .filter(p => p.path.starts-with("blog/"))
  .sorted(key: p => p.meta.at("date", default: ""))
  .rev()

#for post in posts [
  - #link(post.href)[#post.title]
]
```

Each page entry includes:

- `path`: source path relative to the website root
- `href`: rendered HTML path relative to the current page
- `title`: resolved page title
- `language`: language code, when multilingual sites are configured
- `translation_key`: key used to connect translated pages
- `translations`: matching pages in other languages
- `pdf`: PDF path when the page has a PDF output
- `meta`: the page's `<website-metadata>` dictionary

Use `meta` for your own fields, such as dates, tags, authors, categories, venues, summaries, or feature flags. Since `calepin.pages()` returns a Typst array of dictionaries, you can use Typst methods such as `filter`, `map`, `sorted`, and `rev` to select and format the pages you need.

= Multilingual sites

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
#set document(title: [À propos])

#metadata((
  translation_key: "about",
  slug: "a-propos",
)) <website-metadata>

#title()
```

When more than one language is configured, the bundled themes show a language picker. Local navigation links are shown only for the current language; external links stay global.
