#set document(title: [HTML templates])
#import "/.calepin/calepin.typ" as calepin
#title()

= HTML templates

For a single HTML notebook, use `layouts/notebook.html`. For websites, the default entry is `layouts/webpage.html`.

Layouts are MiniJinja templates. The most common values are:

- `doc.head`, `doc.body_open`, `doc.body`, `doc.body_close`
- `site.title`, `site.base_url`, `site.logo`, `site.favicon`
- `site.sidebar`, `site.sidebar_sections`, `site.toc`, `site.menus`
- `css`, `js`, `syntax_css`, `theme`, `target`

Navigation entries also expose `href`, `label`, `label_html`, and `active`.

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
