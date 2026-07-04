#import "/.calepin/calepin.typ": *



#let _raw-chunk-langs = ("python", "r", "mermaid", "dot", "tikz", "d2", "sh", "text", "toml")
#show raw.where(block: true, lang: "typ", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "typst", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "python", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("python", it) }
#show raw.where(block: true, lang: "r", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("r", it) }
#show raw.where(block: true, lang: "mermaid", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("mermaid", it) }
#show raw.where(block: true, lang: "dot", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("dot", it) }
#show raw.where(block: true, lang: "tikz", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("tikz", it) }
#show raw.where(block: true, lang: "d2", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("d2", it) }
#show raw.where(block: true, lang: "sh", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("sh", it) }
#show raw.where(block: true, lang: "text", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("text", it) }
#show raw.where(block: true, lang: "toml", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("toml", it) }

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

// Notebook theme
#import "/.calepin/calepin.typ": _html-themed-raw-block, chunk_from_raw_plain

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
  } else if it.lang != none and _raw-chunk-langs.contains(it.lang) and _fenced-chunks-runs(
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
#set document(title: [Themes])
#import "/.calepin/calepin.typ" as calepin

#metadata((title: "Themes")) <website-metadata>

#title()

Themes control how _Calepin_ renders websites and notebooks. A theme can provide MiniJinja HTML templates, partials, CSS, JavaScript, and a Typst-side `layouts/pdf.typ` layout for paged (PDF or SVG) notebook output. The default theme is called `calepin`.

Theme customization uses one mechanism: create a local theme directory and point `calepin` to it when compiling.

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

Select a built-in or local theme with `--set theme=...`:

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin compile notebook.typ --set theme=calepin\ncalepin compile notebook.typ --set theme=academic\ncalepin compile notebook.typ --set theme=typst\ncalepin compile notebook.typ --set theme=themes/my-theme\n", block: true, lang: "sh"))

The built-in `calepin` theme is the default documentation theme, `academic` is for essay/blog pages, and `typst` emits raw Typst output with no Calepin styling.

You can also set the theme in-document:

```typ
#calepin.setup(theme: "academic")
```

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

`typst` disables the website and notebook themed wrappers and uses raw Typst output. Use this when you want unstyled HTML or PDF output.

= Build or customize

A theme is a directory with templates, resources, and a manifest.  A minimal theme could look like this, with only a `theme.toml` manifest and one CSS stylesheet:

```text
my-theme/
  theme.toml
  css/
    site.css
```

A custom theme _must_ extend one, and only one, built-in theme. This inheritance is specified explicitly in the `theme.toml` manifest:

```toml
extends = "academic"
# extends = "calepin" # full website with navigation
# extends = "typst"   # bare bones HTML and PDF; no styling
```

A fuller theme can provide any of these files:

```text
my-theme/
  theme.toml            # theme metadata and inheritance
  layouts/
    site.html           # website page wrapper
    document.html       # standalone notebook HTML wrapper
    site-landing.html   # optional page-specific website layout
    pdf.typ             # PDF/SVG Typst layout around notebook source
  partials/
    ...                 # reusable MiniJinja fragments
  css/
    ...                 # theme CSS
  js/
    ...                 # theme JavaScript
```

All the files in your custom theme are optional, except for the `theme.toml` manifest. When a file is present, it overrides the built-in theme file of the same name. When a file absent, it falls back to the built-in theme. New CSS and JavaScript files are appended in sorted order after inherited files.

A good way to start building a theme is to "eject" one of the built-in themes into your project. This copies all of the built-in theme's files into a local directory where you can modify or delete them. To start a new theme based on `academic`, run:

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin new theme /path/to/my_theme --theme=academic\n", block: true, lang: "sh"))

Modify the files in your `my_theme` directory, then render your document or website using the new theme:

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin compile notebook.typ --set theme=/path/to/my_theme/\n", block: true, lang: "sh"))
