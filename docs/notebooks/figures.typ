#import "@preview/calepin:0.0.1" as calepin

#set document(title: [Figures and tables])

#calepin.setup(
  echo: true,
  eval: true,
  results: "render",
)

#title()

A chunk that draws a plot becomes a figure, and a chunk that prints Typst markup becomes a table. _Calepin_ runs the code, collects the output, and renders it back into the document in place.

= Figures

Any chunk that produces a plot is shown as a figure. Nothing extra is required:

```r
plot(mpg ~ hp, data = mtcars)
```

== Captions and labels

Add `fig-caption` to wrap the plot in a numbered figure. Add a `fig-` `label` so you can refer to it from the prose with `@fig-mpg` (see #link("cross-references.html")[Cross-references]).

````typ
#calepin.chunk(label: "fig-mpg", fig-caption: [Mileage and horsepower])[
```r
plot(mpg ~ hp, data = mtcars)
```
]
````

#calepin.chunk(label: "fig-mpg", echo: false, fig-caption: [Mileage and horsepower])[
```r
plot(mpg ~ hp, data = mtcars)
```
]

== Size and alignment

Set the rendered size with `fig-width` and `fig-height`, and the placement with `fig-align`. Sizes accept a Typst length or ratio, such as `50%` or `12cm`.

````typ
#calepin.chunk(fig-width: 50%, fig-align: left)[
```r
plot(mpg ~ hp, data = mtcars)
```
]
````

#calepin.chunk(echo: false, fig-width: 50%, fig-align: left)[
```r
plot(mpg ~ hp, data = mtcars)
```
]

In HTML output, `fig-responsive` (on by default) lets a figure shrink to fit a narrow screen.

== Display vs. device

A figure has two sizes that do different jobs. The _device_ is the canvas the engine draws on, set in inches with `fig-device-width`, `fig-device-height`, and `fig-device-aspect`. The _display_ size is how large the finished image appears in the document, set with `fig-width` and `fig-height`.

Text, points, and line widths are drawn at fixed sizes on the device, and the whole canvas is then scaled to the display size. A larger device fits more inches into the same displayed width, so these elements end up looking smaller; a smaller device makes them look larger. To enlarge labels and points, shrink the device rather than the display width.

Here is the same plot on a wide device and a narrow one, both shown at the same display width. Only `fig-device-width` differs:

````typ
#calepin.chunk(fig-device-width: 8, fig-width: 55%)[
```r
plot(mpg ~ hp, data = mtcars, pch = 19)
```
]
````

#calepin.chunk(echo: false, fig-device-width: 8, fig-width: 55%, fig-caption: [Wide device (8 in): smaller text and points])[```r
plot(mpg ~ hp, data = mtcars, pch = 19)
```]

#calepin.chunk(echo: false, fig-device-width: 4, fig-width: 55%, fig-caption: [Narrow device (4 in): larger text and points])[```r
plot(mpg ~ hp, data = mtcars, pch = 19)
```]

See #link("code_execution.html")[Code execution] for the full list of device settings.

== Image format

Figures are written as SVG by default. Switch to a raster format with `fig-device-format`, and set its resolution with `fig-device-dpi`:

````typ
#calepin.chunk(fig-device-format: "png", fig-device-dpi: 200)[
```r
plot(mpg ~ hp, data = mtcars)
```
]
````

== Multiple panels

A chunk that draws several plots can lay them out as one figure. Use `fig-layout-columns` and `fig-layout-rows` for the grid, and `fig-subcaptions` for per-panel captions:

````typ
#calepin.chunk(
  label: "fig-diagnostics",
  fig-caption: [Regression diagnostics],
  fig-subcaptions: (
    [Residuals vs fitted],
    [Normal Q-Q],
    [Scale-location],
    [Cook's distance],
  ),
  fig-layout-columns: (1fr, 1fr),
)[```r
model <- lm(mpg ~ wt + hp, data = mtcars)
plot(model, which = 1)
plot(model, which = 2)
plot(model, which = 3)
plot(model, which = 4)
```]
````

#calepin.chunk(
  label: "fig-diagnostics",
  echo: false,
  fig-caption: [Regression diagnostics],
  fig-subcaptions: (
    [Residuals vs fitted],
    [Normal Q-Q],
    [Scale-location],
    [Cook's distance],
  ),
  fig-layout-columns: (1fr, 1fr),
)[```r
model <- lm(mpg ~ wt + hp, data = mtcars)
plot(model, which = 1)
plot(model, which = 2)
plot(model, which = 3)
plot(model, which = 4)
```]

With two columns, the four plots fill a two-by-two grid. The first plot is saved under the chunk label; later plots use numbered names such as `fig-diagnostics-2.svg`.

= Tables

A table is Typst markup produced by your code. Set `results: "typst"` so _Calepin_ passes that markup through to the document instead of showing it as plain text.

The R #link("https://vincentarelbundock.github.io/tinytable/")[tinytable] package can print a styled table as Typst:

````typ
#calepin.chunk(results: "typst")[
```r
library(tinytable)
tt(head(iris), caption = "First rows of iris") |>
  style_tt(i = 1:2, j = 2:3, background = "teal", color = "white") |>
  print("typst")
```
]
````

#calepin.chunk(echo: false, results: "typst")[
```r
library(tinytable)
tt(head(iris), caption = "First rows of iris") |>
  style_tt(i = 1:2, j = 2:3, background = "teal", color = "white") |>
  print("typst")
```
]

At the time of writing, Typst's HTML export does not support the styled table above. Switch to PDF output to see the full effect.
