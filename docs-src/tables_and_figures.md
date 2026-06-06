---
title: Tables and Figures
---

# Tables and figures

## Tables

Use `results: "asis"` when a chunk emits Typst markup. Add `kind: "table"` when
the result should be wrapped as a table figure with a caption and label.

````typ
#calepin.chunk(
  "r",
  label: "tbl-model",
  results: "asis",
  kind: "table",
  fig-caption: [Model summary],
)[```r
cat('#table(
  columns: 2,
  [Term], [Estimate],
  [Intercept], [37.29],
  [Weight], [-5.34],
)')
```]
````

## Multi-plot figures

Chunks that produce multiple plots can be displayed as one figure. Use
`fig-layout-columns` and `fig-layout-rows` to control the grid, and
`fig-subcaptions` to add per-panel captions.

````typ
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
````

With `fig-layout-columns: (1fr, 1fr)`, Calepin displays the four plots in a
two-column grid. The first plot is saved with the chunk label, and later plots
use numbered artifact names such as `fig-diagnostics-2.svg`.
