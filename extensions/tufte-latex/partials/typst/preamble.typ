#set document(title: "{{config.plain_title}}", author: "{{config.author}}")
#set text(font: "ETBembo", size: 11pt)
#set par(justify: true, leading: 0.65em)
#set page(margin: (inside: 1in, outside: 3.5in, top: 1in, bottom: 1in))

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
