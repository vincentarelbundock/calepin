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

*What changed*

- No Touying theme is used.
- The built-in `revealjs` Calepin theme handles slide structure.
- Slides can still mix Markdown-style content, code execution, and images.

= Visual slide with images

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

= Compute and show output

This slide runs a Python chunk and displays its output.

#calepin.chunk("python", label: "summary2")[
```python
values = [2, 3, 5, 8, 13]
total = sum(values)
print(f"Total = {total}")
```
]

= Compute in one slide; show in another

#calepin.chunk("python", label: "next-slide-claim", results: "hidden")[
```python
baseline = 120
current = 156
change = (current - baseline) / baseline
print(f"Change from baseline: {change:.0%}")
```
]

The source stays on this slide, while the result is shown in the next one.

= Final slide with a result image

#calepin.results("next-slide-claim")

#figure(
  image("/docs/assets/flowers_03.jpg", width: 72%),
  caption: [A closing visual, still fully compatible with the Reveal.js layout],
)
