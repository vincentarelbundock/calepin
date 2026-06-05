---
title: Options
---

## Global options
`calepin.setup()` sets document-wide defaults for every chunk and inline expression that follows. For language-local defaults, add a second `calepin.setup` with `lang:`.

`calepin.chunk` and `calepin.inline` accept the same options and override those defaults per call, where `auto` means inherit the value set in `calepin.setup()`. The `engine` (inferred from the body fence's language when omitted), `body`, and `label` arguments apply to chunks only. For HTML themes, use the CLI `--template` flag.

```typ
#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
  fig-device-format: "svg",
  fig-display-width: 70%,
)
```

## Language-specific options

The next block demonstrates `calepin.setup` with language-local defaults and global fallback.

````typ
#import ".calepin/calepin.typ"

#set document(
  title: [Language-specific setup],
)

#calepin.setup(
  echo: false,
  results: "verbatim",
)

#calepin.setup(lang: "python", echo: true)
#calepin.setup(lang: "r", eval: false)

```python
print("python: echoed + executed")
```

```r
cat("r: not executed")
```

#calepin.chunk("r", eval: true, results: "verbatim")[
```r
cat("r: forced by chunk override")
```
]
````

## Chunk-specific options

The `engine`, `body`, and `label` arguments are specific to `#calepin.chunk`. Use `engine` to force or override inference from the fence language, `label` to give stable references, and `body` when calling chunks programmatically without a raw code block.

```typ
#calepin.chunk(
  engine: "python",
  label: "fit-summary",
  echo: true,
  results: "asis",
)[
```python
print("This chunk is forced to use python and gets a stable label")
```
]
```

## Reference table

| Option | Default | Meaning |
| --- | --- | --- |
| `lang` | `none` | Restrict setup defaults to this language (`"python"`, `"r"`, ...). Global defaults apply when a chunk's language has no language-specific setup entry. |
| `engine` | inferred | Execution engine: `"python"`, `"r"`, `"mermaid"`, `"dot"`, `"tikz"`, `"d2"`, or any Jupyter kernel name (e.g. `"bash"`, `"julia"`, `"octave"`). Omit it to infer the engine from the fenced block's language (`` ```python ``); pass it explicitly to override, or when the body fence has no language. |
| `body` | required | Raw code body. It must contain exactly one raw element. |
| `label` | auto-generated | Unique result label. Required for stable references and figures. |
| `echo` | `true` | Show source code before results. |
| `eval` | `true` | Execute code. |
| `output` | `true` | Include chunk output in the rendered document. |
| `error` | `false` | Capture execution errors as output instead of failing preprocessing. |
| `warning` | `true` | Include warning diagnostics. |
| `message` | `true` | Include message diagnostics. |
| `results` | `"verbatim"` | Render text output as verbatim text. Use `"asis"` for Typst markup or `"hide"` to suppress results. |
| `format` | `auto` | Preferred result formats for display. |
| `item` | `"all"` | Which result item to render: `"all"`, `"first"`, `"last"`, or an index. |
| `placeholder` | `auto` | Reserve a placeholder when no result is available. |
| `kind` | `auto` | Output kind hint, such as table handling for Typst markup results. |
| `fig-device-format` | `"svg"` | Figure artifact format. |
| `fig-device-dpi` | `150` | DPI for raster figure devices. |
| `fig-device-width` | `6` | Figure device width in inches for engines that create graphics. |
| `fig-device-height` | `auto` | Figure device height. |
| `fig-device-aspect` | `0.618` | Aspect ratio used when height is automatic. |
| `fig-display-width` | `70%` | Width used when displaying generated images. |
| `fig-display-height` | `auto` | Height used when displaying generated images. |
| `fig-display-align` | `center` | Alignment for displayed figures. |
| `fig-display-responsive` | `true` | Constrain displayed figures to the available width in HTML. |
| `fig-display-link` | `auto` | Optional link around a displayed figure. |
| `fig-caption` | `none` | Figure caption. |
| `fig-caption-position` | `auto` | Caption position when Typst can express it. |
| `fig-alt-text` | `none` | Alternative text for generated images. |
| `fig-subcaptions` | `none` | Subcaption metadata for multi-output layouts. |
| `fig-layout-columns` | `auto` | Column layout metadata for future multi-output figure layouts. |
| `fig-layout-rows` | `auto` | Row layout metadata for future multi-output figure layouts. |
| `fig-layout-design` | `auto` | Layout design metadata for future multi-output figure layouts. |

