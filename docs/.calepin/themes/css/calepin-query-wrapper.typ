#let _calepin-document-element = document
#import "/.calepin/calepin.typ": *
#let document = _calepin-document-element



#let _raw-chunk-langs = ("python", "r", "mermaid", "dot", "tikz", "d2")
#show raw.where(block: true, lang: "typ", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "typst", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "python", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("python", it) }
#show raw.where(block: true, lang: "r", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("r", it) }
#show raw.where(block: true, lang: "mermaid", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("mermaid", it) }
#show raw.where(block: true, lang: "dot", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("dot", it) }
#show raw.where(block: true, lang: "tikz", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("tikz", it) }
#show raw.where(block: true, lang: "d2", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("d2", it) }

#show raw.where(block: true, theme: auto): it => {
  if _is-query() {
    it
  } else if _disable-raw-chunk-transforms.get() {
    _html-themed-raw-block(it)
  } else if it.has("lang") and it.lang != none and _raw-chunk-langs.contains(it.lang) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}

#show heading: it => {
  if _is-html() and "label" in it.fields() {
    std.html.elem("calepin-heading-anchor", attrs: (data-id: str(it.label)))
  }
  it
}

// Notebook theme
#import "/.calepin/calepin.typ": _html-themed-raw-block, _is-query, chunk_from_raw_plain

// Body text size, captured below at document-body level. Code blocks are sized
// relative to this rather than to `1em`, which would compound: a literal
// ```typ block is rendered by replacing its source `raw` element, so it renders
// inside Typst's already-reduced raw text context, whereas executed chunks are
// emitted as ordinary calls at body size. Anchoring to the captured body size
// gives both paths a single, matching reduction instead of shrinking twice.
#let _calepin-body-size = std.state("calepin-body-size", 11pt)

#show raw.where(block: true): it => {
  if it.theme != auto {
    context {
      set text(size: _calepin-body-size.get() * 0.8)
      it
    }
  } else if it.lang != none and (_is-query() or _raw-chunk-langs.contains(it.lang)) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}

#context _calepin-body-size.update(text.size)

#import "/.calepin/calepin.typ" as calepin_runtime
#set document(title: [CSS])
#import "/.calepin/calepin.typ" as calepin
#show: calepin.document
#metadata((tags: ("themes", "CSS", "HTML"))) <website-metadata>
#title()

= CSS tokens

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

= Light and dark themes

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

= Style raw HTML

Use a local theme that extends `typst` when you want a simple, unstyled HTML base.

Create a directory with a manifest, a layout, and a stylesheet:

```text
raw/
  theme.toml
  layouts/
    document.html
  css/
    raw.css
```

Edit the manifest `raw/theme.toml`:

```toml
extends = "typst"
```

Edit `raw/layouts/document.html` to wrap Typst's generated HTML and inline the theme CSS files:

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
  <main class="page">
    {{ doc.body }}
  </main>
{{ doc.body_close }}
```

The important pieces are `doc.head`, `doc.body_open`, `doc.body`, and `doc.body_close`, which preserve the HTML shell Typst produced. The `css` loop loads files from the theme's `css/` directory, including `raw.css`.

Compile the notebook:

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin compile notebook.typ --format html --set theme=raw\n", block: true, lang: "sh"))

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
