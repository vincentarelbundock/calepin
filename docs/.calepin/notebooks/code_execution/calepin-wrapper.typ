#let _calepin-document-element = document
#import "/.calepin/calepin.typ": *
#let document = _calepin-document-element

#let _calepin-expected-generation = "32c93bb265fc9695-1349cde127705c16"
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

#set document(title: [Code execution])
#metadata((tags: ("notebooks", "code execution"))) <website-metadata>

#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
)

#title()

This page is the reference for controlling how _Calepin_ runs chunks and displays their output. If you are starting from scratch, read #link("../getting-started/basics.html")[Basics] first for the basic document structure, runtime import, code chunks, and inline results.

= Execution model

_Calepin_ collects executable chunks before Typst renders the document, runs them, writes their results to `.calepin`, and then asks Typst to render with those results available. Each programming language runs in a persistent session for the duration of the document build, so objects created in one chunk are available in later chunks with the same engine.

Use chunk options when you need to change what runs, what is shown, or where the output appears.

To extract the source chunks into separate language-specific files without
running them, use `calepin compile document.typ --format script`. See
#link("../compile_watch_serve.html#extract-scripts")[Extract scripts] for output
templates and extension rules.

= Output elsewhere

Sometimes you want to run a chunk in one place but show its result somewhere else. Set `results: "hide"` so the chunk runs without showing anything where it is written, give it a `label`, then print its output later with `#calepin.results`:

````typ
#calepin.chunk("python", label: "summary", echo: false, results: "hide")[
```python
total = 40 + 2
print(f"The total is {total}.")
```
]

Run live, the chunk above shows nothing where it is defined, and its output appears here on request:

#calepin.results("summary")
````

`#calepin.results("summary")` prints that chunk's full output: text, figures, warnings, everything it would have shown in place. You can put the call before or after the chunk, and you can call it more than once to repeat the output.

When the chunk produces a cross-referenced figure or table (a `fig-`, `tbl-`, or `lst-` label), the `@label` reference points to where the figure is shown: the chunk's own position when it is visible, or the relocation when the chunk is hidden. Printing the same figure in more than one place and then referencing it is ambiguous, so Typst reports an error.

Run live, the hidden chunk below shows nothing where it is defined, and its output is printed on request:

#calepin.chunk("python", label: "summary", echo: false, results: "hide")[
```python
total = 40 + 2
print(f"The total is {total}.")
```
]
#calepin.results("summary")

= Supported languages

_Calepin_ has built-in engines for *Python* and *R*, and built-in diagram engines for *Mermaid*, *Graphviz DOT*, *TikZ*, and *D2*.

Any language with a #link("https://github.com/jupyter/jupyter/wiki/Jupyter-kernels")[Jupyter kernel] also works: use the kernel name as the block language. Popular examples include *Bash* (`bash`), *Julia* (`julia`), *Octave* (`octave`), *Gnuplot* (`gnuplot`), and *Ruby* (`ruby`). Install kernels as described in the #link("../getting-started/install.html#jupyter-support")[Jupyter install section].

Run `jupyter kernelspec list` to see what is registered. Whatever name appears there can be used as a block language directly:

````typ
```bash
echo "hello from bash"
```
````

= Options

Options can be set in three places: as document-wide defaults, as arguments to one chunk, or as `#|` header lines inside a block.

== Document defaults

`#calepin.setup` sets defaults for every chunk in the document:

````typ
#calepin.setup(echo: true, eval: true, results: "verbatim")
````

== Chunk arguments

`#calepin.chunk(...)` overrides options for a single chunk. Pass the body as a fenced block and _Calepin_ infers the engine from the fence:

````typ
#calepin.chunk(echo: false, results: "typst")[
```python
print("#strong[42 in Typst]")
```
]
````

#calepin.chunk(echo: false, results: "typst")[
```python
print("#strong[42 in Typst]")
```
]

== `#|` header lines

You can also place options at the top of a plain fenced block, one per line, each prefixed with `#|`:

````typ
```r
#| echo: false
#| fig-align: right
plot(mpg ~ hp, data = mtcars)
```
````

The `#|` form keeps options next to the code. Its downside is outside _Calepin_: compiling the `.typ` file directly with `typst` shows the `#|` lines as text inside the code block, while options passed to `#calepin.chunk(...)` do not.

= Options reference

== Global and chunk options

These options can be set in `#calepin.setup` as document-wide defaults and overridden per chunk.

