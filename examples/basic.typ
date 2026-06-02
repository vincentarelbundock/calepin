#import ".calepin/calepin.typ"

// Local aliases for the common engines used below.
#let py = calepin.chunk.with("python")
#let r = calepin.chunk.with("r")
#let sh = calepin.chunk.with("sh")

// Document-wide defaults for all chunks in this example.
#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
)

// Keep code blocks visually distinct from the surrounding prose.
#show raw.where(block: true): block.with(
  width: 100%,
  fill: luma(245),
  inset: 8pt,
  radius: 4pt,
  stroke: luma(220),
)

// A minimal chunk example.
= Calepin Typst Example

== Python

#py()[
```
print("#strong[42]")
```]

#py(echo: false, results: "asis")[
```
print("#strong[42 in Typst]")
```]

== R

#r(label: "fig-scatter", fig-cap: [Scatterplot])[
```
x <- 1:10
y <- x + rnorm(10)
plot(x, y)
```]

== Shell

#sh[
```
printf "hello $USER"
```]
