#set document(title: [Touying slides])
#metadata((title: "Touying", pdf: false)) <website-metadata>

#let target = sys.inputs.at("calepin-target", default: "paged")

#title()

Slides are often where computational notebooks feel most useful: the audience sees the result, while the author keeps the code, parameters, and generated figures close to the claim being made. #link("https://touying-typ.github.io/")[Touying] gives the deck structure, and Calepin adds live code execution without leaving Typst.

Touying is a natural slide layer for Calepin because it keeps presentations in Typst's normal markup while adding slide-specific features: heading-based decks or explicit `#slide[...]` blocks, built-in themes, fast Typst previews, progressive reveals with `#pause`, and the rest of the Typst package ecosystem. Calepin can then focus on the notebook part: running code, collecting figures and text output, and making those results available to the rendered deck.

This example shows a small but important workflow: run a chunk where it is easiest to explain, then display its output where it best supports the presentation. The code can stay visible in one column while the result appears in another, or a slide can introduce a calculation and delay its output until the next slide.

What to notice:

- Plain `python` and `r` fences are executable chunks, so the slide source stays close to ordinary Typst.
- `#|` chunk options travel with the code; the R plot uses `fig-width` to control its displayed size.
- `results: "hidden"` runs a chunk but suppresses output at the chunk location.
- `#calepin.results("label")` prints a saved chunk result later, which lets you place output in a different column or on a different slide.
- The same delayed result can be reused more than once when that is useful for the story.

= Full `slides.typ`

````typ
#import "/.calepin/calepin.typ" as calepin
#import "@preview/touying:0.7.4": *
#import themes.simple: *

#import "/.calepin/calepin.typ" as calepin
#import "@preview/touying:0.7.4": *
#import themes.simple: *

#set document(title: [Calepin Touying slides])

#show: simple-theme.with(aspect-ratio: "16-9")

#show raw.where(block: true): set text(size: .8em)

#title-slide[
  #calepin.setup(
    echo: true,
    eval: true,
    results: "render",
  )
  _Calepin_: Code execution in Touying slides
]

== Print from Python

```python
print("Hello, Touying!")
```

== Plot from R

```r
#| fig-width: 50%
plot(mpg ~ hp, data = mtcars)
```

== Compute in one column; show in another

Use the `results: "hidden"` option to keep the output hidden from the code location, then call it later with `calepin.results(label)` in the other column.

#grid(columns: (1fr, 1fr), gutter: .4em, [
=== Column 1: Code here
#calepin.chunk("python", label: "summary2", results: "hidden")[
```python
values = [2, 3, 5, 8, 13]
total = sum(values)
print(f"Total = {total}")
```
]
],

[
=== Column 2: Output here
#calepin.results("summary2")
],
)

== Compute in one slide; show in another

#calepin.chunk("python", label: "next-slide-claim", results: "hidden")[
```python
baseline = 120
current = 156
change = (current - baseline) / baseline
print(f"Change from baseline: {change:.0%}")
```
]

The chunk source is visible here, but `results: "hidden"` keeps its output off
this slide.

== Result on the next slide

#calepin.results("next-slide-claim")

The same label can be reused multiple times, anywhere in the deck.

#calepin.results("next-slide-claim")
````

= Rendered PDF

#if target == "html" [
  #html.elem("iframe", attrs: (
    src: "../assets/slides.pdf",
    title: "Rendered Touying slides PDF",
    style: "display: block; width: 100%; height: min(78vh, 52rem); border: 1px solid var(--pico-muted-border-color); border-radius: var(--pico-border-radius);",
  ))[]
] else [
  #link("../assets/slides.pdf")[Open the rendered slides PDF.]
]
