#set document(title: [Themes])
#import "/.calepin/calepin.typ" as calepin

#metadata((title: "Themes")) <website-metadata>

#title()

Themes control how _Calepin_ renders websites and notebooks. A theme can provide MiniJinja HTML templates, partials, CSS, JavaScript, supporting assets, and a Typst-side `notebook.typ.jinja` template for paged notebook output. The default theme is `calepin`.

Theme customization uses one mechanism: create a local theme directory, point `theme` at it, and optionally declare a base theme with `extends` in `theme.toml`. Start with a tiny local theme for CSS changes, then add templates or scripts only when needed.

#calepin.elements.gallery(
  (
    ("/themes/screenshots/calepin_website_dark.png", "Calepin website theme in dark mode", [Calepin website theme in dark mode]),
    ("/themes/screenshots/calepin_website_light.png", "Calepin website theme in light mode", [Calepin website theme in light mode]),
    ("/themes/screenshots/academic_website_dark.png", "Academic website theme in dark mode", [Academic website theme in dark mode]),
    ("/themes/screenshots/academic_website_light.png", "Academic website theme in light mode", [Academic website theme in light mode]),
    ("/themes/screenshots/tufte_notebook_dark.png", "Tufte notebook theme in dark mode", [Tufte notebook theme in dark mode]),
    ("/themes/screenshots/tufte_notebook_light.png", "Tufte notebook theme in light mode", [Tufte notebook theme in light mode]),
  ),
  columns: 3,
  max-width: 32em,
)

= Choose a theme

Select a built-in or local theme with `theme` in `calepin.toml`:

```toml
theme = "calepin"            # the default documentation theme
theme = "academic"           # a built-in essay/blog theme
theme = "themes/my-theme"    # a local theme directory

theme = "typst"              # raw Typst output, no Calepin theme
```

If the `calepin.toml` file is not located in the same directory as the document or website being compiled, specify it with `--config`:

```sh
calepin compile notebook.typ --config=/path/to/calepin.toml
```

You can also set the theme in-document with:

```typ
#calepin.setup(theme: "academic")
```

When several theme settings are present during a compile, the document setting wins, then `calepin.toml`, then the default theme.

= Built-in themes

_Calepin_ ships with built-in themes compiled into the binary. They are always available by name and can be selected without adding theme files to your project.

== `calepin`

`calepin` is the default documentation theme. It is designed for project documentation, manuals, notebook collections, and sites where navigation and reference lookup matter.

It includes sidebar navigation, a top bar, previous and next page links, an on-page table of contents, dark mode, copy buttons on code blocks, and rendered, source, and PDF view switching.

#calepin.elements.gallery(
  (
    ("/themes/screenshots/calepin_website_dark.png", "Calepin theme dark website", [Calepin theme dark website]),
    ("/themes/screenshots/calepin_website_light.png", "Calepin theme light website", [Calepin theme light website]),
  ),
  columns: 2,
  max-width: 42em,
)

== `academic`

`academic` is a reading-first essay and blog theme. It is designed for articles, research notes, project blogs, and smaller websites that prioritize long-form reading over dense navigation.

It includes a centered narrow text column, margin-note support, top navigation, dark mode, copy buttons on code blocks, and the shared Calepin search and language controls.

#calepin.elements.gallery(
  (
    ("/themes/screenshots/academic_website_dark.png", "academic theme dark website", [academic theme dark website]),
    ("/themes/screenshots/academic_website_light.png", "academic theme light website", [academic theme light website]),
  ),
  columns: 2,
  max-width: 42em,
)

== `typst`

`typst` disables the website and notebook themed wrappers and uses raw Typst output. Use this when you want unstyled HTML or output unchanged from the Typst source.

= What is a theme?

A theme is a directory of optional files. The only special file is `theme.toml`,
which identifies what the theme inherits and which shared pieces it imports. All
other files are ordinary templates, styles, scripts, or Typst source wrappers
that Calepin discovers by name.

A tiny local theme can contain only a manifest and one stylesheet:

```text
themes/my-theme/
  theme.toml
  css/
    site.css
```

Point Calepin at that directory from `calepin.toml`:

```toml
# calepin.toml
theme = "themes/my-theme"
```

The manifest declares the base theme explicitly:

```toml
# themes/my-theme/theme.toml
extends = "academic"
```

`extends` can name only a built-in theme or `typst`:

```toml
extends = "academic"      # inherit from a built-in theme
extends = "typst"         # inherit from no Calepin theme
```

Every local theme must declare `extends`; use `extends = "typst"` for a bare-bones start.

A fuller theme can provide any of these files:

