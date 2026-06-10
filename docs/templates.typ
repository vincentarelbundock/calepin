HTML themes are supported for HTML output. Select a theme with `--theme`:

```sh
calepin compile paper.typ --format html --theme calepin-html
```

`--theme` is optional and only applies to HTML output. Single-document HTML
builds use `calepin-html` by default. Website directory builds use
`html_theme` in `website.toml` and default to `calepin-website`. `theme` and
`template` are accepted as backward-compatible config aliases.

Local themes are selected by pointing `--theme` or `html_theme` at the directory containing `layout.html`:

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
- `snippets.css.code`
- `snippets.js.copy_code`
- `snippets.js.theme_toggle`
- `snippets.typst.code_block`
- `styles`
- `scripts`
- `syntax_css`
- `theme`
- `target`

Bundled snippets are small reusable pieces that can be used across local themes.
For example, an HTML theme can add shared code/output styling and copy buttons
without maintaining its own CSS or JavaScript:

```html
<style>{{ snippets.css.code }}</style>
<script>{{ snippets.js.copy_code }}</script>
```

PDF themes are Typst files, not MiniJinja templates. The bundled `calepin-pdf`
theme is enabled by default for `calepin compile` and website PDF builds.
Configure a replacement in `config.toml` or `website.toml`:

```toml
pdf_theme = "themes/pdf.typ"
```

You can also write `pdf_theme = "calepin-pdf"` explicitly; omitting
`pdf_theme` has the same effect.

`calepin-pdf` imports `/.calepin/snippets/typst/code-block.typ` and applies it
to raw source blocks. Custom PDF themes can import the same general snippet:

```typ
#import "/.calepin/snippets/typst/code-block.typ": code-block
```

Set `pdf_theme = false` to disable `calepin-pdf`.
