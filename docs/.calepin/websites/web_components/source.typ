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
This page uses Web Awesome for the concrete examples.

#let load-webawesome() = [
  #html.link(
    rel: "stylesheet",
    href: "https://ka-p.webawesome.com/kit/b9bfcf2dca544e85/webawesome@3.8.0/styles/webawesome.css",
  )

  #html.script(
    "",
    src: "https://ka-p.webawesome.com/kit/b9bfcf2dca544e85/webawesome@3.8.0/webawesome.loader.js",
    type: "module",
  )
]

#let carousel-image(src, alt) = html.elem("wa-carousel-item")[
  #html.img(
    src: src,
    alt: alt,
    style: "width: 100%; height: 100%; object-fit: contain; background: #f6f8fa;",
  )
]

#let wa-tab(name, label, body) = [
  #html.elem("wa-tab", attrs: (panel: name))[#label]
  #html.elem("wa-tab-panel", attrs: (name: name))[#body]
]

#load-webawesome()

Define a helper for the library setup and call it once near the top of the
document:

````typ
#let load-webawesome() = [
  #html.link(
    rel: "stylesheet",
    href: "https://ka-p.webawesome.com/kit/b9bfcf2dca544e85/webawesome@3.8.0/styles/webawesome.css",
  )

  #html.script(
    "",
    src: "https://ka-p.webawesome.com/kit/b9bfcf2dca544e85/webawesome@3.8.0/webawesome.loader.js",
    type: "module",
  )
]

#load-webawesome()
````

= Carousel

Web Awesome provides navigation arrows, pagination, looping, and mouse dragging
as attributes on `<wa-carousel>`, so no custom overlay buttons or event
listeners are needed. The images are wrapped in `<wa-carousel-item>` custom
elements, while `html.img` handles the ordinary image element.

````typ
#let carousel-image(src, alt) = html.elem("wa-carousel-item")[
  #html.img(
    src: src,
    alt: alt,
    style: "width: 100%; height: 100%; object-fit: contain; background: #f6f8fa;",
  )
]

#html.elem("wa-carousel", attrs: (
  navigation: "true",
  pagination: "true",
  loop: "true",
  "mouse-dragging": "true",
  style: "display: block; width: 100%; max-width: 42rem; --aspect-ratio: 3 / 2;",
))[
  #carousel-image("../assets/flowers_01.jpg", "First flower")
  #carousel-image("../assets/flowers_02.jpg", "Second flower")
  #carousel-image("../assets/flowers_03.jpg", "Third flower")
]
````

#html.elem("wa-carousel", attrs: (
  navigation: "true",
  pagination: "true",
  loop: "true",
  "mouse-dragging": "true",
  style: "display: block; width: 100%; max-width: 42rem; --aspect-ratio: 3 / 2;",
))[
  #carousel-image("../assets/flowers_01.jpg", "First flower")
  #carousel-image("../assets/flowers_02.jpg", "Second flower")
  #carousel-image("../assets/flowers_03.jpg", "Third flower")
]

The Typst above produces the same component structure as ordinary Web Awesome
markup:

```html
<wa-carousel
  navigation="true"
  pagination="true"
  loop="true"
  mouse-dragging="true"
  style="display: block; width: 100%; max-width: 42rem; --aspect-ratio: 3 / 2;"
>
  <wa-carousel-item>
    <img
      src="../assets/flowers_01.jpg"
      alt="First flower"
      style="width: 100%; height: 100%; object-fit: contain; background: #f6f8fa;"
    >
  </wa-carousel-item>
  <wa-carousel-item>
    <img
      src="../assets/flowers_02.jpg"
      alt="Second flower"
      style="width: 100%; height: 100%; object-fit: contain; background: #f6f8fa;"
    >
  </wa-carousel-item>
  <wa-carousel-item>
    <img
      src="../assets/flowers_03.jpg"
      alt="Third flower"
      style="width: 100%; height: 100%; object-fit: contain; background: #f6f8fa;"
    >
  </wa-carousel-item>
</wa-carousel>
```

= Tabs

A tab group follows the same pattern: the Web Awesome component is emitted as
custom HTML, and each tab points to a panel by name. The `active` attribute sets
the initially selected panel. Here, each panel contains a nested
`#calepin.chunk()` call with equivalent code in R and Python.

````typ
#let wa-tab(name, label, body) = [
  #html.elem("wa-tab", attrs: (panel: name))[#label]
  #html.elem("wa-tab-panel", attrs: (name: name))[#body]
]

#html.elem("wa-tab-group", attrs: (active: "python"))[
  #wa-tab("r", [R])[
This tab includes R code:

#calepin.chunk("r")[
```r
x <- c(1, 2, 3, 4)
mean(x)
```]
  ]

  #wa-tab("python", [Python])[
This tab includes Python code:

#calepin.chunk("python")[
```python
x = [1, 2, 3, 4]
sum(x) / len(x)
```]
  ]
]

````

#html.elem("wa-tab-group", attrs: (active: "python"))[
  #wa-tab("r", [R])[
This tab includes R code:

#calepin.chunk("r")[
```r
x <- c(1, 2, 3, 4)
mean(x)
```]
  ]

  #wa-tab("python", [Python])[
This tab includes Python code:

#calepin.chunk("python")[
```python
x = [1, 2, 3, 4]
sum(x) / len(x)
```]
  ]
]

After Calepin expands the chunks, the HTML output follows the same order as the
Typst code above:

```html
<wa-tab-group active="python">
  <wa-tab panel="r">R</wa-tab>
  <wa-tab-panel name="r">
    <p>This tab includes R code:</p>
    <!-- Calepin renders the R chunk here. -->
  </wa-tab-panel>

  <wa-tab panel="python">Python</wa-tab>
  <wa-tab-panel name="python">
    <p>This tab includes Python code:</p>
    <!-- Calepin renders the Python chunk here. -->
  </wa-tab-panel>
</wa-tab-group>
```
