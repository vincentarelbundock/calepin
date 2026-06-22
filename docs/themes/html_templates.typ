#set document(title: [HTML templates])
#import "/.calepin/calepin.typ" as calepin
#title()

= HTML templates

For a single HTML notebook, use `layouts/notebook.html`. For websites, the default entry is `layouts/webpage.html`.

Layouts are MiniJinja templates. The template context contains these top-level values:

- `doc`: Typst's generated HTML shell and document content.
- `site`: website metadata, navigation, page assets, and search settings.
- `css`: theme CSS files. Each item has `name` and `content`.
- `js`: theme JS files. Each item has `name` and `content`.
- `syntax_css`: standalone syntax-highlight CSS.
- `theme`: the active theme name.
- `target`: the render target; currently `html`.

`doc` contains:

- `doc.head`: Typst's document shell before `</head>`.
- `doc.body_open`: `</head>` and Typst's opening `<body>` tag.
- `doc.body`: Typst's generated body content.
- `doc.body_close`: Typst's closing `</body>` tag and any remaining document shell.
- `doc.title`: the document title, or an empty string.

`site` contains:

- `site.title`, `site.description`, `site.base_url`
- `site.logo`, `site.logo_alt`, `site.home_url`, `site.favicon`
- `site.current_url`, `site.page_title`
- `site.sidebar`: flat navigation entries.
- `site.sidebar_sections`: grouped navigation sections.
- `site.sidebar_fold`: whether titled sidebar sections should fold.
- `site.toc`: the current page table of contents.
- `site.menus`: named menus, such as `site.menus.main`, `site.menus.social`, and `site.menus.footer`.
- `site.menu_list`: all named menus as a list.
- `site.languages`: language entries for the language picker.
- `site.translations`: alternate-language links for the current page.
- `site.language`: the current language code.
- `site.scripts`: extra script URLs.
- `site.theme_assets`: emitted theme asset URLs, usually for built-in website themes.
- `site.pagefind`: Pagefind search assets and bundle path, when search is enabled.
- `site.revealjs`: Reveal.js runtime configuration as a JSON string.

Nested entries expose these fields:

- Navigation entries in `site.sidebar`, `site.menus.<name>`, and section `items`: `href`, `label`, `label_html`, `active`.
- Sidebar sections: `title`, `active`, `items`.
- TOC entries: `level`, `href`, `label`.
- Menu entries in `site.menu_list`: `name`, `items`.
- Language and translation entries: `code`, `label`, `href`, `active`.
- Theme assets: `name`, `href`, `kind`.
- Pagefind: `css`, `js`, `bundle`.

Here is a minimal `layouts/notebook.html`:

```html
{{ doc.head }}
  <meta charset="UTF-8">
  <title>{{ doc.title }}</title>
  {% for file in css %}
  <style>
{{ file.content }}
  </style>
  {% endfor %}
{{ doc.body_open }}
  <header class="document-header">
    <a href="index.html">Home</a>
    <button type="button" data-calepin-theme-toggle>Theme</button>
  </header>
  <main class="container">
    {{ doc.body }}
  </main>
  {% for file in js %}
  <script>
{{ file.content }}
  </script>
  {% endfor %}
{{ doc.body_close }}
```

Keep `doc.head`, `doc.body_open`, and `doc.body_close` unless you are intentionally replacing the entire HTML shell.

== Website layouts

You can switch layouts per page with `<website-metadata>`:

```typ
#metadata((
  title: "Landing page",
  layout: "layouts/landing.html",
)) <website-metadata>
```

The `layout` value must be a relative `.html` path inside the active theme. Calepin does not add `layouts/` or `.html` for you and does not fall back to `layouts/webpage.html` if the file is missing.

= Partials

Keep repeated HTML in partials under `partials/` and include them from layouts.

```html
{% include "partials/header.html" %}
```

Partials receive the same template context as the file that includes them.

= Shared imports

`theme.toml` can request shared partials, CSS, and JS so a theme uses common pieces from the built-in stack.

```toml
[shared]
partials = ["site-meta.html", "theme-init.html", "styles.html", "scripts.html", "pagefind-modal.html", "theme-toggle.html"]
css = ["theme.css", "code.css", "widgets.css"]
js = ["theme-toggle.js", "language-picker.js", "copy-code.js"]
```

Shared items load first, then local files in `partials/`, `css/`, and `js/` override by filename if they exist.

Use filenames only (`theme.css`, not `css/theme.css`, and not `../theme.css`).
