#let srcbox(body) = block(
  stroke: 0.4pt + luma(200),
  inset: (x: 8pt, y: 6pt),
  width: 100%,
  body
)

#let outbox(body) = block(
  stroke: 0.4pt + luma(200),
  inset: (x: 8pt, y: 6pt),
  width: 100%,
  body
)

{{body}}
