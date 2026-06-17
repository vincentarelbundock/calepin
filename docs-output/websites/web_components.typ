#import "@preview/calepin:0.0.1" as calepin

#set document(title: [Web components])

#metadata((pdf: false)) <website-metadata>

#calepin.setup(fenced-chunks: true)

#title()

#link("https://github.com/vincentarelbundock/calepin/tree/main/calepin/src")[Calepin source] includes the `elements` namespace implementation.

Web components are a practical way to add focused browser-only interaction to a Calepin site without turning the page into a JavaScript application. They work well for self-contained widgets such as carousels, media controls, maps, tabs, or small form controls: Typst still owns the document, while the component library handles the interactive behavior in HTML output.

Popular web component libraries include #link("https://webawesome.com/")[Web Awesome],
#link("https://shoelace.style/")[Shoelace],
#link("https://ui5.github.io/webcomponents/")[UI5 Web Components],
#link("https://opensource.adobe.com/spectrum-web-components/")[Spectrum Web Components],
and #link("https://github.com/material-components/material-web")[Material Web].
This page uses UI5 Web Components for the carousel and tab examples.
It also documents `calepin.elements.gallery(...)`, which wraps gallery layout
and lightbox behavior in a reusable Typst element API.

#let load-ui5() = [
  #html.script("", src: "https://unpkg.com/@ui5/webcomponents@2.23.1/dist/Assets.js?module", type: "module",)
  #html.script("", src: "https://unpkg.com/@ui5/webcomponents@2.23.1/dist/Carousel.js?module", type: "module",)
  #html.script("", src: "https://unpkg.com/@ui5/webcomponents@2.23.1/dist/TabContainer.js?module", type: "module",)
  #html.script("", src: "https://unpkg.com/@ui5/webcomponents@2.23.1/dist/Tab.js?module", type: "module",)
]

#let carousel-image(src, alt) = html.img(
  src: src, alt: alt, style: "width: 100%; height: auto; object-fit: contain; background: #f6f8fa;",
)

#load-ui5()

#html.elem("style", "
ui5-carousel:focus,
ui5-carousel:focus-visible,
ui5-carousel:focus-within,
ui5-carousel button:focus,
ui5-carousel button:focus-visible,
ui5-carousel [role='button']:focus,
ui5-carousel [role='button']:focus-visible,
ui5-carousel [tabindex]:focus,
ui5-carousel [tabindex]:focus-visible {
  outline: none;
  box-shadow: none;
}

ui5-tabcontainer {
  display: block;
  margin-block: 1rem;
  color: var(--pico-color);
  font: inherit;
  font-family: var(--pico-font-family);
  --sapFontFamily: var(--pico-font-family);
  --sapFontSize: 1rem;
  --sapTextColor: var(--pico-color);
  --sapContent_TextColor: var(--pico-color);
  --sapTextInvertedColor: var(--pico-color);
  --sapTab_TextColor: var(--pico-muted-color);
  --sapTab_Selected_TextColor: var(--pico-color);
  --sapTab_Hover_TextColor: var(--pico-color);
  --sapTab_Active_TextColor: var(--pico-color);
  --sapList_TextColor: var(--pico-color);
  --sapList_BorderColor: var(--pico-muted-border-color);
  --sapTile_TextColor: var(--pico-color);
  --sapObjectHeader_Title_TextColor: var(--pico-color);
  --sapPage_Background: transparent;
  --sapGroup_ContentBackground: transparent;
  --sapList_Background: transparent;
  --sapList_HeaderBackground: transparent;
}

ui5-tab {
  color: var(--pico-color);
  font: inherit;
  font-family: var(--pico-font-family);
  font-weight: 400;
  --sapFontFamily: var(--pico-font-family);
  --sapFontSize: 1rem;
  --sapTextColor: var(--pico-color);
  --sapContent_TextColor: var(--pico-color);
  --sapTab_TextColor: var(--pico-muted-color);
  --sapTab_Selected_TextColor: var(--pico-color);
  --sapTab_Hover_TextColor: var(--pico-color);
  --sapTab_Active_TextColor: var(--pico-color);
}

