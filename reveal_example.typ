#import "/.calepin/calepin.typ" as calepin

#calepin.setup(
  theme: "themes/revealjs-pico",
  echo: true,
  results: "render",
)

#show raw.where(block: true): set text(size: .8em)

= Reveal.js + Calepin

This deck mirrors the structure from `docs/slides/touying.typ`: horizontal slides are
`h1` headings and vertical slides are `h2` headings.

== What changed

- No Touying theme is used.
- The built-in `revealjs` Calepin theme handles slide structure.
- Slides can still mix Markdown-style content, code execution, and images.

= Visual slide with images

== Left and right visuals

#calepin.elements.columns(
  columns: 2,
  [
    #figure(
      image("/docs/assets/flowers_01.jpg", width: 100%),
      caption: [Left image: rose field],
    )
  ],
  [
    #figure(
      image("/docs/assets/flowers_02.jpg", width: 100%),
      caption: [Right image: petals in daylight],
    )
  ],
)

= Compute in one column and reuse output

== Keep output hidden, place it elsewhere

A `results: "hidden"` chunk can be rendered with `#calepin.results(label)` later
in the same slide, or in another slide.

#grid(
  columns: (1fr, 1fr),
  gutter: .4em,
  [
    === Column 1: Code
    #calepin.chunk("python", label: "summary2", results: "hidden")[
```python
values = [2, 3, 5, 8, 13]
total = sum(values)
print(f"Total = {total}")
```
    ]
  ],
  [
    === Column 2: Output
    #calepin.results("summary2")
  ],
)

== Output on a follow-up slide

#calepin.chunk("python", label: "next-slide-claim", results: "hidden")[
```python
baseline = 120
current = 156
change = (current - baseline) / baseline
print(f"Change from baseline: {change:.0%}")
```
]

The source stays on this slide, while the result is shown in the next one.

= Follow-up slide

== Final slide with a result image

#calepin.results("next-slide-claim")

#figure(
  image("/docs/assets/flowers_03.jpg", width: 72%),
  caption: [A closing visual, still fully compatible with the Reveal.js layout],
)
