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
#show: calepin.document
````

Use that import, not Typst Universe, for now. You may see a `calepin` package on Typst Universe, but it is currently only a placeholder and should not be used while _Calepin_ is evolving quickly.

= Tinymist and plain Typst preview

After one successful single-file compile, the original notebook can be previewed directly by Tinymist or compiled by the ordinary Typst CLI without adding notebook-specific `--input` arguments:

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin compile paper.typ\n", block: true, lang: "sh"))

The generic import uses the artifacts from the most recently compiled or watched notebook when Calepin itself is not driving Typst:

````typ
#import "/.calepin/calepin.typ" as calepin
#show: calepin.document
````

The document show rule makes executable raw fences render their stored Calepin results in the original source. Ordinary Typst edits then refresh normally in the preview. Tinymist does not execute notebook code: after changing Python, R, Julia, shell, Jupyter, or diagram code, run Calepin again to refresh the stored output.

Compiling another notebook changes the generic import's active fallback. When several notebook previews must remain independent, use the generated notebook-specific facade instead:

````typ
#import "/.calepin/paper/calepin.typ" as calepin
#show: calepin.document
````

For `chapters/intro.typ`, the corresponding path is `/.calepin/chapters/intro/calepin.typ`. A notebook-specific facade always uses that notebook's artifacts and ignores which notebook is active. Both import forms continue to work when Calepin drives Typst; Calepin's internal inputs take precedence for the generic form.

Existing aliased imports and Calepin-managed compile/watch workflows remain compatible. Avoid wildcard imports such as `#import "/.calepin/calepin.typ": *`: the exported `document` adapter would shadow Typst's built-in `document` element and break rules such as `#set document(...)`.

See #link("../editors.html")[Editor integration] for VS Code, Cursor, Positron, Tinymist, and other Typst editors.

Deleting `.calepin` removes the generated runtime and results. Run `calepin compile` again before restarting the preview.

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
#show: calepin.document

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
