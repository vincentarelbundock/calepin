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

#import "/.calepin/calepin.typ" as calepin

#set document(title: [Diagrams])

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

= Adding a new diagram engine

Diagram support is intentionally small. Each diagram engine is a stateless wrapper around an external command-line tool. _Calepin_ writes the chunk source to a temporary file, asks the tool to produce SVG, and records that SVG as the figure output. Diagram engines do not keep a persistent session like Python, R, Julia, or shell chunks.

The main entry point is `calepin/src/engines/diagram.rs`. Add a `DiagramSpec` with:

- `name`: the fenced-code language users will write, such as `mermaid` or `dot`.
- `input_ext`: the temporary source-file extension passed to the external tool.
- `prepare_source`: usually `identity_source`, unless the tool needs a wrapper like TikZ.
- `render`: a function that runs the external tool and writes `run.fig_path`.

Simple tools can use the `simple_diagram_renderer!` helper in `diagram.rs`. More complex tools should get a small module under `calepin/src/engines/diagram/`, like `mermaid.rs` or `tikz.rs`, so retries, generated config files, or multi-step conversions stay out of the common path.

If the new engine uses a new executable, also register it in the surrounding plumbing:

- `calepin/src/config.rs` for the configurable executable path.
- `calepin/src/utils/tools.rs` for the default command name and install hint.
- `calepin/src/health/mod.rs` so `calepin health` can report whether the tool is available.
- `calepin/src/typst/preprocess/fingerprint.rs` so cached results are invalidated when the tool path changes.

Finally, make the chunk language visible to Typst and source rewriting by updating the built-in engine lists in `calepin/src/assets/typst-runtime/notebook/chunk.typ`, `calepin/src/typst/preprocess/staging.rs`, and any parser or execution tests that enumerate diagram engines. Add a small example to this page and tests for the renderer behavior.

Pull requests for new diagram engines are welcome. Open a pull request with the implementation, tests, and documentation example so the engine can be reviewed against the same notebook behavior as the built-in diagram tools.
