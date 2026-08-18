#import "/.calepin/calepin.typ" as calepin
#import "@preview/touying:0.7.4": *
#import themes.simple: *

#show: calepin.document

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
#| fig-alt-text: Scatter plot of fuel efficiency against horsepower
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
