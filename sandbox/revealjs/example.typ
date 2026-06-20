#import "/.calepin/calepin.typ" as calepin

#calepin.setup(
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

#figure(
  image("../../images/flowers_01.jpg", width: 100%),
  caption: [Local image from `sandbox/revealjs/images/`],
)

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

== Final slide

This slide confirms the local theme + config setup works in the sandbox folder.
