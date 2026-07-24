#let _calepin-document-element = document
#import "/.calepin/calepin.typ": *
#let document = _calepin-document-element



#let _raw-chunk-langs = ("python", "r", "mermaid", "dot", "tikz", "d2")
#show raw.where(block: true, lang: "typ", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "typst", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "python", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("python", it) }
#show raw.where(block: true, lang: "r", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("r", it) }
#show raw.where(block: true, lang: "mermaid", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("mermaid", it) }
#show raw.where(block: true, lang: "dot", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("dot", it) }
#show raw.where(block: true, lang: "tikz", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("tikz", it) }
#show raw.where(block: true, lang: "d2", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("d2", it) }

#show raw.where(block: true, theme: auto): it => {
  if _is-query() {
    it
  } else if _disable-raw-chunk-transforms.get() {
    _html-themed-raw-block(it)
  } else if it.has("lang") and it.lang != none and _raw-chunk-langs.contains(it.lang) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}

#show heading: it => {
  if _is-html() and "label" in it.fields() {
    std.html.elem("calepin-heading-anchor", attrs: (data-id: str(it.label)))
  }
  it
}

// Notebook theme
#import "/.calepin/calepin.typ": _html-themed-raw-block, _is-query, chunk_from_raw_plain

// Body text size, captured below at document-body level. Code blocks are sized
// relative to this rather than to `1em`, which would compound: a literal
// ```typ block is rendered by replacing its source `raw` element, so it renders
// inside Typst's already-reduced raw text context, whereas executed chunks are
// emitted as ordinary calls at body size. Anchoring to the captured body size
// gives both paths a single, matching reduction instead of shrinking twice.
#let _calepin-body-size = std.state("calepin-body-size", 11pt)

#show raw.where(block: true): it => {
  if it.theme != auto {
    context {
      set text(size: _calepin-body-size.get() * 0.8)
      it
    }
  } else if it.lang != none and (_is-query() or _raw-chunk-langs.contains(it.lang)) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}

#context _calepin-body-size.update(text.size)

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
