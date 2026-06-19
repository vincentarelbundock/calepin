#set document(title: [Customize themes])
#metadata((title: "Customize CSS")) <website-metadata>

#title()

For CSS-only changes, keep a built-in theme and append project CSS with
`styles` in `calepin.toml`:

```toml
theme = "academic"
styles = [
  "styles/site.css",
  "styles/figures.css",
]
```

Configured CSS files load after the selected theme's CSS, in the order listed.
Paths are resolved relative to `calepin.toml`, and each file must have a `.css`
extension. The setting affects HTML output only.

= Theme tokens

Built-in HTML themes expose stable CSS custom properties in the `--calepin-*`
namespace. Override these tokens from project CSS instead of targeting
theme-specific internals such as `--academic-*`, `--pico-*`, or one-off
component variables.

Every color and surface token has explicit light, dark, and system-dark values:

```css
:root,
html[data-theme="light"] {
  --calepin-color-background: #fffff8;
  --calepin-color-text: #12110f;
  --calepin-color-accent: #7a2e2a;
  --calepin-surface: #fffff8;
}

html[data-theme="dark"] {
  --calepin-color-background: #15130f;
  --calepin-color-text: #eee6d6;
  --calepin-color-accent: #d18b82;
  --calepin-surface: #1d1a14;
}
```

The stable token groups cover colors, state colors, callouts, surfaces,
typography, spacing, reading width, margin notes, borders, shadows, focus rings,
and interface layers. For example:

```css
:root {
  --calepin-font-body: Palatino, "Palatino Linotype", Georgia, serif;
  --calepin-font-heading: var(--calepin-font-body);
  --calepin-heading-weight: 400;
  --calepin-content-width: 39rem;
  --calepin-margin-width: 16rem;
  --calepin-margin-gap: 2.25rem;
  --calepin-block-gap: 1.5rem;
  --calepin-callout-warning-color: #b26a00;
}
```

_Calepin_ maps its tokens into Pico CSS variables internally, but `--calepin-*`
is the public customization surface for built-in themes.

= Raw HTML plus CSS

Set `theme = "typst"` when you do not want a bundled base theme. Configured
styles still apply to HTML output:

```toml
theme = "typst"
styles = ["styles/raw.css"]
```

This mode is useful when you want Typst's raw HTML structure and a completely
project-owned stylesheet.
