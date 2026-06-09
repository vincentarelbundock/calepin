HTML themes are supported for HTML output. Select a built-in theme with
`--template`:

```sh
calepin compile paper.typ --format html --template pico
calepin compile paper.typ --format html --template basic
```

`--template` is optional and only applies to HTML output. Use `pico` and
`basic` to force either built-in document theme on HTML output. Website
directory builds use `template` in `website.toml`; the bundled website theme is
`calepin-website`.

Local themes live under the configured `themes_dir`:

```text
themes/my-theme/
  layout.html
  partials/
  styles/main.css
  scripts/main.js
```

`layout.html` is a MiniJinja template. `partials/` files can be included with
`{% include "partials/name.html" %}`. CSS files in `styles/` and JavaScript
files in `scripts/` are loaded in filename order and exposed as `styles` and
`scripts` arrays.

Templates can access:

- `doc.head`
- `doc.body_open`
- `doc.body`
- `doc.body_close`
- `doc.title`
- `site.nav`
- `site.nav_sections`
- `site.toc`
- `site.title`
- `site.description`
- `site.base_url`
- `site.logo`
- `site.logo_alt`
- `site.home_url`
- `site.github_url`
- `site.current_url`
- `site.page_title`
- `styles`
- `scripts`
- `syntax_css`
- `theme`
- `target`

PDF themes are Typst files, not MiniJinja templates. The bundled PDF theme is
enabled by default for `calepin compile` and website PDF builds. Configure a
replacement in `config.toml` or `website.toml`:

```toml
pdf_theme = "themes/pdf.typ"
```

Set `pdf_theme = false` to disable the bundled PDF theme.
