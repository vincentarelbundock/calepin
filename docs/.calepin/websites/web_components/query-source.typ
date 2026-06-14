#import "/.calepin/query-html.typ" as html

#import "/.calepin/calepin.typ" as calepin

#set document(title: [Web components])

#metadata((pdf: false)) <website-metadata>

#title()

Web components are a practical way to add focused browser-only interaction to a
Calepin site without turning the page into a JavaScript application. They work
well for self-contained widgets such as carousels, media controls, maps, tabs,
or small form controls: Typst still owns the document, while the component
library handles the interactive behavior in HTML output.

Popular web component libraries include #link("https://webawesome.com/")[Web Awesome],
#link("https://shoelace.style/")[Shoelace],
#link("https://ui5.github.io/webcomponents/")[UI5 Web Components],
#link("https://opensource.adobe.com/spectrum-web-components/")[Spectrum Web Components],
and #link("https://github.com/material-components/material-web")[Material Web].
This page uses UI5 Web Components for the carousel example and `ui5-tabcontainer` for tabs.

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
  --sapFontFamily: inherit;
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
  --sapFontFamily: inherit;
  --sapFontSize: 1rem;
  --sapTextColor: var(--pico-color);
  --sapContent_TextColor: var(--pico-color);
  --sapTab_TextColor: var(--pico-muted-color);
  --sapTab_Selected_TextColor: var(--pico-color);
  --sapTab_Hover_TextColor: var(--pico-color);
  --sapTab_Active_TextColor: var(--pico-color);
}

ui5-tab::part(tab),
ui5-tab::part(text) {
  color: inherit;
}

ui5-tabcontainer::part(tabStrip) {
  border-block-end: 1px solid var(--pico-muted-border-color);
}

ui5-tabcontainer::part(tabContainer) {
  background: transparent;
  color: var(--pico-color);
}
")

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

= Carousel

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
  #carousel-image("../assets/flowers_02.jpg", "Second flower")
  #carousel-image("../assets/flowers_03.jpg", "Third flower")
]
```
#html.elem("ui5-carousel", attrs: (
  cyclic: "true",
  style: "display: block; width: 100%; max-width: 42rem; aspect-ratio: 3 / 2; height: 28rem;",
))[
  #carousel-image("../assets/flowers_01.jpg", "First flower")
  #carousel-image("../assets/flowers_02.jpg", "Second flower")
  #carousel-image("../assets/flowers_03.jpg", "Third flower")
]

The Typst above produces the equivalent UI5 Web Components markup:

```html
<ui5-carousel
  cyclic="true"
  style="display: block; width: 100%; max-width: 42rem; aspect-ratio: 3 / 2; height: 28rem;"
>
  <img src="../assets/flowers_01.jpg" alt="First flower"
    style="width: 100%; height: auto; object-fit: contain; background: #f6f8fa;"
  >
  <img
    src="../assets/flowers_02.jpg" alt="Second flower"
    style="width: 100%; height: auto; object-fit: contain; background: #f6f8fa;"
  >
  <img
    src="../assets/flowers_03.jpg" alt="Third flower"
    style="width: 100%; height: auto; object-fit: contain; background: #f6f8fa;"
  >
</ui5-carousel>
```


= Tabs

`<ui5-tabcontainer>` supports the native tabs pattern, with each `<ui5-tab>`
containing its corresponding content directly:

````typ
#html.elem("ui5-tabcontainer", attrs: (
  header-background-design: "Transparent",
  content-background-design: "Transparent",
))[
  #html.elem("ui5-tab", attrs: (text: "R", selected: "true"))[
This tab includes R code:

#calepin.chunk("r")[
```r
x <- c(1, 2, 3, 4)
mean(x)
```
]
  ]

  #html.elem("ui5-tab", attrs: (text: "Python"))[
This tab includes Python code:

#calepin.chunk("python")[
```python
x = [1, 2, 3, 4]
sum(x) / len(x)
```
]
  ]
]
````

#html.elem("ui5-tabcontainer", attrs: (
  header-background-design: "Transparent",
  content-background-design: "Transparent",
))[
  #html.elem("ui5-tab", attrs: (text: "R", selected: "true"))[
This tab includes R code:

#calepin.chunk("r")[
```r
x <- c(1, 2, 3, 4, 5)
mean(x)
```
]
  ]

  #html.elem("ui5-tab", attrs: (text: "Python"))[
This tab includes Python code:

#calepin.chunk("python")[
```python
x = [1, 2, 3, 4]
sum(x) / len(x)
```
]
  ]
]

After Calepin expands the chunks, the HTML output follows the same order as the
Typst code above:

```html
<ui5-tabcontainer header-background-design="Transparent" content-background-design="Transparent">
  <ui5-tab text="R" selected>
    <p>This tab includes R code:</p>
    <!-- Calepin renders the R chunk here. -->
  </ui5-tab>
  <ui5-tab text="Python">
    <p>This tab includes Python code:</p>
    <!-- Calepin renders the Python chunk here. -->
  </ui5-tab>
</ui5-tabcontainer>
```

