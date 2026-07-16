#import "/.calepin/calepin.typ" as calepin
#set document(title: [Touying slides])
#metadata((title: "Touying", pdf: false, tags: ("slides", "notebooks"))) <website-metadata>

#let target = sys.inputs.at("calepin-target", default: "paged")

#title()

This page shows a small #link("https://touying-typ.github.io/")[Touying] slide deck that uses Calepin chunks.

Touying handles the presentation structure: slides, themes, headings, and features such as `#pause`. Calepin handles code execution and stores chunk output so it can be shown later in the document.

The main interesting pattern here is delayed display. A chunk can run in one place with `results: "hidden"`, and its saved output can be printed somewhere else with `#calepin.results("label")`.

Example details:

- Plain `python` and `r` fences are executable chunks.
- The R plot uses a `#| fig-width` option.
- One Python chunk is displayed in the left column, while its output appears in the right column.
- Another Python chunk is shown on one slide, while its output appears on the next slide.
- The same saved output is printed twice at the end to show that `#calepin.results` can be reused.

= Full `slides.typ`

````typ
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
