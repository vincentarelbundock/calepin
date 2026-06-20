#import "/.calepin/calepin.typ" as calepin

#calepin.setup(
  theme: "./theme-reveal",
  echo: true,
  results: "render",
)

#show raw.where(block: true): set text(size: .8em)

= Reveal.js + Calepin

This deck uses Touying-style headings: `=` starts a section, and `==` starts a slide inside that section.

== What changed

- No Touying theme is used.
- The built-in `revealjs` Calepin theme handles slide structure.
- Slides can still mix Markdown-style content, code execution, and images.

= Visuals

== Visual slide with images

#calepin.elements.columns(
  columns: 2,
  [
    #figure(
      image("../../images/flowers_01.jpg", width: 100%),
      caption: [Left image: rose field],
    )
  ],
  [
    #figure(
      image("../../images/flowers_02.jpg", width: 100%),
      caption: [Right image: petals in daylight],
    )
  ],
)

= Math

== Typst math formulas

A nice identity is Euler’s formula:

$ e^(i*theta) = cos(theta) + i sin(theta) $

And the quadratic formula:

$ x = (-b \pm sqrt(b^2 - 4*a*c)) / (2*a) $

= Computation

== Compute and show output

This slide runs a Python chunk and displays its output.

#calepin.chunk("python", label: "summary2")[
```python
values = [2, 3, 5, 8, 13]
total = sum(values)
print(f"Total = {total}")
```
]

== Compute in one slide; show in another

#calepin.chunk("python", label: "next-slide-claim", results: "hidden")[
```python
baseline = 120
current = 156
change = (current - baseline) / baseline
print(f"Change from baseline: {change:.0%}")
```
]

The source stays on this slide, while the result is shown in the next one.

== Delayed reveal of the result

#calepin.results("next-slide-claim")

= Closing

== Oh yeah Final slide with a result image

#figure(
  image("../../images/flowers_03.jpg", width: 60%),
  caption: [A closing visual, still fully compatible with the Reveal.js layout],
)
