#set document(title: "{{plain_title}}", author: "{{author}}")
#set text(size: 11pt)
#set par(justify: true, leading: 0.65em)
#set heading(numbering: "1.1")

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
