#let _calepin-document-element = document
#import "/.calepin/calepin.typ": *
#let document = _calepin-document-element

#let _calepin-expected-generation = "b2bd747ef85cc721-1349cde127705c16"
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



#let _raw-chunk-langs = ("python", "r", "mermaid", "dot", "tikz", "d2", "sh")
#show raw.where(block: true, lang: "typ", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "typst", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "python", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("python", it) }
#show raw.where(block: true, lang: "r", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("r", it) }
#show raw.where(block: true, lang: "mermaid", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("mermaid", it) }
#show raw.where(block: true, lang: "dot", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("dot", it) }
#show raw.where(block: true, lang: "tikz", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("tikz", it) }
#show raw.where(block: true, lang: "d2", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("d2", it) }
#show raw.where(block: true, lang: "sh", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("sh", it) }

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

#import "/.calepin/calepin.typ" as calepin_runtime
#import "/.calepin/calepin.typ" as calepin

#set document(title: [Basics])
#metadata((tags: ("getting started", "notebooks", "overview"))) <website-metadata>

#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
)

#title()

_Calepin_ turns ordinary Typst documents into computational notebooks. You write prose, equations, figures, tables, and page layout in Typst, then place executable code directly in the same `.typ` file. When you run `calepin compile notebook.typ`, _Calepin_ scans the document for chunks, runs them, stores their results, and asks Typst to render the final document with those results inserted in place.

= Standard Typst

A _Calepin_ notebook is still a standard Typst document. Headings, paragraphs, lists, math, links, functions, variables, and layout rules are normal Typst. The notebook behavior comes from a small set of imported helper functions and from fenced code blocks that _Calepin_ sees before Typst renders the document.

That means a notebook can begin like any other Typst file:

````typ
#import "/.calepin/calepin.typ" as calepin
#show: calepin.document

#set document(title: [My analysis])

= Introduction

This is regular Typst prose. The expression $pi r^2$ is regular Typst math.
````

During compilation, _Calepin_ creates a regenerable `.calepin` directory beside the document. The generic import above loads the generated Typst runtime and stored results from that directory.

Use this generated import rather than the `calepin` placeholder package on Typst Universe.

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

The #link("../index.html#a-simple-computational-notebook")[simple computational notebook] on the homepage combines these pieces in one complete, copyable file.

= Plain Typst preview

Run Calepin once to generate the runtime and results:

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin compile paper.typ\n", block: true, lang: "sh"))

Afterward, ordinary Typst tooling can render the original notebook without extra `--input` arguments:

#calepin_runtime.chunk_from_raw_plain("sh", raw("typst compile paper.typ\n", block: true, lang: "sh"))

The generic import uses the artifacts from the most recently compiled or watched notebook when Calepin itself is not driving Typst. The document show rule replaces executable raw fences with their stored results, so ordinary Typst edits refresh normally in the preview.

Typst tooling does not evaluate notebook code. After changing Python, R, Julia, shell, Jupyter, or diagram code, run Calepin again, or keep `calepin watch paper.typ --eval-only` running to refresh the results when the computational fingerprint changes.

Compiling another notebook changes the generic import's active fallback. When several notebook previews must remain independent, use the generated notebook-specific facade:

````typ
#import "/.calepin/paper/calepin.typ" as calepin
#show: calepin.document
````

For `chapters/intro.typ`, the corresponding path is `/.calepin/chapters/intro/calepin.typ`. A notebook-specific facade always uses that notebook's artifacts and ignores which notebook is active. Both import forms continue to work when Calepin drives Typst; Calepin's internal inputs take precedence for the generic form.

Avoid wildcard imports such as `#import "/.calepin/calepin.typ": *`: the exported `document` adapter would shadow Typst's built-in `document` element and break rules such as `#set document(...)`.

See #link("../editors.html")[Editor integration] for editor-specific setup. Deleting `.calepin` removes the generated runtime and results; run `calepin compile` again before restarting the preview.
