#import ".calepin/calepin.typ"

#set document(
  title: [#emph[Calepin]: Computational notebooks in Typst],
)

// Document-wide defaults for all chunks in this example.
#calepin.setup(
  echo: true,
  eval: true,
  results: "render",
)

#title()

This notebook demonstrates the #emph[Calepin] executable Typst workflow: code chunks and inline snippets are collected during preprocessing, run with the requested engine, and rendered back into the document alongside the source that produced them.

= Language-specific settings

`#calepin.setup()` sets document-wide defaults for all chunks unless overridden per chunk.

#calepin.chunk("python", label: "python-lang-default")[
```python
print(40 + 2)
```
]

Define a short alias for inline Python expressions:

// A short alias avoids spelling out calepin.inline("python") inside sentences.
#let py = calepin.inline.with("python")

= Python

Python is a general-purpose programming language; learn more at #link("https://www.python.org/")[python.org].

Start with a visible chunk when readers should see both the source and the
captured output. This is the default notebook style and is useful while building
up an example step by step.

```python
print("#strong[42]")
```

When the generated result should become part of the Typst document, set
`results` to `"typst"`. Hiding the source lets the rendered document read like
authored Typst while still keeping the value produced by the chunk.

#calepin.chunk(echo: false, results: "typst")[
```python
print("#strong[42 in Typst]")
```
]

Inline snippets are for values that belong inside a sentence. They keep prose and executable code close together without introducing a full block.

The inline Python result is #py[`print("42")`].

For graphical output, put the label and caption on the chunk itself. This keeps
the figure metadata beside the code that creates the figure.

#calepin.chunk(label: "fig-plotnine", fig-caption: [Plotnine scatterplot])[
```python
from plotnine import ggplot, aes, geom_point, labs
from plotnine.data import mtcars

(
    ggplot(mtcars, aes("mpg", "hp"))
    + geom_point(color = "orange")
    + labs(x="Miles per gallon", y="Horsepower")
).show()
```
]

= R

R is a language and environment for statistical computing; learn more at #link("https://www.r-project.org/")[r-project.org].

The R interface follows the same pattern as the Python interface. Use inline snippets for compact values that should appear directly in the surrounding paragraph.

The inline R result is #calepin.inline("r")[`cat("42")`].

Use a regular chunk when the source and captured console output should be shown together. The document structure stays the same even though the execution engine changes.

```r
mod <- lm(hp ~ mpg, data = mtcars)
summary(mod)
```

Chunks that produce graphics can carry the same label and caption fields. This makes figures portable across engines and keeps the rendered output easy to reference later.

#calepin.chunk(label: "fig-scatter", fig-caption: [Scatterplot])[
```r
plot(hp ~ mpg, data = mtcars)
```
]

= Julia

Julia is a language for technical and scientific computing; learn more at #link("https://julialang.org/")[julialang.org].

Julia runs through a Jupyter kernel. Use the kernel name reported by
`jupyter kernelspec list`; this example uses `julia-1.12`.

The inline Julia result is #calepin.inline("julia-1.12")[`println(42)`].

For block output, pass the Jupyter kernel name as the chunk engine.

#calepin.chunk("julia-1.12")[
```julia
x = 40
println(x + 2)
```
]

= Shell

Shell chunks run through the Bash Jupyter kernel. They behave like the other
engines: the command is collected, executed, and its captured output is placed
back into the document.

#calepin.chunk("bash")[
```bash
printf "hello from bash\n"
```
]

= Mermaid

Mermaid is a text-based diagramming tool; learn more at #link("https://mermaid.js.org/")[mermaid.js.org].

Diagram engines use chunk bodies too, but the body is diagram source instead of
program output. Give the chunk a label and caption when the rendered diagram
should appear as a figure in the document.

#calepin.chunk(label: "fig-mermaid", fig-caption: [Mermaid flowchart])[
```mermaid
%%{init: {"htmlLabels": false}}%%
flowchart LR
  Source["Typst document"] --> Query["Calepin query"]
  Query --> Execute["Run chunks"]
  Execute --> Render["Typst render"]
```
]

= Graphviz DOT

Graphviz DOT is a graph description language used by Graphviz; learn more at #link("https://graphviz.org/")[graphviz.org].

The DOT engine follows the same figure pattern as Mermaid. Keeping the graph source in the Typst file makes the diagram editable alongside the prose that introduces it.

#calepin.chunk(label: "fig-dot", fig-caption: [Graphviz state graph])[
```dot
digraph {
  rankdir=LR
  Draft -> Review -> Publish
  Review -> Draft [label="revise"]
}
```
]

= TikZ

TikZ is a LaTeX package for creating vector graphics; learn more at #link("https://tikz.dev/")[tikz.dev].

TikZ chunks are another way to keep graphics source-controlled with the document. Use this pattern when a diagram is best authored in LaTeX-style drawing commands but should still be rendered through Calepin.

#calepin.chunk(label: "fig-tikz", fig-caption: [TikZ path])[
```tikz
\begin{tikzpicture}
  \draw[thick, blue] (0,0) -- (2,1) -- (4,0);
  \fill[red] (2,1) circle (2pt);
\end{tikzpicture}
```
]

= D2

D2 is a text-to-diagram language; learn more at #link("https://d2lang.com/")[d2lang.com].

D2 chunks demonstrate the same external-renderer workflow with a different
diagram syntax. The important shape is unchanged: source in the chunk, rendered
figure in the document.

#calepin.chunk(label: "fig-d2", fig-caption: [D2 service sketch])[
```d2
direction: right

client -> api: request
api -> worker: job
worker -> database: write
```
]

= Math

Currently, math is only supported by Typst in PDF output.

Use regular Typst math when the expression should be typeset by the document
renderer instead of executed by a language engine. These examples show the math
syntax living next to executable chunks in the same notebook.

#let x = 5

$ A = pi r^2 $

$ "area" = pi dot "radius"^4 $

$ cal(A) := { x in RR | x "is natural" } $

$ #x < 17 $

Warning: On 2026-06-06, math export in HTML was only supported in the development version of Typst, available from Github.
