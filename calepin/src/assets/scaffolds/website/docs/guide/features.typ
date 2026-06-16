#import "/assets/site.typ" as site

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
)

= Lists

- #lorem(8)
- #lorem(10)
- #lorem(9)

= Quote

#quote(block: true)[
  #lorem(24)
]

= Margin Note

#site.margin-note[Margin notes are intentionally
plain HTML spans with a class, so custom themes can decide how to display them.]

#lorem(80)
