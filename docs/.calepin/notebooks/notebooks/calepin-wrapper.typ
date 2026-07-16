#import "/.calepin/calepin.typ": *



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
#import "/.calepin/calepin.typ": _html-themed-raw-block, chunk_from_raw_plain

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
  } else if it.lang != none and _raw-chunk-langs.contains(it.lang) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}

#context _calepin-body-size.update(text.size)

#import "/.calepin/calepin.typ" as calepin_runtime
#import "/.calepin/calepin.typ" as calepin

#set document(title: [Notebooks])
#metadata((tags: ("notebooks", "overview"))) <website-metadata>

#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
)

#title()

_Calepin_ turns ordinary Typst documents into computational notebooks. You write prose, equations, figures, tables, and page layout in Typst, then place executable code directly in the same `.typ` file. When you run `calepin compile notebook.typ`, _Calepin_ scans the document for chunks, runs them, stores their results, and asks Typst to render the final document with those results inserted in place.

During compilation, _Calepin_ creates a hidden `.calepin` directory beside your document. That directory is an implementation detail and can be regenerated, but it contains the Typst runtime file that notebooks use while rendering. Every _Calepin_ notebook should import those Typst functions at the top of the document:

````typ
#import "/.calepin/calepin.typ" as calepin
````

Use that import, not Typst Universe, for now. You may see a `calepin` package on Typst Universe, but it is currently only a placeholder and should not be used while _Calepin_ is evolving quickly.

= Standard Typst

A _Calepin_ notebook is still a standard Typst document. Headings, paragraphs, lists, math, links, functions, variables, and layout rules are normal Typst. The notebook behavior comes from a small set of imported helper functions and from fenced code blocks that _Calepin_ sees before Typst renders the document.

That means a notebook can begin like any other Typst file:

````typ
#import "/.calepin/calepin.typ" as calepin

#set document(title: [My analysis])

= Introduction

This is regular Typst prose. The expression $pi r^2$ is regular Typst math.
````

= Code chunks

The simplest executable chunk is an ordinary fenced code block named after its language. _Calepin_ runs the block and places the captured output below the source:

````typ
```python
print(40 + 2)
```
````

#calepin_runtime.chunk_from_raw_plain("python", raw("print(40 + 2)\n", block: true, lang: "python"))

Languages run in persistent sessions, so variables defined in one chunk are available in later chunks:

````typ
```python
x = 41
```

```python
print(x + 1)
```
````

#calepin_runtime.chunk_from_raw_plain("python", raw("x = 41\n", block: true, lang: "python"))

#calepin_runtime.chunk_from_raw_plain("python", raw("print(x + 1)\n", block: true, lang: "python"))

Set document-wide defaults with `#calepin.setup(...)`. For example, this page uses `results: "verbatim"` so console output is shown as plain text. Other pages use `results: "render"` when plots, rich values, or Typst output should be rendered more fully.

= Inline results

Inline code is for computed values that belong inside a sentence. The common pattern is to create a short alias once, then call it where the result should appear:

````typ
#let py = calepin.inline.with("python")

The answer is #py[`print(40 + 2)`].
````

#let py = calepin.inline.with("python")

The answer is #py[`print(40 + 2)`].

= Full example

Here is a complete starter notebook with comments. Save it as `notebook.typ`, run `calepin compile notebook.typ`, and open the generated output.

````typ
// Import the Calepin Typst runtime generated in .calepin/.
#import "/.calepin/calepin.typ" as calepin

// This is regular Typst metadata.
#set document(title: [My Calepin notebook])

// Defaults apply to every chunk unless a chunk overrides them.
#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
)

// A short alias keeps inline code readable in prose.
#let py = calepin.inline.with("python")

= My Calepin notebook

This paragraph is ordinary Typst.

// A plain fenced code block is executable when its language is supported.
```python
x = 41
print(x + 1)
```

Variables persist across chunks in the same language session:

```python
print(x + 2)
```

Inline results can appear inside prose. The answer is #py[`print(40 + 2)`].

// Use calepin.chunk(...) when a block needs options.
#calepin.chunk(echo: false, results: "typst")[
```python
print("#strong[This text was produced by Python and rendered by Typst.]")
```
]
````
