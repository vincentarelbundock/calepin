#set document(title: "{{meta.title}}", author: "{{meta.author}}")
#set text(size: 11pt)
#set par(justify: true, leading: 0.65em)
#set heading(numbering: "1.1")

#show heading.where(level: 1): it => {
  pagebreak(weak: true)
  it
}

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