ui5-tab p {
  font-family: var(--pico-font-family);
  font-weight: 400;
}

ui5-tabcontainer::part(tabStrip) {
  border-block-end: 1px solid var(--pico-muted-border-color);
}

ui5-tabcontainer::part(tabContainer) {
  background: transparent;
  color: var(--pico-color);
}
")

= Gallery

`calepin.elements.gallery` is the same idea with built-in defaults and cleaner
API ergonomics for Typst documents.

Pass image entries as tuples, dictionaries, or mixed lists. Include width/height
when available so the built-in lightbox receives exact aspect-ratio metadata.

````typ
#(calepin.elements.gallery)(
  (
    ("../assets/flowers_01.jpg", "First flower", [First flower], 5184, 3456),
    ("../assets/flowers_04.jpg", "Fourth flower", [Fourth flower], 3531, 5295),
    ("../assets/flowers_02.jpg", "Second flower", [Second flower], 1920, 1280),
    ("../assets/flowers_03.jpg", "Third flower", [Third flower], 2640, 1760),
    ("../assets/flowers_05.jpg", "Fifth flower", [Fifth flower], 2001, 3000),
  ),
  columns: 3,
  gap: 0.75em,
  max-width: 42em,
  show-captions: true,
)
````

Rendered example:

#(calepin.elements.gallery)(
  (
    ("../assets/flowers_01.jpg", "First flower"),
    ("../assets/flowers_04.jpg", "Fourth flower"),
    ("../assets/flowers_02.jpg", "Second flower"),
    ("../assets/flowers_03.jpg", "Third flower"),
    ("../assets/flowers_05.jpg", "Fifth flower"),
  ),
  columns: "repeat(auto-fit, minmax(180px, 1fr))",
  gap: 0.75em,
  max-width: 48em,
)

= UI5

Define helper functions for the library setup and call them once near the top of the
document:

````typ
#let load-ui5() = [
  #html.script("", src: "https://unpkg.com/@ui5/webcomponents@2.23.1/dist/Assets.js?module", type: "module",)
  #html.script("", src: "https://unpkg.com/@ui5/webcomponents@2.23.1/dist/Carousel.js?module", type: "module",)
  #html.script("", src: "https://unpkg.com/@ui5/webcomponents@2.23.1/dist/TabContainer.js?module", type: "module",)
  #html.script("", src: "https://unpkg.com/@ui5/webcomponents@2.23.1/dist/Tab.js?module", type: "module",)
]

#load-ui5()
````

== Carousel

UI5 Web Components provide navigation arrows, a page indicator, and looping
via `<ui5-carousel>`, so no custom overlay buttons or event listeners are
needed. Images are added as direct child elements, and `html.img` handles
each image directly.

```typ
#let carousel-image(src, alt) = html.img(
  src: src, alt: alt, style: "width: 100%; height: auto; object-fit: contain; background: #f6f8fa;",
)

#html.elem("ui5-carousel", attrs: (cyclic: "true",
  style: "display: block; width: 100%; max-width: 42rem; aspect-ratio: 3 / 2; height: 28rem;",
))[
  #carousel-image("../assets/flowers_01.jpg", "First flower")
  #carousel-image("../assets/flowers_05.jpg", "Third flower")
  #carousel-image("../assets/flowers_02.jpg", "Second flower")
  #carousel-image("../assets/flowers_03.jpg", "Third flower")
  #carousel-image("../assets/flowers_04.jpg", "Third flower")
]
```
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

== Tabs

`<ui5-tabcontainer>` supports the native tabs pattern, with each `<ui5-tab>` containing its corresponding content directly. To keep the syntax cleaner let's define two simple helpers:

- `tabs(...)` for the outer container (so defaults stay in one place)
- `tab(...)` for each individual tab

#let tabs(body, header-background-design: "Transparent", content-background-design: "Transparent") = {
  html.elem("ui5-tabcontainer", attrs: (
    header-background-design: header-background-design,
    content-background-design: content-background-design,
  ))[
    #body
  ]
}

