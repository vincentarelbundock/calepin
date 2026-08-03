#let _calepin-document-element = document
#import "/.calepin/calepin.typ": *
#let document = _calepin-document-element

#let _calepin-expected-generation = "5d456988768b63de-1349cde127705c16"
#let _calepin-verify-generation() = {
  let path = sys.inputs.at("calepin-results", default: none)
  if path != none and path != "" {
    let actual = json(path).at("generation", default: "")
    if actual != _calepin-expected-generation {
      panic("Calepin results changed while this render was starting; Typst will retry with the completed build")
    }
  }
}
#_calepin-verify-generation()



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
#show: calepin.document

#set document(title: [Cross-references])
#metadata((tags: ("notebooks", "cross-references"))) <website-metadata>

#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
)

#title()

Give a chunk a label and you can refer to its output from the prose with Typst's `@label` syntax. Use a recognized prefix so _Calepin_ knows what the label points at; a `fig-` label attaches to figure output.

There are three ways to attach a label. Use exactly one of them per chunk.

= `label` argument

The clearest place for a label is the `label` argument of `#calepin.chunk`, alongside the caption:

In prose we mention @fig-cross-scatter.

#calepin.chunk(label: "fig-cross-scatter", fig-caption: [Miles per gallon and horsepower])[
```r
plot(mpg ~ hp, data = mtcars)
```]

= `#|` header

Put `#| label:` at the top of a plain fenced block when you want the label next to other chunk options:

````typ
```r
#| label: fig-cross-qmd
#| fig-caption: Distribution of car weights
hist(mtcars$wt, col = "gray80", border = "white")
```
````

In prose we mention @fig-cross-qmd.

#calepin.chunk(label: "fig-cross-qmd", fig-caption: [Distribution of car weights])[
```r
hist(mtcars$wt, col = "gray80", border = "white")
```]

= Trailing fence label

For a plain fenced block, you can also write a single label right after the closing fence. This is the most compact form, equivalent to one `#| label:` header:

````typ
```r
plot(dist ~ speed, data = cars)
```<fig-cross-trailing>
```
````

In prose we mention @fig-cross-trailing.

````typ
```r
plot(dist ~ speed, data = cars)
```<fig-cross-trailing>
```
````

#calepin.chunk(label: "fig-cross-trailing", fig-caption: [Speed and stopping distance])[
```r
plot(dist ~ speed, data = cars)
```]

= Label prefixes

Use a recognized prefix so _Calepin_ knows where the label belongs. A label without a recognized prefix, such as `label: "myplot"`, is still a valid chunk identifier (you can look it up with `#calepin.results`), but it is not a cross-reference, so `@myplot` will not resolve.

#table(
  columns: (0.7fr, 1.5fr, 2fr),
  stroke: none,
  inset: 0.55em,
  [*Prefix*], [*Target*], [*Status*],
  [`fig-`], [Figure or plot output], [Supported.],
  [`tbl-`], [Table output], [Reserved for a later milestone.],
  [`lst-`], [Code listing output], [Reserved for a later milestone.],
)

`tbl-` and `lst-` labels are classified and can appear in chunk metadata today, but only `fig-` labels are attached to rendered output for now. Independent labels for sub-captioned panels (so a panel can be referenced as `@fig-name-2`) and multiple labels per chunk (`label: ("fig-name", "lst-name")`) are also planned for later milestones.
