== Options

Set defaults with `#calepin.setup` and override per call with
`#calepin.chunk(...)` or `#calepin.inline(...)`.

== Global + chunk options

These options can be configured in `#calepin.setup` as document-wide defaults.
Use one setup block to define defaults for all chunks unless overridden per chunk.

#table(
  columns: (1.1fr, 0.9fr, 2.2fr),
  stroke: none,
  inset: 0.55em,
  [*Option*], [*Default*], [*Meaning*],
  [echo], [`true`], [Show the chunk's source code in the rendered document.],
  [eval], [`true`], [Execute the code. When `false`, nothing runs and no output is produced (the source can still be shown via `echo`).],
  [error], [`false`], [When `true`, capture an execution error and render it as output. When `false`, an error in the chunk aborts the build.],
  [warning], [`true`], [Include warnings emitted by the engine in the output. When `false`, they are suppressed.],
  [message], [`true`], [Include informational messages emitted by the engine (for example R's `message()` output). When `false`, they are suppressed.],
  [results], [`"render"`], [How results are shown: `render` (pretty display of values, images, and tables), `verbatim` (raw output in a code block), `typst` (treat output text as Typst markup and render it), or `hide` (run the code but omit its output).],
  [item], [`"all"`], [Which result(s) to display: `all`, `first`, `last`, or a 0-based integer index. Negative indices count from the end, so `-1` is the last result.],
  [fig-device-format], [`"svg"`], [Format for figure files written by the engine: `svg`, `png`, `jpeg` (alias `jpg`), or `pdf`. Diagram engines always emit `svg` regardless of this setting.],
  [fig-device-dpi], [`150`], [Resolution in dots per inch for raster formats (`png`, `jpeg`). Ignored for vector formats (`svg`, `pdf`).],
  [fig-device-width], [`6`], [Width of the plotting device, in inches.],
  [fig-device-height], [`"auto"`], [Height of the plotting device, in inches. `auto` derives it from the width and `fig-device-aspect`.],
  [fig-device-aspect], [`0.618`], [Height-to-width ratio used when `fig-device-height` is `auto`: device height = `fig-device-width` × `fig-device-aspect`.],
  [fig-display-width], [`"70%"`], [Width of the figure as rendered in the document. Accepts a Typst length or ratio (for example `70%` or `12cm`) or `auto`.],
  [fig-display-height], [`"auto"`], [Height of the figure as rendered in the document. Accepts a Typst length or `auto`.],
  [fig-display-align], [`"center"`], [Horizontal alignment of the figure in the document: `left`, `center`, or `right`.],
  [fig-display-responsive], [`true`], [HTML output only: allow the figure to shrink to fit narrow viewports (sets `max-width: 100%`). No effect on paged output.],
)

== Chunk-only options

These options are only understood on individual chunks; they are not valid as keys
in `#calepin.setup`. The figure caption and layout options live here because they
apply to one chunk's specific output.

#table(
  columns: (1.2fr, 0.9fr, 2.0fr),
  stroke: none,
  inset: 0.55em,
  [*Option*], [*Default*], [*Meaning*],
  [engine], [inferred], [Force the engine/language for this chunk instead of inferring it from the fence or surrounding context.],
  [body], [from fence], [Provide the raw code body directly instead of writing a fenced block.],
  [label], [auto], [Assign a stable chunk identifier used for cross-references and result lookup.],
  [fig-caption], [`none`], [Caption text for the figure. When set, the output is wrapped in a numbered `figure` that can be cross-referenced.],
  [fig-caption-position], [`"auto"`], [Where the caption sits relative to the figure: `top` or `bottom`. `auto` uses Typst's default placement.],
  [fig-alt-text], [`none`], [Accessibility (alt) text for generated images. Empty when unset.],
  [fig-subcaptions], [`none`], [Per-panel captions for a multi-image chunk, given as an array of strings (one per image, in order).],
  [fig-layout-columns], [`"auto"`], [Column layout for a multi-image chunk: an integer number of equal columns, an array of explicit track sizes, or `auto` to choose a count from the number of images.],
  [fig-layout-rows], [`"auto"`], [Row layout for a multi-image chunk: an integer number of equal rows, an array of explicit track sizes, or `auto`.],
)

== Examples

Set defaults once near the top of your document:

```typst
#import ".calepin/calepin.typ": calepin

#calepin.setup(
  echo: true,
  results: "render",
  fig-display-width: "75%",
)
```

Then override per chunk:

```typst
#calepin.chunk(
  "python",
  label: "fit-summary",
  echo: false,
  results: "typst",
)[
  import numpy as np
  a = np.array([1, 2, 3])
  sum(a)
]
```
