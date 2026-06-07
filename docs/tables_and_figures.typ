#import ".calepin/calepin.typ"

#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
  raw-chunks: false,
)

= Tables and figures
<tables-and-figures>

== Tables
<tables>

Tinytable emits raw Typst content directly. Keep `results: "asis"` to pass it
through into the document:

````typ
#calepin.chunk(results: "asis")[
```r
library(tinytable)
dat <- head(iris)
tt(dat, caption = "Hello world!") |> 
  style_tt(i = 1:2, j = 2:3, background = "teal", color = "white") |>
  print('typst')
```
]
````

#calepin.chunk(echo: false, results: "asis")[
```r
library(tinytable)
dat <- head(iris)
tt(dat, caption = "Hello world!") |> 
  style_tt(i = 1:2, j = 2:3, background = "teal", color = "white") |>
  print('typst')
```
]

== Multi-plot figures
<multi-plot-figures>

Chunks that produce multiple plots can be displayed as one figure. Use
`fig-layout-columns` and `fig-layout-rows` to control the grid, and
`fig-subcaptions` to add per-panel captions.

````typ
#calepin.chunk(
  fig-caption: [Regression diagnostics],
  fig-subcaptions: (
    [Residuals vs fitted],
    [Normal Q-Q],
    [Scale-location],
    [Cook's distance],
  ),
  fig-layout-columns: (1fr, 1fr),
  fig-layout-rows: (auto, auto),
  fig-display-width: 90%,
)[```r
model <- lm(mpg ~ wt + hp, data = mtcars)

plot(model, which = 1)
plot(model, which = 2)
plot(model, which = 3)
plot(model, which = 4)
```]
````

#calepin.chunk(
  "r",
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
  fig-layout-rows: (auto, auto),
  fig-display-width: 90%,
)[```r
model <- lm(mpg ~ wt + hp, data = mtcars)

plot(model, which = 1)
plot(model, which = 2)
plot(model, which = 3)
plot(model, which = 4)
```]

With `fig-layout-columns: (1fr, 1fr)`, Calepin displays the four plots
in a two-column grid. The first plot is saved with the chunk label, and
later plots use numbered artifact names such as `fig-diagnostics-2.svg`.
