#import "/.calepin/calepin.typ": *



#let _raw-chunk-langs = ("python", "r", "mermaid", "dot", "tikz", "d2", "css")
#show raw.where(block: true, lang: "typ", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "typst", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "python", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("python", it) }
#show raw.where(block: true, lang: "r", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("r", it) }
#show raw.where(block: true, lang: "mermaid", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("mermaid", it) }
#show raw.where(block: true, lang: "dot", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("dot", it) }
#show raw.where(block: true, lang: "tikz", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("tikz", it) }
#show raw.where(block: true, lang: "d2", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("d2", it) }
#show raw.where(block: true, lang: "css", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("css", it) }

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

#show raw.where(block: true): set text(size: .8em)

#show raw.where(block: true): it => {
  if it.theme != auto {
    it
  } else if it.lang != none and _raw-chunk-langs.contains(it.lang) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}

#import "/.calepin/calepin.typ" as calepin

#set document(title: [Custom web elements])
#metadata((title: "Custom elements", pdf: false)) <website-metadata>

#calepin.setup(fenced-chunks: true)
#let target = sys.inputs.at("calepin-target", default: "paged")

#title()

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

= Carousel

```typ
#let carousel-image(src, alt) = html.img(
  src: src, alt: alt, style: "width: 100%; height: auto; object-fit: contain; background: #f6f8fa;",
)

#html.elem("ui5-carousel", attrs: (
  cyclic: "true",
  style: "display: block; width: 100%; max-width: 42rem; aspect-ratio: 3 / 2; height: 28rem;",
))[
  #carousel-image("../assets/flowers_01.jpg", "First flower")
  #carousel-image("../assets/flowers_05.jpg", "Fifth flower")
  #carousel-image("../assets/flowers_02.jpg", "Second flower")
  #carousel-image("../assets/flowers_03.jpg", "Third flower")
  #carousel-image("../assets/flowers_04.jpg", "Fourth flower")
]
```

#if target == "html" [
  #html.elem("ui5-carousel", attrs: (
    cyclic: "true",
    style: "display: block; width: 100%; max-width: 42rem; aspect-ratio: 3 / 2; height: 28rem;",
  ))[
    #carousel-image("../assets/flowers_01.jpg", "First flower")
    #carousel-image("../assets/flowers_05.jpg", "Fifth flower")
    #carousel-image("../assets/flowers_02.jpg", "Second flower")
    #carousel-image("../assets/flowers_03.jpg", "Third flower")
    #carousel-image("../assets/flowers_04.jpg", "Fourth flower")
  ]
]


= CSS hooks

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
