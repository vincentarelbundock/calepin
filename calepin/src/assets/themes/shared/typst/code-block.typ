#let code-block(
  body,
  fill: rgb("#f7f7f5"),
  stroke: 0.5pt + rgb("#d8d8d2"),
  radius: 2pt,
  inset: (x: 0.65em, y: 0.45em),
  text-fill: rgb("#1f2933"),
) = {
  block(
    width: 100%,
    fill: fill,
    stroke: stroke,
    radius: radius,
    inset: inset,
  )[
    #text(fill: text-fill)[#body]
  ]
}
