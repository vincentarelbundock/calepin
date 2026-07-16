#let _calepin-document-element = document
#import "/.calepin/calepin.typ": *
#let document = _calepin-document-element



#let _raw-chunk-langs = ("python", "r", "mermaid", "dot", "tikz", "d2", "toml")
#show raw.where(block: true, lang: "typ", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "typst", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "python", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("python", it) }
#show raw.where(block: true, lang: "r", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("r", it) }
#show raw.where(block: true, lang: "mermaid", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("mermaid", it) }
#show raw.where(block: true, lang: "dot", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("dot", it) }
#show raw.where(block: true, lang: "tikz", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("tikz", it) }
#show raw.where(block: true, lang: "d2", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("d2", it) }
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

#show heading: it => {
  if _is-html() and "label" in it.fields() {
    std.html.elem("calepin-heading-anchor", attrs: (data-id: str(it.label)))
  }
  it
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

#import "/.calepin/calepin.typ" as calepin
#set document(title: [Website navigation])
#metadata((title: "Navigation", tags: ("websites", "navigation"))) <website-metadata>

#title()

= Side bar

The sidebar is the main navigation for documentation-style sites. Configure it with `[sidebar]`; a section can list pages one by one:

```toml
[sidebar]

[[sidebar.section]]
title = "Guide"

  [[sidebar.section.item]]
  target = "install.typ"
```

or include several pages with a glob:

```toml
[[sidebar.section.item]]
glob = "guide/*.typ"
```

Use `target` for one source page and `glob` for a list of source pages. Page targets point to Typst source files, not rendered `.html` files. _Calepin_ resolves those pages and writes the right `.html` links in the generated site.

Sidebar items must be nested under `[[sidebar.section]]`, but the section does not need a title. Use an untitled section when you want items to appear as a plain unheaded list:

```toml
[sidebar]

[[sidebar.section]]
  [[sidebar.section.item]]
  target = "index.typ"

  [[sidebar.section.item]]
  target = "getting-started.typ"
```

The sidebar label comes from the page source, not from `calepin.toml`. Put the label in the page's website metadata:

```typ
#set document(title: [Install])
#metadata((title: "Install")) <website-metadata>

#title()
```

If a page has no `website-metadata.title`, _Calepin_ falls back to the document title, then the filename stem. This keeps multilingual sidebars in one place: each translated page carries its own translated title.

Use an external URL as `target` when you want a sidebar link to leave the site. External targets must set `label` because there is no page metadata to read:

```toml
[[sidebar.section.item]]
target = "https://example.com/reference"
label = "External reference"
```

Add non-link subheadings inside a section with an item that sets only `label`:

```toml
[[sidebar.section.item]]
label = "Language"

[[sidebar.section.item]]
target = "reference/syntax.typ"

[[sidebar.section.item]]
target = "reference/styling.typ"

[[sidebar.section.item]]
label = "Library"

[[sidebar.section.item]]
target = "reference/model.typ"
```

Subheadings are rendered in sidebar order with the `calepin-website-sidebar-subheading` class so themes can style them separately. They do not link anywhere, add build pages, or affect which folded section opens. A sidebar item with a `.typ` `target` or `glob` cannot also set `label`; page labels still come from page metadata.

If you do not configure a sidebar, _Calepin_ builds one from `.typ` files in the source directory. Hidden files are skipped.

Titled sections are foldable: each page loads with the section that contains it open and the others folded. Opening a different section folds the previous one. To keep every section expanded instead, disable folding:

```toml
[sidebar]
fold = false
```

= Table of contents

Pages can show an "On this page" table of contents built from their own headings (levels 1-3 by default). The `calepin` theme shows one by default; other themes, including `academic`, are opt-in.

Set a site-wide default with `[toc]`:

```toml
[toc]
enabled = true
depth = 2
```

`depth` is the maximum heading level included, from 1 to 6.

Override either field for a single page with `<website-metadata>`:

```typ
#metadata((toc: (enabled: false))) <website-metadata>
```

```typ
#metadata((toc: (depth: 2))) <website-metadata>
```

Page metadata and `calepin.toml` merge field by field: a page can override just `depth` and still inherit `enabled` from `calepin.toml`, or the reverse. Whatever is left unset falls back to the theme's own default.

= Site menus

Use `[menus]` for named navigation groups. Menu names describe what the links
mean; themes decide where to render them. The bundled themes understand
`main` and `social`. Custom themes can use any additional menu name.

```toml
[[menus.main]]
target = "index.typ"
label = "Home"
weight = 10

[[menus.social]]
target = "https://github.com/user/repo"
label = "{icon:github}"
aria-label = "GitHub"
```

Menu items use `target` or `glob`. Use a `.typ` `target` or `glob` for internal
source pages; use any other `target` for external links or a literal
already-rendered URL. Omit `label` for internal pages to use the page metadata
title, document title, or filename stem.

= Footer

Configure the site footer with `[[footer.item]]`. Footer items can be links or
plain text rows for copyright and legal notices:

```toml
[[footer.item]]
label = "© 2026 Example"

[[footer.item]]
target = "https://example.com/privacy"
label = "Privacy"
```

A footer row with only `label` is rendered as text (no hyperlink).

Use `weight` to control ordering within one menu or footer. Lower weights appear first.
Items without weights keep their config order after weighted items.

Labels can include Iconify icons with `{icon:...}`. If the prefix is omitted,
_Calepin_ uses `lucide`, so `{icon:github}` means `{icon:lucide:github}`.
Icon prefixes are Iconify collection names. Search available icons in the
#link("https://icon-sets.iconify.design/")[Iconify icon sets] browser.

For icon-only visible labels, set `aria-label` so screen readers get a human-readable name:

```toml
aria-label = "GitHub"
```

(If you omit `aria-label`, Calepin uses fallback text if available; for an icon-only label with no fallback text this will be less readable.)

Local SVG icons are also supported with source-relative paths:

```toml
[[menus.social]]
target = "https://example.com/project"
label = "{icon:assets/icons/project.svg} Project"
```

Local icon paths must stay inside the website source directory. _Calepin_
sanitizes local and downloaded SVGs before inlining them.
