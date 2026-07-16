#import "/.calepin/calepin.typ" as calepin

#set document(title: [Custom web elements])
#metadata((title: "Custom elements", pdf: false, tags: ("websites", "HTML", "elements"))) <website-metadata>

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
