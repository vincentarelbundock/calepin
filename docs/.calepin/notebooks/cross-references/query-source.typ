#import "/.calepin/calepin.typ" as calepin

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
  [*Prefix*], [*Target*], [*Status*],
  [`fig-`], [Figure or plot output], [Supported.],
  [`tbl-`], [Table output], [Reserved for a later milestone.],
  [`lst-`], [Code listing output], [Reserved for a later milestone.],
)

`tbl-` and `lst-` labels are classified and can appear in chunk metadata today, but only `fig-` labels are attached to rendered output for now. Independent labels for sub-captioned panels (so a panel can be referenced as `@fig-name-2`) and multiple labels per chunk (`label: ("fig-name", "lst-name")`) are also planned for later milestones.
