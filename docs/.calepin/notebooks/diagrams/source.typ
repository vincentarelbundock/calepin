#import "/.calepin/calepin.typ" as calepin

#set document(title: [Diagrams])
#metadata((title: "Diagrams")) <website-metadata>

#calepin.setup(
  echo: true,
  eval: true,
  results: "render",
)

#title()

_Calepin_ can run text-to-diagram engines from the same chunk system used for Python, R, and other computational code. Diagram chunks keep the source for figures in the Typst document, so prose, references, and diagram definitions stay together.

Diagram engines are stateless external tools. They convert source text to SVG, and _Calepin_ renders the SVG as a figure. Give a diagram chunk a `label` and `fig-caption` when it should be numbered and cross-referenced.

= Mermaid

Mermaid is a text-based diagramming tool; learn more at #link("https://mermaid.js.org/")[mermaid.js.org].

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

#calepin.chunk(label: "fig-d2", fig-caption: [D2 service sketch])[
```d2
direction: right

client -> api: request
api -> worker: job
worker -> database: write
```
]
