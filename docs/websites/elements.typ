#import "@preview/calepin:0.0.1" as calepin

#set document(title: [Re-useable elements])

#metadata((pdf: false)) <website-metadata>

#calepin.setup(fenced-chunks: true)
#let target = sys.inputs.at("calepin-target", default: "paged")

#title()

In Typst, `#let` is your primary building block for reusable formatting and construction. Use it to avoid repeating the same patterns.

#let banner(message, note) = [
  #strong(message)
  #v(0.3em)
  #quote(note)
]

```typ
#let banner(message, note) = [
  #strong(message)
  #v(0.3em)
  #quote(note)
]

#banner("Reusable functions", "Define UI once and reuse it for many places.")
```

#banner("Reusable function in action", "If you update this definition, both call sites update automatically.")

= Output-aware formatting

Calepin exposes `#calepin.elements.target()` to pick content for HTML, static outputs, or a fallback. This example uses colors to make output differences explicit. In Typst `0.15`, HTML color output was not fully supported, so HTML branches use explicit CSS via `html.elem()`.

```typ
#let html-color-label(body, color) = html.elem("span", attrs: (style: "color: " + color + ";"))[
  #body
]

#calepin.elements.target(
  html: () => [#html-color-label([#strong("HTML")], "blue") output branch],
  paged: () => [#text(fill: green)[#strong("PDF/SVG")] output branch],
  fallback: () => [#text(fill: gray)[#strong("Fallback")] output branch],
)
```

The rendered result:


#let html-color-label(body, color) = html.elem("span", attrs: (style: "color: " + color + ";"))[
  #body
]

#calepin.elements.target(
  html: () => [#html-color-label([#strong("HTML")], "blue") output branch],
  paged: () => [#text(fill: green)[#strong("PDF/SVG")] output branch],
  fallback: () => [#text(fill: gray)[#strong("Fallback")] output branch],
)

= Built-in elements

The namespace includes a compact set of reusable elements for cards, galleries, columns, tabs, and lightbox media. They are designed to demonstrate output-aware patterning in practical notebook websites.

== `card`

`calepin.elements.card` keeps one piece of content boxed consistently across formats.

```typ
#let callout = calepin.elements.card[
  #heading(level: 3)[Reusable card]
  A card wraps content with matching style in HTML and paged output.
]
#callout
```

#calepin.elements.card[
  #heading(level: 3)[Reusable card]
  A card wraps content with matching style in HTML and paged output.
]

== Gallery

`calepin.elements.gallery` accepts image items as tuples or dictionaries and handles local image metadata automatically in static outputs. In HTML output, it can activate lightbox behavior.

```typ
#calepin.elements.gallery(
  (
    ("../assets/flowers_01.jpg", "First flower", [First flower]),
    ("../assets/flowers_04.jpg", "Fourth flower", [Fourth flower]),
    ("../assets/flowers_02.jpg", "Second flower", [Second flower]),
    ("../assets/flowers_03.jpg", "Third flower", [Third flower]),
    ("../assets/flowers_05.jpg", "Fifth flower", [Fifth flower]),
  ),
  columns: 3,
  max-width: 42em,
)
```

#calepin.elements.gallery(
  (
    ("../assets/flowers_01.jpg", "First flower", [First flower]),
    ("../assets/flowers_04.jpg", "Fourth flower", [Fourth flower]),
    ("../assets/flowers_02.jpg", "Second flower", [Second flower]),
    ("../assets/flowers_03.jpg", "Third flower", [Third flower]),
    ("../assets/flowers_05.jpg", "Fifth flower", [Fifth flower]),
  ),
  columns: 3,
  max-width: 42em,
)

== Columns

`calepin.elements.columns` is output-aware, so the same call produces a plain Pico `.grid` for HTML and a `grid(...)` for paged output. Columns are equal-width; pass the paged-output column count as an integer. By default, each item is wrapped in a `<div>` so plain Typst blocks stay together as one column; use `wrap: false` when the items already render as standalone HTML elements such as cards.

```typ
#calepin.elements.columns(
  columns: 2,
  wrap: false,
  [
    #calepin.elements.card[
      #heading(level: 3)[Left]
      Use an equal-width two-column layout for related content.
    ]
  ],
  [
    #calepin.elements.card[
      #heading(level: 3)[Right]
      Paged output renders this via Typst `grid(...)`; HTML uses `<div class="grid">` by default.
    ]
  ],
)
```

#calepin.elements.columns(
  columns: 2,
  wrap: false,
  [
    #calepin.elements.card[
      #heading(level: 3)[Left]
      Use an equal-width two-column layout for related content.
    ]
  ],
  [
    #calepin.elements.card[
      #heading(level: 3)[Right]
      Paged output renders this via Typst `grid(...)`; HTML uses `<div class="grid">` by default.
    ]
  ],
)

You can also request more than two columns:

```typ
#calepin.elements.columns(
  columns: 4,
  wrap: true,
  [One], [Two], [Three]
)
```

#calepin.elements.columns(
  columns: 4,
  wrap: true,
  [One], [Two], [Three]
)

== Tabs

