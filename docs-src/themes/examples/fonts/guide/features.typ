#set document(title: [Theme Features])
#metadata((title: "Theme features", translation_key: "features")) <website-metadata>

#title()

This page gathers common content shapes that should remain usable in every
bundled theme.

= Sections

#lorem(60)

== Nested Section

#lorem(35)

= Figure

#figure(
  rect(width: 100%, height: 12em, fill: luma(92%), stroke: luma(70%)),
  caption: [A placeholder figure for checking figure width and captions.],
  alt: "A plain grey placeholder rectangle",
)

= Lists

- #lorem(8)
- #lorem(10)
- #lorem(9)

= Quote

#quote(block: true)[
  #lorem(24)
]

= Footnote

Footnotes are ordinary Typst content.#footnote[They stay portable across
HTML and paged outputs.]

#lorem(80)