#table(
  columns: (1.5fr, 0.9fr, 2.2fr),
  stroke: none,
  inset: 0.55em,
  [*Option*], [*Default*], [*Meaning*],
  [echo], [`true`], [Show the chunk's source code in the rendered document.],
  [eval], [`true`], [Execute the code. When `false`, nothing runs and no output is produced (the source can still be shown via `echo`).],
  [error], [`false`], [When `true`, capture an execution error and render it as output. When `false`, an error in the chunk aborts the build.],
  [warning], [`true`], [Include warnings emitted by the engine in the output. When `false`, they are suppressed.],
  [message], [`true`], [Include informational messages emitted by the engine (for example R's `message()` output). When `false`, they are suppressed.],
  [results], [`"render"`], [How results are shown: `render` (pretty display of values, images, and tables), `verbatim` (raw output in a code block), `typst` (treat output text as Typst markup and render it), or `hide` (run the code but omit its output).],
  [fig-device-format], [`"svg"`], [Format for figure files written by the engine: `svg`, `png`, `jpeg` (alias `jpg`), or `pdf`. Diagram engines always emit `svg` regardless of this setting.],
  [fig-device-dpi], [`150`], [Resolution in dots per inch for raster formats (`png`, `jpeg`). Ignored for vector formats (`svg`, `pdf`).],
  [fig-device-width], [`6`], [Width of the plotting device, in inches.],
  [fig-device-height], [`"auto"`], [Height of the plotting device, in inches. `auto` derives it from the width and `fig-device-aspect`.],
  [fig-device-aspect], [`0.618`], [Height-to-width ratio used when `fig-device-height` is `auto`: device height = `fig-device-width` × `fig-device-aspect`.],
  [fig-width], [`"70%"`], [Width of the figure as rendered in the document. Accepts a Typst length or ratio (for example `70%` or `12cm`) or `auto`.],
  [fig-height], [`"auto"`], [Height of the figure as rendered in the document. Accepts a Typst length or `auto`.],
  [fig-align], [`"center"`], [Horizontal alignment of the figure in the document: `left`, `center`, or `right`.],
  [fig-responsive], [`true`], [HTML output only: allow the figure to shrink to fit narrow viewports (sets `max-width: 100%`). No effect on paged output.],
)

== Additional chunk options

`engine`, `body`, and `label` apply only to individual chunks. The figure caption, link, accessibility, and layout options can also be passed to `#calepin.setup` when every figure should share a document-wide default. Pass `none` on one chunk to clear an inherited caption, link, alt text, subcaption, or optional layout value for that chunk.

#table(
  columns: (1.2fr, 0.9fr, 2.0fr),
  stroke: none,
  inset: 0.55em,
  [*Option*], [*Default*], [*Meaning*],
  [engine], [inferred], [Force the engine for this chunk instead of inferring it from the fence or surrounding context.],
  [body], [from fence], [Provide the raw code body directly instead of writing a fenced block.],
  [label], [auto], [Assign a stable chunk identifier used for cross-references and result lookup.],
  [fig-link], [`none`], [Wrap the rendered figure in a link to this URL.],
  [fig-caption], [`none`], [Caption text for the figure. When set, the output is wrapped in a numbered `figure` that can be cross-referenced.],
  [fig-cap-location], [`"auto"`], [Where the caption sits relative to the figure: `top`, `bottom`, or `margin`. `auto` uses Typst's default placement.],
  [fig-alt-text], [`none`], [Accessibility (alt) text for generated images. Empty when unset.],
  [fig-subcaptions], [`none`], [Per-panel captions for a multi-image chunk, given as an array of strings (one per image, in order).],
  [fig-layout-columns], [`"auto"`], [Column layout for a multi-image chunk: an integer number of equal columns, an array of explicit track sizes, or `auto` to choose a count from the number of images.],
  [fig-layout-rows], [`"auto"`], [Row layout for a multi-image chunk: an integer number of equal rows, an array of explicit track sizes, or `auto`.],
  [kind], [`none`], [Compatibility metadata carried with the chunk's display options. It is accepted in Quarto-style headers but does not currently change rendering.],
)

= Quarto-style names

Chunk options have different names in _Calepin_ and Quarto. Some Quarto aliases are accepted, but using them is not recommended, and _Calepin_ emits a warning when it meets an unsupported name. The accepted aliases are:

- `out-width` maps to `fig-width`
- `out-height` maps to `fig-height`
- `out-align` maps to `fig-align`
- `fig-alt` maps to `fig-alt-text`
- `fig-subcap` maps to `fig-subcaptions`
- `fig-format` maps to `fig-device-format`
- `fig-dpi` maps to `fig-device-dpi`
- `layout-ncol` maps to `fig-layout-columns`
- `layout-nrow` maps to `fig-layout-rows`
