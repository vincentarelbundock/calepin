#import "/.calepin/query-html.typ" as html

#import "/.calepin/calepin.typ" as calepin

#set document(title: [Web components])

#metadata((pdf: false)) <website-metadata>

#calepin.setup(fenced-chunks: true)

#title()

Web components are a practical way to add focused browser-only interaction to a Calepin site without turning the page into a JavaScript application. They work well for self-contained widgets such as carousels, media controls, maps, tabs, or small form controls: Typst still owns the document, while the component library handles the interactive behavior in HTML output.

Popular web component libraries include #link("https://webawesome.com/")[Web Awesome],
#link("https://shoelace.style/")[Shoelace],
#link("https://ui5.github.io/webcomponents/")[UI5 Web Components],
#link("https://opensource.adobe.com/spectrum-web-components/")[Spectrum Web Components],
and #link("https://github.com/material-components/material-web")[Material Web].
This page uses UI5 Web Components for the carousel and tab examples.
It also shows PhotoSwipe as an example of the same pattern with a small
HTML-driven JavaScript library.

#let load-ui5() = [
  #html.script("", src: "https://unpkg.com/@ui5/webcomponents@2.23.1/dist/Assets.js?module", type: "module",)
  #html.script("", src: "https://unpkg.com/@ui5/webcomponents@2.23.1/dist/Carousel.js?module", type: "module",)
  #html.script("", src: "https://unpkg.com/@ui5/webcomponents@2.23.1/dist/TabContainer.js?module", type: "module",)
  #html.script("", src: "https://unpkg.com/@ui5/webcomponents@2.23.1/dist/Tab.js?module", type: "module",)
]

#let carousel-image(src, alt) = html.img(
  src: src, alt: alt, style: "width: 100%; height: auto; object-fit: contain; background: #f6f8fa;",
)

#let load-photoswipe() = [
  #html.elem("link", "", attrs: (
    rel: "stylesheet",
    href: "https://unpkg.com/photoswipe@5.4.4/dist/photoswipe.css",
  ))
  #html.script("
import PhotoSwipeLightbox from 'https://unpkg.com/photoswipe@5.4.4/dist/photoswipe-lightbox.esm.js';

document.addEventListener('DOMContentLoaded', () => {
  const lightbox = new PhotoSwipeLightbox({
    gallery: '.calepin-photoswipe-gallery',
    children: 'a',
    pswpModule: () => import('https://unpkg.com/photoswipe@5.4.4/dist/photoswipe.esm.js')
  });
  lightbox.init();
});
", type: "module")
]

#let photoswipe-image(src, alt, width, height) = {
  html.elem("a", attrs: (
    href: src,
    target: "_blank",
    rel: "noopener",
    "data-pswp-width": repr(width),
    "data-pswp-height": repr(height),
  ))[
    #html.elem("img", "", attrs: (
      src: src,
      alt: alt,
      width: repr(width),
      height: repr(height),
      loading: "lazy",
      decoding: "async",
    ))
  ]
}

#load-ui5()

#html.elem("style", "
.calepin-photoswipe-gallery {
  columns: 3 10rem;
  column-gap: 0.75rem;
  max-width: 42rem;
  margin-block: 1rem;
}

.calepin-photoswipe-gallery a {
  display: inline-block;
  width: 100%;
  margin-block-end: 0.75rem;
  break-inside: avoid;
  overflow: hidden;
  border-radius: 0.35rem;
  background: var(--pico-muted-border-color);
  cursor: zoom-in;
}

.calepin-photoswipe-gallery img {
  display: block;
  width: 100%;
  height: auto;
}

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

= PhotoSwipe gallery

#link("https://photoswipe.com/getting-started/")[PhotoSwipe] is not a web
component, but it is useful for the same kind of progressive enhancement:
Typst writes ordinary links and images, and JavaScript turns them into an
HTML lightbox in browsers that support ES modules.

PhotoSwipe needs the full image dimensions on each link. Define the setup
and helper once:

````typ
#let load-photoswipe() = [
  #html.elem("link", "", attrs: (
    rel: "stylesheet",
    href: "https://unpkg.com/photoswipe@5.4.4/dist/photoswipe.css",
  ))
  #html.script("
import PhotoSwipeLightbox from 'https://unpkg.com/photoswipe@5.4.4/dist/photoswipe-lightbox.esm.js';

document.addEventListener('DOMContentLoaded', () => {
  const lightbox = new PhotoSwipeLightbox({
    gallery: '.calepin-photoswipe-gallery',
    children: 'a',
    pswpModule: () => import('https://unpkg.com/photoswipe@5.4.4/dist/photoswipe.esm.js')
  });
  lightbox.init();
});
", type: "module")
]

#let photoswipe-image(src, alt, width, height) = {
  html.elem("a", attrs: (
    href: src,
    target: "_blank",
    rel: "noopener",
    "data-pswp-width": repr(width),
    "data-pswp-height": repr(height),
  ))[
    #html.elem("img", "", attrs: (
      src: src,
      alt: alt,
      width: repr(width),
      height: repr(height),
      loading: "lazy",
      decoding: "async",
    ))
  ]
}

#load-photoswipe()
````

Then render the gallery as normal HTML:

```typ
#html.elem("div", attrs: (
  class: "pswp-gallery calepin-photoswipe-gallery",
))[
  #photoswipe-image("../assets/flowers_01.jpg", "First flower", 5184, 3456)
  #photoswipe-image("../assets/flowers_04.jpg", "Fourth flower", 3531, 5295)
  #photoswipe-image("../assets/flowers_02.jpg", "Second flower", 1920, 1280)
  #photoswipe-image("../assets/flowers_03.jpg", "Third flower", 2640, 1760)
  #photoswipe-image("../assets/flowers_05.jpg", "Fifth flower", 2001, 3000)
]
```

#load-photoswipe()

#html.elem("div", attrs: (
  class: "pswp-gallery calepin-photoswipe-gallery",
))[
  #photoswipe-image("../assets/flowers_01.jpg", "First flower", 5184, 3456)
  #photoswipe-image("../assets/flowers_04.jpg", "Fourth flower", 3531, 5295)
  #photoswipe-image("../assets/flowers_02.jpg", "Second flower", 1920, 1280)
  #photoswipe-image("../assets/flowers_03.jpg", "Third flower", 2640, 1760)
  #photoswipe-image("../assets/flowers_05.jpg", "Fifth flower", 2001, 3000)
]

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

To style the results on this page, we added these CSS settings to the default `calepin` theme.

```css
.calepin-photoswipe-gallery {
  columns: 3 10rem;
  column-gap: 0.75rem;
  max-width: 42rem;
  margin-block: 1rem;
}

.calepin-photoswipe-gallery a {
  display: inline-block;
  width: 100%;
  margin-block-end: 0.75rem;
  break-inside: avoid;
  overflow: hidden;
  border-radius: 0.35rem;
  background: var(--pico-muted-border-color);
  cursor: zoom-in;
}

.calepin-photoswipe-gallery img {
  display: block;
  width: 100%;
  height: auto;
}

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