```text
themes/my-theme/
  theme.toml            # theme metadata, inheritance, and shared imports
  layouts/
    webpage.html        # website page wrapper
    notebook.html       # standalone notebook HTML wrapper
    landing.html        # optional page-specific website layout
  partials/
    ...                 # reusable MiniJinja fragments
  css/                  # or styles/
    ...                 # theme CSS
  js/                   # or scripts/
    ...                 # theme JavaScript
  notebook.typ.jinja    # Typst template around notebook source
```

A child theme can override only the files it needs. Supporting files are
inherited from parent to child, and a child file with the same filename replaces
the parent file in place. New CSS and JavaScript files are appended in sorted
order after inherited files.

If you want a full copy of a built-in theme as a starting point, eject it with
`calepin new theme`:

```sh
calepin new theme                            # eject the default `calepin` theme to calepin_theme/
calepin new theme --theme academic           # eject the `academic` theme to calepin_theme/
calepin new theme --theme calepin themes/my  # copy into a custom directory
```

Once copied, the theme is project-owned: edit its templates, styles, scripts,
and `theme.toml` freely and keep it in version control.

= CSS customization

Built-in HTML themes expose stable CSS custom properties in the `--calepin-*` namespace. Prefer overriding these tokens from local theme CSS rather than targeting private theme internals.

```css
/* themes/my-theme/css/custom.css */
:root {
  --calepin-color-background: #f6f7f9;
  --calepin-color-text: #111827;
  --calepin-color-accent: #2563eb;
  --calepin-color-border: #d1d5db;
}

body {
  font-family: Inter, system-ui, sans-serif;
  background: var(--calepin-color-background);
  color: var(--calepin-color-text);
}

pre {
  border: 1px solid var(--calepin-color-border);
}
```

In Calepin HTML output, `#title()` is rendered as the page `<h1>`, so the first Typst heading `=` becomes `<h2>`, `==` as `<h3>`, and so on. If you target heading selectors in CSS, use this mapping unless your theme remaps heading markup.

== Light and dark themes

To define distinct values for light and dark mode, use `html[data-theme="light"]` and `html[data-theme="dark"]` selectors in your local theme stylesheet:

```css
html[data-theme="light"] {
  --calepin-color-background: #f6f7f9;
  --calepin-color-text: #111827;
  --calepin-color-accent: #2563eb;
}

html[data-theme="dark"] {
  --calepin-color-background: #101826;
  --calepin-color-text: #f8fafc;
  --calepin-color-accent: #60a5fa;
}
```

Calepin sets `html[data-theme="light"]` or `html[data-theme="dark"]` when the theme toggle is forced by user preference, explicit control, or URL/state storage. If no explicit mode is set, the browser preference is used for initial mode.

== Style raw HTML with CSS

Use a local theme that extends `typst` when you want a simple, unstyled HTML base:

```toml
# calepin.toml
theme = "themes/raw"
```

```toml
# themes/raw/theme.toml
extends = "typst"
```

Then add the raw HTML layout and CSS to the local theme:

```text
themes/raw/
  theme.toml
  layouts/
    notebook.html
  css/
    raw.css
```

= HTML templates

For a single HTML notebook, use `layouts/notebook.html`. For websites, the default entry is `layouts/webpage.html`.

Layouts are MiniJinja templates. The most common values are:

- `doc.head`, `doc.body_open`, `doc.body`, `doc.body_close`
- `site.title`, `site.base_url`, `site.logo`, `site.favicon`
- `site.sidebar`, `site.sidebar_sections`, `site.toc`, `site.menus`
- `styles`, `scripts`, `syntax_css`, `theme`, `target`

Navigation entries also expose `href`, `label`, `label_html`, and `active`.

Here is a minimal `layouts/notebook.html`:

```html
{{ doc.head }}
  <meta charset="UTF-8">
  <title>{{ doc.title }}</title>
  {% for style in styles %}
  <style>
{{ style.css }}
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
  {% for script in scripts %}
  <script>
{{ script.content }}
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

= Notebook Typst templates

`notebook.typ.jinja` is the Typst-side wrapper used by notebook outputs.

```text
themes/my-theme/
  notebook.typ.jinja
```

Before Typst runs, Calepin renders this file with MiniJinja so the output is still valid Typst source.

Inside the template, place notebook content with `document.body`:

```typ
#set page(
  paper: "us-letter",
  margin: (x: 1in, y: 0.85in),
  numbering: "1",
)

#set text(font: "Libertinus Serif", size: 10.5pt)

{{ document.body }}
```

Useful `notebook.typ.jinja` values:

- `theme`: local theme directory name
- `target`: `notebook`
- `document.path`: `.typ` input path relative to workspace
- `document.dir`: input directory relative to workspace
- `document.stem`: input filename without `.typ`
- `document.body`: notebook body, injected as a `#include`
- `document.meta`: values from `#metadata(...) <website-metadata>`
- `params`: CLI parameter map

