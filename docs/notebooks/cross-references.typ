#import "/.calepin/calepin.typ" as calepin
#show: calepin.document

#set document(title: [Cross-references])
#metadata((tags: ("notebooks", "cross-references"))) <website-metadata>

#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
)

#title()

Give a chunk a label and you can refer to its output from the prose with Typst's `@label` syntax. Use a recognized prefix so _Calepin_ knows what the label points at; a `fig-` label attaches to figure output.

There are three ways to attach a label. Use exactly one of them per chunk.

= `label` argument

The clearest place for a label is the `label` argument of `#calepin.chunk`, alongside the caption:

In prose we mention @fig-cross-scatter.

#calepin.chunk(label: "fig-cross-scatter", fig-caption: [Miles per gallon and horsepower])[
```r
plot(mpg ~ hp, data = mtcars)
```]

= `#|` header

Put `#| label:` at the top of a plain fenced block when you want the label next to other chunk options:

````typ
```r
#| label: fig-cross-qmd
#| fig-caption: Distribution of car weights
hist(mtcars$wt, col = "gray80", border = "white")
```
````

In prose we mention @fig-cross-qmd.

#calepin.chunk(label: "fig-cross-qmd", fig-caption: [Distribution of car weights])[
```r
hist(mtcars$wt, col = "gray80", border = "white")
```]

= Trailing fence label

For a plain fenced block, you can also write a single label right after the closing fence. This is the most compact form, equivalent to one `#| label:` header:

````typ
```r
plot(dist ~ speed, data = cars)
```<fig-cross-trailing>
```
````

In prose we mention @fig-cross-trailing.

````typ
```r
plot(dist ~ speed, data = cars)
```<fig-cross-trailing>
```
````

#calepin.chunk(label: "fig-cross-trailing", fig-caption: [Speed and stopping distance])[
```r
plot(dist ~ speed, data = cars)
```]

= Label prefixes

Use a recognized prefix so _Calepin_ knows where the label belongs. A label without a recognized prefix, such as `label: "myplot"`, is still a valid chunk identifier (you can look it up with `#calepin.results`), but it is not a cross-reference, so `@myplot` will not resolve.

#table(
  columns: (0.7fr, 1.5fr, 2fr),
  stroke: none,
  inset: 0.55em,
  [*Prefix*], [*Target*], [*Caption option*],
  [`fig-`], [Figure or plot output], [`fig-caption`],
  [`tbl-`], [The chunk's non-image output], [`tbl-caption`],
  [`lst-`], [The chunk's echoed source], [`lst-caption`],
)

Each kind numbers from its own Typst counter, so a document can hold Figure 1, Table 1, and Listing 1 at once.

= Tables

A `tbl-` label wraps everything the chunk printed, so it works whatever produced the table --- `knitr::kable`, a plain `print()` of a data frame, or text you assembled yourself.

In prose we mention @tbl-cross-summary.

#calepin.chunk(label: "tbl-cross-summary", tbl-caption: [First rows of `mtcars`], echo: false)[
```r
knitr::kable(head(mtcars[, 1:3]))
```]

= Listings

An `lst-` label names the code itself rather than what it produced, so the chunk must echo its source (`echo: true`, the default).

In prose we mention @lst-cross-fit.

#calepin.chunk(label: "lst-cross-fit", lst-caption: [Fitting the model], results: "hide")[
```r
fit <- lm(mpg ~ hp, data = mtcars)
```]

A chunk can carry one label per kind, so `label: ("fig-plot", "lst-plot")` names both the plot and the code that drew it.

= Panels

A chunk that draws several plots lays them out in a grid. Give it `fig-subcaptions` and each panel becomes a sub-figure, lettered within its parent and referenceable on its own: `@fig-name-1` is the first panel, `@fig-name-2` the second, and a reference reads "Figure 1b".

In prose we mention @fig-cross-panels, @fig-cross-panels-1 and @fig-cross-panels-2.

#calepin.chunk(
  label: "fig-cross-panels",
  fig-caption: [Speed and stopping distance],
  fig-subcaptions: ("Scatter", "Histogram"),
  echo: false,
)[
```r
plot(dist ~ speed, data = cars)
hist(cars$speed, col = "gray80", border = "white")
```]

A grid without sub-captions or a `fig-` label is left alone: its panels get no letters and consume no numbers.
