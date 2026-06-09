#show raw.where(block: true): it => {
  if sys.inputs.at("calepin-target", default: "paged") == "html" {
    it
  } else {
    block(
      width: 100%,
      fill: rgb("#f7f7f5"),
      stroke: 0.5pt + rgb("#d8d8d2"),
      radius: 2pt,
      inset: (x: 0.65em, y: 0.45em),
    )[
      #text(fill: rgb("#1f2933"))[#it]
    ]
  }
}