If `document.body` is not referenced, Calepin appends the notebook body after the rendered template.

`theme = "typst"` disables notebook-specific theming, and `extends = "typst"` creates a local theme with no inherited Calepin base. Use an empty `notebook.typ.jinja` for a minimal pass-through template. `paged.typ.jinja` is not supported; use `notebook.typ.jinja`.

= Case study: Tufte


In this case study, we build on top of the `academic` theme to replicate
features of the popular Tufte CSS article style: serif typography, warm paper
colors, restrained accents, sidenotes, margin figures, and code/output surfaces
that match the page.

Reference files and rendered output can be viewed here:

- #link("examples/tufte/calepin.toml")[calepin.toml]
- #link("examples/tufte/themes/tufte/css/tufte.css")[tufte.css]
- #link("examples/tufte/tufte.typ")[tufte.typ]
- #link("examples/tufte/tufte.html")[HTML]
- #link("examples/tufte/tufte.pdf")[PDF]

Start with a small `calepin.toml` next to the document:

```toml
theme = "themes/tufte"
```

The local theme extends `academic`:

```toml
# themes/tufte/theme.toml
extends = "academic"
```

That keeps all of the built-in `academic` theme structure: the single-document
HTML wrapper, the theme toggle, sidenotes, side figures, code styling, and
dark-mode support. The local `themes/tufte/css/tufte.css` file is intentionally
small. It overrides the public `--calepin-*` tokens instead of targeting private
theme internals:

```css
:root, html[data-theme="light"] {
  --calepin-font-body: Palatino, "Palatino Linotype", "Book Antiqua", Georgia, serif;
  --calepin-font-heading: Palatino, "Palatino Linotype", "Book Antiqua", Georgia, serif;
  --calepin-surface-code: #fffdf0;
  --calepin-surface-output: #fffdf0;
  --calepin-surface: #fffdf0;
  --calepin-surface-muted: #fff8eb;
  --calepin-color-background: #fffff8;
  --calepin-color-text: #12110f;
  --calepin-color-muted: #5a5650;
  --calepin-color-accent: #7a2e2a;
  --calepin-color-accent-hover: #5f2520;
  --calepin-color-link: #7a2e2a;
  --calepin-color-link-hover: #5f2520;
}

html[data-theme="dark"] {
  --calepin-color-background: #18130d;
  --calepin-color-text: #f6efe8;
  --calepin-color-muted: #c5baa7;
  --calepin-color-accent: #d58a7f;
  --calepin-color-accent-hover: #f0b7ab;
  --calepin-color-link: #d58a7f;
  --calepin-color-link-hover: #f0b7ab;
  --calepin-surface-code: #1f1a14;
  --calepin-surface-output: #1f1a14;
  --calepin-surface: #1f1a14;
  --calepin-surface-muted: #2b241b;
}
```

The document source can then use the normal academic-theme elements:
`calepin.elements.sidenote` for margin notes,
`calepin.elements.sidefigure` for margin figures, and regular executable code
chunks for computed output. The stylesheet changes the feel of those elements,
but the layout behavior still comes from the built-in theme.

From the case-study directory, render the HTML and PDF with:

```sh
cd docs/themes/examples/tufte
calepin compile tufte.typ --config calepin.toml --format html
calepin compile tufte.typ --config calepin.toml --format pdf
```

The config path matters because `theme = "themes/tufte"` is resolved relative to
`calepin.toml`. Keeping the config, local theme, and document together makes the
example portable: copy the directory, run the same commands, and the academic
theme plus Tufte overlay are applied in both rendered outputs.

= List of tokens


