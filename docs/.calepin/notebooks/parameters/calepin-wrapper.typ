#import "/.calepin/calepin.typ": *



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

#import "/.calepin/calepin.typ" as calepin_runtime
#import "/.calepin/calepin.typ" as calepin

#set document(title: [Parameters])

#calepin.setup(
  eval: true,
  echo: true,
  vars: (
    Species: "versicolor",
    min_petal_length: 4.5,
    palette: "viridis",
  ),
)

#title()

Variables let you render the same notebook with different inputs. Declare them in `#calepin.setup`, and chunks read them through a `vars` object. For the `r` and `python` engines, _Calepin_ creates the binding automatically: R receives a named `list` read as `vars$Species`, and Python receives a `dict` read as `vars["Species"]`.

This complete document filters the built-in R `iris` data to one species and a minimum petal length, then uses a third parameter to choose the color palette:

````typ
#import "/.calepin/calepin.typ" as calepin

#calepin.setup(
  vars: (
    Species: "versicolor",
    min_petal_length: 4.5,
    palette: "viridis",
  ),
)

The selected species is #calepin.inline("r")[`cat(vars$Species)`].

```r
#| fig-caption: Iris rows selected by document variables
filtered <- subset(
  iris,
  Species == vars$Species & Petal.Length >= vars$min_petal_length
)

colors <- hcl.colors(3, palette = vars$palette)

plot(
  Sepal.Length ~ Petal.Length,
  data = filtered,
  pch = 19,
  col = colors[2],
  xlab = "Petal length",
  ylab = "Sepal length",
  main = paste(vars$Species, "with Petal.Length >=", vars$min_petal_length)
)
```
````

The selected species is #calepin.inline("r")[`cat(vars$Species)`].

#calepin_runtime.chunk_from_raw_plain("r", raw("#| fig-caption: Iris rows selected by document variables\nfiltered <- subset(\n  iris,\n  Species == vars$Species & Petal.Length >= vars$min_petal_length\n)\n\ncolors <- hcl.colors(3, palette = vars$palette)\n\nplot(\n  Sepal.Length ~ Petal.Length,\n  data = filtered,\n  pch = 19,\n  col = colors[2],\n  xlab = \"Petal length\",\n  ylab = \"Sepal length\",\n  main = paste(vars$Species, \"with Petal.Length >=\", vars$min_petal_length)\n)\n", block: true, lang: "r"))

= Overriding at render time

Because variables live in `#calepin.setup`, you can override them on the command line with `--var key=value` (repeatable). This renders the same source with different inputs, without editing the notebook:

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin compile iris.typ --var Species=setosa --var min_petal_length=1.5 --var palette=magma\n", block: true, lang: "sh"))

Command-line values are typed the same way as `#|` header values, so `1.5` is a number, `true` is a boolean, and `setosa` is a string.

= Value types

A variable may be `none`, a boolean, an integer, a float, a string, an array, or a dictionary, nested freely. Other Typst values such as content, functions, lengths, colors, and dates cannot be passed directly; supply them as strings or numbers instead. An unsupported value fails the build with a message naming the offending variable.

Variables are also written to `.calepin/<document>/vars.json`. Engines reached through a Jupyter kernel, including `julia` and any other kernel, do not yet receive an automatic `vars` binding. Read that JSON file yourself using the `CALEPIN_VARS_PATH` environment variable that _Calepin_ sets in the kernel. Variables are not secret: treat them as build inputs written to disk.