`calepin.elements.tabs` renders Web Awesome tabs in HTML and lists each enabled panel in paged output. Use `calepin.elements.tabs[...]` as the group and `calepin.elements.tab("Label", active: true)[...]` for each panel. Panel names are generated automatically; pass `name: "..."` only when you need a stable custom panel id. Fenced code chunks inside tabs are still discovered and executed.

````typ
#calepin.elements.tabs[
  #calepin.elements.tab("R", active: true)[
This tab shows R code:

```r
x <- c(1, 2, 3, 4, 5)
mean(x)
```
  ]

  #calepin.elements.tab("Python")[
This tab shows Python code:

```python
x = [1, 2, 3, 4, 5]
sum(x) / len(x)
```
  ]
]
````

#calepin.elements.tabs[
  #calepin.elements.tab("R", active: true)[
This tab shows R code:

```r
x <- c(1, 2, 3, 4, 5)
mean(x)
```
  ]

  #calepin.elements.tab("Python")[
This tab shows Python code:

```python
x = [1, 2, 3, 4, 5]
sum(x) / len(x)
```
  ]
]

== Lightbox

`lightbox-image(...)` and `lightbox-video(...)` produce browser-only interactive media wrappers in HTML while degrading gracefully in paged output.

```typ
#calepin.elements.lightbox-image(
  "editor-image",
  "../assets/screenshot_notebook.png",
  "Notebook screenshot",
  width: 16em,
)
#calepin.elements.lightbox-video(
  "editor-video",
  "../assets/calepin_vscode.mp4",
  poster: "../assets/calepin_vscode-thumb.png",
  width: 16em,
)
```

#calepin.elements.columns(
  columns: 2,
  wrap: false,
  [
    #calepin.elements.lightbox-image(
      "editor-image",
      "../assets/screenshot_notebook.png",
      "Notebook screenshot",
      width: 16em,
    )
  ],
  [
    #calepin.elements.lightbox-video(
      "editor-video",
      "../assets/calepin_vscode.mp4",
      poster: "../assets/calepin_vscode-thumb.png",
      width: 16em,
    )
  ],
)

= Custom web elements

The next examples show the same UI5 pattern as before for when built-in elements are not enough.

#let load-ui5() = [
  #html.script("", src: "https://unpkg.com/@ui5/webcomponents@2.23.1/dist/Assets.js?module", type: "module",)
  #html.script("", src: "https://unpkg.com/@ui5/webcomponents@2.23.1/dist/Carousel.js?module", type: "module",)
  #html.script("", src: "https://unpkg.com/@ui5/webcomponents@2.23.1/dist/TabContainer.js?module", type: "module",)
  #html.script("", src: "https://unpkg.com/@ui5/webcomponents@2.23.1/dist/Tab.js?module", type: "module",)
]

#let carousel-image(src, alt) = html.img(
  src: src, alt: alt, style: "width: 100%; height: auto; object-fit: contain; background: #f6f8fa;",
)

#if target == "html" [
  #load-ui5()

  #html.elem("style", "
ui5-carousel:focus,
ui5-carousel:focus-visible,
ui5-carousel:focus-within,
ui5-carousel button:focus,
ui5-carousel button:focus-visible,
ui5-carousel [role='button']:focus,
ui5-carousel [role='button']:focus-visible,
")
]

== Carousel

```typ
#let carousel-image(src, alt) = html.img(
  src: src, alt: alt, style: "width: 100%; height: auto; object-fit: contain; background: #f6f8fa;",
)

#html.elem("ui5-carousel", attrs: (
  cyclic: "true",
  style: "display: block; width: 100%; max-width: 42rem; aspect-ratio: 3 / 2; height: 28rem;",
))[
  #carousel-image("../assets/flowers_01.jpg", "First flower")
  #carousel-image("../assets/flowers_05.jpg", "Third flower")
  #carousel-image("../assets/flowers_02.jpg", "Second flower")
  #carousel-image("../assets/flowers_03.jpg", "Third flower")
  #carousel-image("../assets/flowers_04.jpg", "Third flower")
]
```

#if target == "html" [
  #html.elem("ui5-carousel", attrs: (
    cyclic: "true",
    style: "display: block; width: 100%; max-width: 42rem; aspect-ratio: 3 / 2; height: 28rem;",
  ))[
    #carousel-image("../assets/flowers_01.jpg", "First flower")
    #carousel-image("../assets/flowers_05.jpg", "Third flower")
    #carousel-image("../assets/flowers_02.jpg", "Second flower")
    #carousel-image("../assets/flowers_03.jpg", "Third flower")
    #carousel-image("../assets/flowers_04.jpg", "Third flower")
  ]
]


== CSS hooks

`calepin.elements.gallery` injects its own base styles, and the lightbox/card styles are similarly opinionated. If needed, these selectors can be overridden:

```css
.calepin-elements-gallery {}
.calepin-elements-gallery__item {}
.calepin-elements-gallery__image {}
.calepin-elements-gallery__caption {}
.calepin-elements-gallery--lightbox {}
.calepin-elements-card {}
.calepin-elements-tabs {}
.calepin-screenshot-thumb {}
.calepin-screenshot-dialog {}
.calepin-video-thumb {}
.calepin-video-dialog {}
```
