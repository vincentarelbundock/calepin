#import "@preview/calepin:0.0.1" as calepin

#set document(title: [Cross-references])

#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
)

#title() <cross-references>

Calepin supports three mechanisms for specifying labels: arguments in the `#calepin.chunk()` function, `#|` Quarto-style syntax at the top of the code block, and a compact trailing label on plain executable fences. Use Quarto-style prefixes so Calepin knows what the label refers to; `fig-` labels attach to figure output.

= `label` argument
<label-argument>

== Single figure
<label-argument-single-figure>

````typ
In prose we mention @fig-cross-scatter.

#calepin.chunk(label: "fig-cross-scatter", fig-caption: [Miles per gallon and horsepower],)[
```r
plot(mpg ~ hp, data = mtcars)
```]
````

In prose we mention @fig-cross-scatter.

#calepin.chunk(label: "fig-cross-scatter", fig-caption: [Miles per gallon and horsepower],)[
```r
plot(mpg ~ hp, data = mtcars)
```]

= `#|` syntax
<qmd-syntax>

== Quarto-style `#| label:`
<qmd-label-syntax>

Put `#| label:` at the top of the code block when you want the label next to
other executable-code options.

````typ
In prose we mention @fig-cross-qmd.

```r
#| fig-caption: Distribution of car weights
#| label: fig-cross-qmd
hist(mtcars$wt, col = "gray80", border = "white")
```
````

In prose we mention @fig-cross-qmd.

```r
#| fig-caption: Distribution of car weights
#| label: fig-cross-qmd
hist(mtcars$wt, col = "gray80", border = "white")
```

= Trailing fence labels
<trailing-fence-labels>

For plain executable fences, you can also put a single routed label after the
closing fence. This is equivalent to a single `#| label:` header, but more
compact.

````typ
In prose we mention @fig-cross-trailing.

```r
plot(dist ~ speed, data = cars)
```<fig-cross-trailing>
````

In prose we mention @fig-cross-trailing.

```r
plot(dist ~ speed, data = cars)
```<fig-cross-trailing>

== Label rules
<label-rules>

Use recognized prefixes so Calepin knows where the label belongs:

#table(
  columns: (0.7fr, 1.5fr, 2fr),
  stroke: none,
  inset: 0.55em,
  [*Prefix*], [*Target*], [*Status*],
  [`fig-`], [Figure or plot output], [Supported.],
  [`tbl-`], [Table output], [Reserved for table cross-references in a later milestone.],
  [`lst-`], [Code listing output], [Reserved for listing cross-references in a later milestone.],
)

== Roadmap
<cross-reference-roadmap>

Future cross-reference work includes independent labels for subcaptioned panels, so multi-plot figures can support references such as `@fig-name-2` or rendered forms like "Figure 2b". That requires subfigure semantics: each panel needs a stable label, numbering, and rendering behavior instead of only caption text inside a shared figure grid.

Unprefixed labels such as `label: "myplot"` are rejected. Panel labels such as `@fig-name-2` are planned for a later milestone.

Trailing fence labels are strict and single-label only. Use exactly one of
`label:`, `#| label:`, or a trailing fence label for any one chunk.

Multiple label kinds are also reserved for later milestones. The planned form is a label list such as `label: ("fig-name", "lst-name")`, where one chunk can carry labels for more than one output kind. Table labels with the `tbl-` prefix and listing labels with the `lst-` prefix are classified and can appear in chunk metadata today, but only `fig-` labels are attached to rendered figure output in this milestone.
