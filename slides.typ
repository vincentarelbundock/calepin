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
  Calepin results in Touying
]

== Calepin results in Touying

Run Python on one slide, then place the output exactly where the presentation needs it.

```python
print("Hello, Touying!")
```

== Output in the next column

#grid(
  columns: (1fr, 1fr),
  gutter: 1.2em,
[
=== Column 1: Code goes here
#calepin.chunk("python", label: "column-summary", results: "hidden")[
```python
values = [2, 3, 5, 8, 13]
total = sum(values)
print(f"Total = {total}")
```
]
],

[
=== Column 2: Output appears here
#calepin.results("column-summary")
],
)

== Compute now, show later

#calepin.chunk("python", label: "next-slide-claim", results: "hidden")[
```python
baseline = 120
current = 156
change = (current - baseline) / baseline
print(f"Current value: {current}")
print(f"Change from baseline: {change:.0%}")
```
]

The chunk source is visible here, but `results: "hidden"` keeps its output off
this slide.

== Result on the next slide

#calepin.results("next-slide-claim")

The same label can be reused anywhere the deck needs that computed result.
