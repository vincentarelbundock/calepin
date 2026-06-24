#import "/.calepin/calepin.typ": *



#let _raw-chunk-langs = ("python", "r", "mermaid", "dot", "tikz", "d2", "text")
#show raw.where(block: true, lang: "typ", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "typst", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "python", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("python", it) }
#show raw.where(block: true, lang: "r", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("r", it) }
#show raw.where(block: true, lang: "mermaid", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("mermaid", it) }
#show raw.where(block: true, lang: "dot", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("dot", it) }
#show raw.where(block: true, lang: "tikz", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("tikz", it) }
#show raw.where(block: true, lang: "d2", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("d2", it) }
#show raw.where(block: true, lang: "text", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("text", it) }

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

// Notebook theme
#import "/.calepin/calepin.typ": _html-themed-raw-block, chunk_from_raw_plain

#show raw.where(block: true): set text(size: .8em)

#show raw.where(block: true): it => {
  if it.theme != auto {
    it
  } else if it.lang != none and _raw-chunk-langs.contains(it.lang) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}

#set document(title: [PDF])
#import "/.calepin/calepin.typ" as calepin
#title()

`layouts/pdf.typ` is the Typst-side layout used by PDF and SVG notebook outputs.

```text
themes/my-theme/
  layouts/
    pdf.typ
```

Before Typst runs, Calepin renders this file with MiniJinja, the same engine used for HTML layouts (see #link("templating.html")[Templating]), so the output is still valid Typst source. The file uses a `.typ` extension because it renders to Typst, even though it is a MiniJinja template.

Inside the layout, place notebook content with `doc.body`:

```typ
#set page(
  paper: "us-letter",
  margin: (x: 1in, y: 0.85in),
  numbering: "1",
)

#set text(font: "Libertinus Serif", size: 10.5pt)

{{ doc.body }}
```

Useful `layouts/pdf.typ` values:

- `theme`: local theme directory name
- `target`: `paged`
- `doc.path`: `.typ` input path relative to workspace
- `doc.dir`: input directory relative to workspace
- `doc.stem`: input filename without `.typ`
- `doc.title`: document title, or an empty string
- `doc.body`: notebook body, injected as a `#include`
- `doc.meta`: values from `#metadata(...) <website-metadata>`
- `vars`: merged document variables from `[vars]` in `calepin.toml`, `calepin.setup(vars: ...)`, and CLI `--set vars.<name>=...` overrides

`doc.body` must be included explicitly if you want the notebook body in the rendered output.

`theme = "typst"` disables notebook-specific theming, and `extends = "typst"` creates a local theme with no inherited Calepin base. Use an empty `layouts/pdf.typ` for a minimal pass-through layout.