#let tab(label, selected: false, body) = {
  html.elem("ui5-tab", attrs: if selected { (text: label, selected: "true") } else { (text: label) })[
    #body
  ]
}

```typ
#let tabs(body, header-background-design: "Transparent", content-background-design: "Transparent",) = {
  html.elem("ui5-tabcontainer", attrs: (
    header-background-design: header-background-design,
    content-background-design: content-background-design,
  ))[
    #body
  ]
}

#let tab(label, selected: false, body) = {
  html.elem("ui5-tab", attrs: if selected { (text: label, selected: "true") } else { (text: label) })[
    #body
  ]
}
```

Here is the rendered example chunk using those helpers:

````typ
#tabs[

#tab("R", selected: true)[
This tab includes R code:

```r
x <- c(1, 2, 3, 4, 5)
mean(x)
```
]

#tab("Python")[
This tab includes Python code:

```python
x = [1, 2, 3, 4]
sum(x) / len(x)
```

]
````

#tabs[

#tab("R", selected: true)[
This tab includes R code:

```r
x <- c(1, 2, 3, 4, 5)
mean(x)
```
]

#tab("Python")[
This tab includes Python code:

```python
x = [1, 2, 3, 4]
sum(x) / len(x)
```

]

]

= CSS

`calepin.elements.gallery` injects its own base styles from the runtime; this is
the same default used across Calepin sites and PDFs where possible.
If you need custom branding, override:

```css
.calepin-elements-gallery {}
.calepin-elements-gallery__item {}
.calepin-elements-gallery__image {}
.calepin-elements-gallery__caption {}
.calepin-elements-gallery--lightbox {}
```

```css
ui5-carousel:focus,
ui5-carousel:focus-visible,
ui5-carousel:focus-within,
ui5-carousel button:focus,
ui5-carousel button:focus-visible,
ui5-carousel [role='button']:focus,
ui5-carousel [role='button']:focus-visible,
ui5-carousel [tabindex]:focus,
ui5-carousel [tabindex]:focus-visible {
  outline: none;
  box-shadow: none;
}

ui5-tabcontainer {
  display: block;
  margin-block: 1rem;
  color: var(--pico-color);
  font: inherit;
  font-family: var(--pico-font-family);
  --sapFontFamily: var(--pico-font-family);
  --sapFontSize: 1rem;
  --sapTextColor: var(--pico-color);
  --sapContent_TextColor: var(--pico-color);
  --sapTextInvertedColor: var(--pico-color);
  --sapTab_TextColor: var(--pico-muted-color);
  --sapTab_Selected_TextColor: var(--pico-color);
  --sapTab_Hover_TextColor: var(--pico-color);
  --sapTab_Active_TextColor: var(--pico-color);
  --sapList_TextColor: var(--pico-color);
  --sapList_BorderColor: var(--pico-muted-border-color);
  --sapTile_TextColor: var(--pico-color);
  --sapObjectHeader_Title_TextColor: var(--pico-color);
  --sapPage_Background: transparent;
  --sapGroup_ContentBackground: transparent;
  --sapList_Background: transparent;
  --sapList_HeaderBackground: transparent;
}

ui5-tab {
  color: var(--pico-color);
  font: inherit;
  font-family: var(--pico-font-family);
  font-weight: 400;
  --sapFontFamily: var(--pico-font-family);
  --sapFontSize: 1rem;
  --sapTextColor: var(--pico-color);
  --sapContent_TextColor: var(--pico-color);
  --sapTab_TextColor: var(--pico-muted-color);
  --sapTab_Selected_TextColor: var(--pico-color);
  --sapTab_Hover_TextColor: var(--pico-color);
  --sapTab_Active_TextColor: var(--pico-color);
}

ui5-tab p {
  font-family: var(--pico-font-family);
  font-weight: 400;
}

ui5-tabcontainer::part(tabStrip) {
  border-block-end: 1px solid var(--pico-muted-border-color);
}

ui5-tabcontainer::part(tabContainer) {
  background: transparent;
  color: var(--pico-color);
}
```
