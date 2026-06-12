#import "@preview/calepin:0.0.1" as calepin

#set document(title: [Options])

#calepin.setup(eval: true, echo: true)

#title()

Set defaults with `#calepin.setup` and override per call with `#calepin.chunk(...)` or `#calepin.inline(...)`.

= Global + chunk options

These options can be configured in `#calepin.setup` as document-wide defaults. Use one setup block to define defaults for all chunks unless overridden per chunk.

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

= Chunk-only options

These options are only understood on individual chunks; they are not valid as keys in `#calepin.setup`. The figure caption and layout options live here because they apply to one chunk's specific output.

#table(
  columns: (1.2fr, 0.9fr, 2.0fr),
  stroke: none,
  inset: 0.55em,
  [*Option*], [*Default*], [*Meaning*],
  [engine], [inferred], [Force the engine/language for this chunk instead of inferring it from the fence or surrounding context.],
  [body], [from fence], [Provide the raw code body directly instead of writing a fenced block.],
  [label], [auto], [Assign a stable chunk identifier used for cross-references and result lookup.],
  [fig-caption], [`none`], [Caption text for the figure. When set, the output is wrapped in a numbered `figure` that can be cross-referenced.],
  [fig-cap-location], [`"auto"`], [Where the caption sits relative to the figure: `top`, `bottom`, or `margin`. `auto` uses Typst's default placement.],
  [fig-alt-text], [`none`], [Accessibility (alt) text for generated images. Empty when unset.],
  [fig-subcaptions], [`none`], [Per-panel captions for a multi-image chunk, given as an array of strings (one per image, in order).],
  [fig-layout-columns], [`"auto"`], [Column layout for a multi-image chunk: an integer number of equal columns, an array of explicit track sizes, or `auto` to choose a count from the number of images.],
  [fig-layout-rows], [`"auto"`], [Row layout for a multi-image chunk: an integer number of equal rows, an array of explicit track sizes, or `auto`.],
)

= Quarto-style options

Quarto-style chunk headers are supported by placing option lines that begin with \#| at the top of a code fence. These lines are parsed as the same options as \#calepin.chunk options. For example,

````typ
```r
#| fig-align: left
plot(mpg ~ hp, data = mtcars)
```
````
```r
#| fig-align: left
#| echo: false
plot(mpg ~ hp, data = mtcars)
```
````typ
```r
#| fig-align: right
plot(mpg ~ hp, data = mtcars)
```
````
```r
#| fig-align: right
#| echo: false
plot(mpg ~ hp, data = mtcars)
```

The main disadvantage of Quarto-style headers is how they behave outside _Calepin_. If you compile the `.typ` file directly with `typst`, the `#|` lines are displayed as configuration artifacts inside the code block. When options are specified as arguments to `#calepin.chunk(...)`, direct `typst` compilation shows the nice unevaluated code chunk instead.

Note that chunk options have different names in _Calepin_ and _Quarto_. Some aliases are supported, but using them is not recommended and _Calepin_ will emit a warning when it encounters an unsupported option name.

- out-width maps to fig-width
- out-height maps to fig-height
- out-align maps to fig-align
- fig-alt maps to fig-alt-text
- fig-subcap maps to fig-subcaptions
- fig-format maps to fig-device-format
- fig-dpi maps to fig-device-dpi
- layout-ncol maps to fig-layout-columns
- layout-nrow maps to fig-layout-rows