#table(
  columns: (1.5fr, 1.5fr, 3.7fr),
  stroke: none,
  inset: 0.55em,
  [*Group*], [*Token*], [*Purpose*],
  "Colors", `--calepin-color-background`, "Page background in normal UI surfaces.",
  "", `--calepin-color-text`, "Primary body text color.",
  "", `--calepin-color-muted`, "Muted/de-emphasized text.",
  "", `--calepin-color-border`, "UI border color.",
  "", `--calepin-color-accent`, "Primary accent color (links, buttons, highlights).",
  "", `--calepin-color-accent-hover`, "Accent color for hover states.",
  "", `--calepin-color-accent-soft`, "Soft tint derived from accent.",
  "", `--calepin-color-accent-contrast`, "Text/icon contrast color on accent backgrounds.",
  "", `--calepin-color-link`, "Default link color.",
  "", `--calepin-color-link-hover`, "Link hover color.",
  "", `--calepin-color-focus`, "Focus ring/text emphasis color.",
  "", `--calepin-color-selection`, "Text-selection background color.",
  "", `--calepin-color-info`, "Info state color.",
  "", `--calepin-color-success`, "Success state color.",
  "", `--calepin-color-warning`, "Warning state color.",
  "", `--calepin-color-danger`, "Danger/error state color.",
  "", `--calepin-color-important`, "Important state color.",
  "", `--calepin-color-info-soft`, "Translucent info variant.",
  "", `--calepin-color-success-soft`, "Translucent success variant.",
  "", `--calepin-color-warning-soft`, "Translucent warning variant.",
  "", `--calepin-color-danger-soft`, "Translucent danger variant.",
  "", `--calepin-color-important-soft`, "Translucent important variant.",
  "Callouts", `--calepin-callout-note-color`, "Callout tone for note blocks.",
  "", `--calepin-callout-tip-color`, "Callout tone for tip blocks.",
  "", `--calepin-callout-warning-color`, "Callout tone for warning blocks.",
  "", `--calepin-callout-caution-color`, "Callout tone for caution blocks.",
  "", `--calepin-callout-important-color`, "Callout tone for important blocks.",
  "Surfaces", `--calepin-surface`, "Primary surface color for cards/panels.",
  "", `--calepin-surface-muted`, "Muted surface tone.",
  "", `--calepin-surface-raised`, "Raised surface tone.",
  "", `--calepin-surface-inset`, "Inset surface tone.",
  "", `--calepin-surface-code`, "Code block background.",
  "", `--calepin-surface-output`, "Code/output container background.",
  "Typography", `--calepin-font-body`, "Primary text font stack.",
  "", `--calepin-font-heading`, "Heading font stack.",
  "", `--calepin-font-mono`, "Monospace font stack.",
  "", `--calepin-font-size`, "Base font size multiplier.",
  "", `--calepin-font-size-sm`, "Small text font size.",
  "", `--calepin-font-size-aside`, "Aside/sidenote font size.",
  "", `--calepin-line-height`, "Default line height for body text.",
  "", `--calepin-line-height-tight`, "Tighter line height for headings.",
  "", `--calepin-heading-weight`, "Default heading weight.",
  "", `--calepin-code-font-size`, "Code and preformatted font size.",
  "Spacing", `--calepin-space-xs`, "Extra-small spacing token.",
  "", `--calepin-space-sm`, "Small spacing token.",
  "", `--calepin-space-md`, "Medium spacing token.",
  "", `--calepin-space-lg`, "Large spacing token.",
  "", `--calepin-space-xl`, "Extra-large spacing token.",
  "", `--calepin-space-2xl`, "2XL spacing token.",
  "", `--calepin-block-gap`, "Vertical block spacing.",
  "", `--calepin-inline-gap`, "Horizontal inline spacing.",
  "", `--calepin-section-gap`, "Vertical section spacing.",
  "Layout", `--calepin-content-width`, "Max width for main content regions.",
  "", `--calepin-wide-width`, "Max width for wide content regions.",
  "", `--calepin-page-width`, "Computed page width for two-column layout.",
  "", `--calepin-margin-width`, "Sidenote/margin note width.",
  "", `--calepin-margin-gap`, "Gap between body and margin region.",
  "", `--calepin-page-padding-inline`, "Inline page padding.",
  "", `--calepin-topbar-height`, "Topbar height token.",
  "", `--calepin-sidebar-width`, "Desktop sidebar width.",
  "", `--calepin-sidebar-mobile-width`, "Mobile drawer width.",
  "", `--calepin-shell-gap`, "Gap between shell grid tracks.",
  "", `--calepin-toc-indent`, "Indent used by generated TOC trees.",
  "Borders", `--calepin-border-width`, "Border width used across theme.",
  "", `--calepin-border`, "Shorthand border style using `--calepin-color-border`.",
  "", `--calepin-radius-sm`, "Small corner radius.",
  "", `--calepin-radius-md`, "Medium corner radius.",
  "", `--calepin-radius-lg`, "Large corner radius.",
  "Shadows", `--calepin-shadow-card`, "Default card shadow.",
  "", `--calepin-focus-ring`, "Focus ring style.",
  "Layers", `--calepin-z-topbar`, "Z-index for top bar.",
  "", `--calepin-z-backdrop`, "Backdrop and overlays.",
  "", `--calepin-z-drawer`, "Side drawer z-index.",
  "", `--calepin-z-popover`, "Popover/dialog z-index.",
  "", `--calepin-z-modal`, "Modal z-index.",
  "Syntax", `--calepin-syntax-foreground`, "Syntax text color for code blocks.",
  "", `--calepin-syntax-background`, "Syntax background color for code blocks.",
  "", `--calepin-syntax-border`, "Syntax border color derived from foreground/background.",
)
