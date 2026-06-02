# Calepin Typst Preprocessor Rewrite Design

## Summary

Calepin will be rewritten around one goal: preprocess Typst documents by executing embedded code chunks and writing result artifacts that Typst can consume. Calepin will no longer render documents, convert markup, assemble pages, manage websites, resolve citations, or act as a Quarto substitute.

The new workflow is a two-pass integration with Typst:

1. Calepin runs `typst query` on the input `.typ` file to discover code chunk metadata emitted by a small Calepin Typst runtime.
2. Calepin executes the discovered R, Python, and shell chunks, using a cache where appropriate.
3. Calepin writes a results JSON file plus figure artifacts under `.calepin/`.
4. Calepin may optionally invoke `typst compile`, passing the results path through `--input`.
5. The Calepin Typst runtime reads the results file and substitutes source, output, figures, tables, warnings, messages, and errors into the document.

Typst remains the renderer. Calepin only prepares execution results.

## Goals

- Support `.typ` documents only.
- Execute code chunks in R, Python, and POSIX shell.
- Preserve notebook-like execution state within each engine for one document pass.
- Support chunk options for `cache`, `echo`, `eval`, `include`, `results`, `warning`, `message`, and `error`.
- Support common figure options such as `fig-width`, `fig-height`, `dev`, `dpi`, `out-width`, `out-height`, `fig-cap`, and `fig-alt`.
- Support table substitution through Typst table figures, especially for chunks that emit Typst markup.
- Keep cache behavior from current Calepin where it remains useful: content-addressed chunk caching with downstream invalidation through an upstream digest chain.
- Provide an optional Typst compile step, while keeping preprocessing as the core operation.
- Aggressively remove legacy systems that belong to rendering, Quarto compatibility, websites, extensions, templates, bibliography, cross-references, and Markdown parsing.

## Non-goals

- No `.qmd`, Markdown, Quarto, or Pandoc parsing.
- No HTML, LaTeX, Markdown, website, slides, or book rendering.
- No template system.
- No Calepin-side syntax highlighting. Typst raw elements handle displayed source.
- No citation processing or cross-reference resolution. Typst handles document semantics.
- No page assembly, target configuration, sidecar templates, extension inheritance, or module registry.
- No source-range rewriting of `.typ` files.
- No execution from inside Typst. System commands are only run by the Calepin CLI before Typst compilation.
- No unlabeled chunks in v1. Labels keep result lookup deterministic without source positions.
- No mandatory Jupyter kernel backend in v1. Jupyter and Ark are future optional execution providers, not core dependencies.

## User workflow

A document imports the Calepin Typst runtime and uses `calepin.chunk` calls:

```typ
#import ".calepin/calepin.typ"

#calepin.chunk(
  engine: "r",
  label: "fig-scatter",
  echo: false,
  fig-cap: [Scatterplot],
)[`
x <- 1:10
y <- x + rnorm(10)
plot(x, y)
`]
```

The chunk body must be a single bare Typst raw element. Triple-backtick raw blocks are allowed but not required; the preferred inline spelling uses one backtick delimiter:

```typ
#calepin.chunk(engine: "python", label: "setup")[`
x = 1
`]
```

This is invalid:

````typ
#calepin.chunk(engine: "python", label: "setup")[
```python
x = 1
```
]
````

The engine is declared only by the `engine:` argument. Calepin rejects chunk bodies whose raw element declares a language.

The basic CLI flow:

```sh
calepin preprocess paper.typ
typst compile paper.typ paper.pdf --input calepin-mode=render --input calepin-results=.calepin/paper/results.json
```

The convenience flow:

```sh
calepin compile paper.typ paper.pdf
```

`calepin compile` runs preprocessing first, then shells out to `typst compile` with the correct `--input` values.

## Typst runtime API

Calepin ships a small Typst runtime file, written to `.calepin/calepin.typ` before running `typst query`. Users import it from their document:

```typ
#import ".calepin/calepin.typ"
```

The v1 API exposes two functions:

```typ
#calepin.setup(
  cache: true,
  echo: true,
  eval: true,
  include_: true,
  results: "verbatim",
  warning: true,
  message: true,
  error: false,
  format: auto,
  item: "all",
  placeholder: auto,
  dev: "svg",
  dpi: 150,
  fig-width: 6,
  fig-height: auto,
)
```

`setup` emits configuration metadata during query mode and otherwise returns no visible content. Chunk-level options override setup defaults.

```typ
#calepin.chunk(
  body,
  engine: "r",
  label: none,
  cache: auto,
  echo: auto,
  eval: auto,
  include_: auto,
  results: auto,
  warning: auto,
  message: auto,
  error: auto,
  format: auto,
  item: auto,
  placeholder: auto,
  dev: auto,
  dpi: auto,
  fig-width: auto,
  fig-height: auto,
  out-width: auto,
  out-height: auto,
  fig-cap: none,
  fig-alt: none,
  tbl-cap: none,
  kind: auto,
)
```

`label` is required by Calepin even though the Typst function accepts `none` to allow the runtime to emit a useful error. Labels must be unique within the document.

Typst reserves `include`, so the public Typst runtime argument is spelled `include_`. Calepin still emits, parses, and stores the normalized option under the metadata key `"include"`.

During query mode, `chunk` emits invisible metadata labeled `<calepin-chunk>`. During render mode, it reads the results JSON and returns visible Typst content.

## Chunk body rules

After ignoring surrounding whitespace, each chunk body must contain exactly one bare Typst raw element. This wrapper is still necessary because Typst parses function bodies before the Calepin runtime receives them; a bare code line such as `x <- 1` is parsed as Typst markup and fails before Calepin can extract it.

Accepted:

```typ
#calepin.chunk(engine: "r", label: "setup")[`
x <- 1
`]
```

Rejected:

- No raw element.
- More than one raw element.
- A raw block with a language, such as ```` ```r ````.
- Extra non-whitespace Typst markup before or after the raw element.
- A missing label.
- A duplicate label.
- An unsupported engine.

The runtime and CLI both validate where practical. The CLI is the authority because it sees serialized Typst metadata from `typst query`.

## Data flow

### Query pass

Calepin invokes Typst roughly as follows:

```sh
typst query paper.typ '<calepin-chunk>' \
  --root <project-root> \
  --input calepin-mode=query \
  --input calepin-results=.calepin/paper/results.json
```

It also queries `<calepin-config>` for optional setup metadata.

Typst returns JSON metadata in document order. Calepin parses each metadata value into a `ChunkSpec`:

```text
ChunkSpec
  label: String
  engine: Engine
  code: String
  exec_options: ExecOptions
  display_options: DisplayOptions
  ordinal: usize
```

Calepin relies on labels for render-time lookup and uses query order only for execution order.

### Execution pass

Chunks execute sequentially in document order. This preserves notebook semantics:

- R chunks share one persistent `Rscript` session.
- Python chunks share one persistent `python3` session.
- Shell chunks share one persistent `/bin/sh` session.
- State is shared within an engine, not across engines.

Execution working directory defaults to the input file directory. A CLI flag can override it.

### Results pass

Calepin writes results to:

```text
.calepin/<input-stem>/results.json
.calepin/<input-stem>/figures/<label>.<ext>
.calepin/<input-stem>/cache/
```

For nested input files, `<input-stem>` is the input path relative to the project root with separators normalized into directories. For example:

```text
chapters/intro.typ
.calepin/chapters/intro/results.json
.calepin/chapters/intro/figures/fig-main.svg
```

The results JSON is the only contract between Calepin and Typst render mode.

## Results JSON schema

The results file is a map keyed by chunk label. It should use a MIME-aware shape inspired by Jupyter notebook outputs and Callisto's Typst-side output handling, without requiring Calepin to produce a full notebook.

```json
{
  "schema": 1,
  "calepin_version": "0.1.0",
  "input": "paper.typ",
  "chunks": {
    "fig-scatter": {
      "label": "fig-scatter",
      "engine": "r",
      "status": "ok",
      "cached": false,
      "items": [
        {
          "type": "display",
          "data": {
            "image/svg+xml": {
              "path": ".calepin/paper/figures/fig-scatter.svg"
            },
            "text/plain": "R plot"
          },
          "metadata": {}
        }
      ]
    }
  }
}
```

Supported result item types:

- `stream`: stdout or stderr text, with fields `name` and `text`.
- `diagnostic`: warning or message text, with fields `level` and `text`.
- `error`: execution error, with fields `name`, `message`, and optional `traceback`.
- `display`: rich display output with `data` and `metadata` dictionaries.
- `result`: rich expression result output with `data` and `metadata` dictionaries.

Rich output `data` is keyed by MIME type. Supported v1 MIME types:

- `image/svg+xml`: SVG figure artifact.
- `image/png`: PNG figure artifact.
- `text/x-typst`: Typst markup to evaluate with `eval(..., mode: "markup")`.
- `text/plain`: plain text.
- `application/json`: structured data for future user handlers.

For large or binary values, the MIME value should be an artifact reference:

```json
{ "path": "/.calepin/paper/figures/fig-scatter.svg" }
```

For small text values, the MIME value may be a string.

Displayed source is not stored in results JSON. The Typst runtime already has the original raw element text and can display it when `echo` is enabled.

Captions are not stored in results JSON. The Typst runtime already has `fig-cap` and `tbl-cap` arguments during render mode.

Calepin should not write a `.ipynb` file as the primary result contract. A future command may export an executed notebook, but the v1 render contract is this smaller results JSON file.

## Rendering behavior in the Typst runtime

In render mode, `calepin.chunk` reads the results file path from `sys.inputs.at("calepin-results")`.

For a chunk with `include: false`, it returns no visible content after preprocessing has executed the chunk.

For a chunk with `echo: true`, it emits:

```typ
#raw(code, block: true, lang: engine)
```

For `stream`, `diagnostic`, and `error` items, it emits raw text blocks by default. Diagnostics can use simple Typst block styling in the runtime, but this styling is intentionally minimal.

For rich `display` and `result` items, it selects the first available MIME type from a format preference list. The default preference order is:

```typ
("image/svg+xml", "image/png", "text/x-typst", "text/plain", "application/json")
```

This mirrors Callisto's useful separation between rich output selection and MIME-specific handling, while keeping Calepin's runtime much smaller.

For `text/x-typst`, the selected MIME value is a string and the runtime emits:

```typ
#eval(value, mode: "markup")
```

For `image/svg+xml` and `image/png`, the selected MIME value is a Typst root-relative artifact reference. With `fig-cap`, the runtime emits:

```typ
#figure(
  image(value.path, width: out-width, height: out-height),
  caption: fig-cap,
) #label(label)
```

For image items without `fig-cap`, it emits the image directly and applies the label if Typst permits that in the selected structure.

For table-like Typst output, `kind: "table"` or a `tbl-cap` option makes the runtime emit:

```typ
#figure(
  kind: table,
  caption: tbl-cap,
)[
  #eval(value, mode: "markup")
] #label(label)
```

`kind: auto` infers table behavior when `tbl-cap` is set or the label starts with `tbl-`.

The Typst runtime should have a small internal handler table for these MIME types. This is not a template system and should not grow into one. User-defined handlers can remain future work unless a concrete v1 use case requires them.

If the results file is missing and `placeholder` is enabled, the runtime should display the source when `echo` is enabled and a minimal placeholder for missing output. If a results file is provided but a label is missing, the runtime should error because that indicates stale or inconsistent preprocessing.

## Option semantics

### Engine

`engine` is required and must be one of:

- `r`
- `python`
- `sh`
- `bash`, accepted as an alias for `sh`

### Label

`label` is required and must be unique. It is the stable key into the results JSON and the Typst label applied to figures and tables.

### eval

Default: `true`.

When `eval: false`, Calepin does not execute the chunk. The chunk still participates in the upstream cache digest. If `echo: true`, the runtime still displays the source code.

### echo

Default: `true`.

When `echo: true`, the Typst runtime displays the source code from the raw element with `#raw(..., lang: engine)`. Changing `echo` does not invalidate the execution cache.

### cache

Default: `true`.

When `cache: true`, Calepin can reuse prior chunk results if the execution-affecting cache key matches. When `cache: false`, Calepin executes the chunk and does not read or write a chunk cache entry.

### include

Default: `true`.

When `include: false`, Calepin executes the chunk but the Typst runtime emits no visible content for it. This is useful for setup chunks.

### results

Default: `"verbatim"`.

Allowed values:

- `"verbatim"`: display stdout as a raw text block.
- `"asis"`: treat stdout or engine-provided as-is output as Typst markup and evaluate it with `eval(..., mode: "markup")`.
- `"hide"`: suppress stdout while still allowing figures and diagnostics unless their own display flags suppress them.

### warning and message

Defaults: `true`.

These options control whether warning and message items are shown by the Typst runtime. They do not affect execution or cache keys.

### error

Default: `false`.

When `error: false`, an execution error stops preprocessing and Calepin exits nonzero.

When `error: true`, Calepin records an `error` result item, continues execution, and the Typst runtime displays the error unless `include: false`.

### format

Default: `auto`.

This controls the rich output MIME preference list used by the Typst runtime for `display` and `result` items. `auto` expands to the default order:

```typ
("image/svg+xml", "image/png", "text/x-typst", "text/plain", "application/json")
```

Changing `format` does not invalidate execution caches.

### item

Default: `"all"`.

This controls which output items are rendered when a chunk produces more than one item. Allowed values:

- `"all"`: render all matching output items in order.
- `"first"`: render the first matching output item.
- `"last"`: render the last matching output item.
- an integer: render the item at that zero-based index, with negative indices counting from the end.

Changing `item` does not invalidate execution caches.

### placeholder

Default: `auto`.

When enabled, direct Typst compilation without a results file can show source and a small missing-output placeholder instead of failing. When a results file is provided but the current label is absent, the runtime should still error.

Changing `placeholder` does not invalidate execution caches.

### Figure execution options

Defaults:

- `dev: "svg"`
- `dpi: 150`
- `fig-width: 6`
- `fig-height: auto`

R and Python figure capture reuse current Calepin engine behavior with Typst-focused defaults. SVG is the default because Typst can consume SVG and vector output is preferable for most documents. Users can request `png` when raster output is needed.

`fig-width` and `fig-height` affect the generated artifact and are execution-affecting cache keys.

`out-width`, `out-height`, `fig-cap`, `fig-alt`, `tbl-cap`, `kind`, `format`, `item`, and `placeholder` are display-only and do not invalidate execution caches.

## Cache design

Reuse the current digest-chain cache with simplification.

Each chunk cache key includes:

- Calepin cache schema version.
- Engine name.
- Chunk source code.
- Execution-affecting options.
- Figure artifact format and dimensions.
- The upstream digest from all prior chunks.

Display-only options are excluded:

- `echo`
- `include`
- `results`
- `warning`
- `message`
- `error`
- `out-width`
- `out-height`
- `fig-cap`
- `fig-alt`
- `tbl-cap`
- `kind`
- `format`
- `item`
- `placeholder`

Changing chunk 3 invalidates chunk 3 and all downstream chunks because the upstream digest changes. Chunks 1 and 2 remain cacheable.

Cache entries store:

- Serialized result items.
- Generated figure artifacts.
- Cache metadata with the full hash and schema version.

Cache reads restore figure files into the current figures directory before writing results JSON.

## Engine backend architecture

The preprocessing pipeline should depend on an engine interface, not on any specific capture protocol:

```text
Engine
  start(config) -> session
  execute(chunk, artifact_paths) -> Vec<ResultItem>
  shutdown()
```

All engines return normalized `ResultItem` values. The rest of Calepin should not know whether those items came from sentinel-delimited subprocess stdout, Jupyter messages, Ark, or another backend.

V1 default backends:

- `RscriptEngine`: direct persistent `Rscript` subprocess.
- `PythonEngine`: direct persistent `python3` subprocess.
- `ShellEngine`: direct persistent `/bin/sh` subprocess.

The direct subprocess engines are the v1 default because they have a small dependency surface, are already mostly implemented, and keep the rewrite focused on Typst preprocessing.

The engine interface must still be designed so it can support structured backends later. In particular, `ResultItem` should be able to represent:

- `stream` output for stdout and stderr.
- `diagnostic` output for warnings and messages.
- `error` output with message and optional traceback.
- `display` output with a MIME bundle.
- `result` output with a MIME bundle.

Future optional backend:

- `JupyterKernelEngine`: launches or connects to a Jupyter kernel and normalizes `stream`, `display_data`, `execute_result`, `error`, and status messages into `ResultItem` values.
- `ArkEngine`: an R-specific provider built on `JupyterKernelEngine`, using Posit's Ark kernel where installed.

Jupyter may be more robust for structured output capture, especially for R through Ark, but it brings more operational complexity: kernelspec discovery, connection files, ZeroMQ transport, message signing, IOPub draining, timeout handling, and kernel shutdown. Those concerns should stay outside v1.

## Engine reuse

The following current Calepin pieces should be retained and simplified:

- `engines/r.rs`: persistent R session, stdout capture, warnings, messages, and plot capture.
- `engines/python.rs`: persistent Python session, stdout capture, warnings, errors, matplotlib capture.
- `engines/sh.rs`: persistent shell session.
- `engines/subprocess.rs`: sentinel-delimited subprocess protocol and timeout support.
- `engines/cache.rs`: digest-chain cache, rewritten around `ChunkSpec` and `ResultItem`.
- The typed chunk option accessors, moved into a smaller preprocessing model.

The R engine should keep `knitr::knit_print` handling where it produces useful Typst markup. LaTeX and HTML preamble collection should be removed. Typst-specific as-is output is enough.

The Python engine should keep matplotlib capture and expression output capture. It should not attempt to render Python objects into Typst tables except through explicit user code that prints Typst markup with `results: "asis"`.

The shell engine remains text-only.

## Aggressive removals

The rewrite should remove or archive these systems:

- `.qmd` block parser.
- Markdown-to-format conversion.
- `Element` render pipeline.
- MiniJinja template system.
- Built-in templates for HTML, LaTeX, Typst, Markdown, website, slides, and books.
- Extension manifests and extension inheritance.
- Module registry and module traits.
- Website and book collection orchestration.
- Preview server and file watcher.
- Citation processing with Hayagriva.
- Cross-reference resolution.
- Syntax highlighting with Syntect.
- SVG-to-PDF and PDF-to-SVG conversion modules.
- Sidecar initialization and template management.
- `man` documentation extraction commands.
- WASM plugin scaffolding and plugin examples unless they are kept in a separate historical branch.

Dependencies should be reduced accordingly. The new crate should need only CLI parsing, serialization, subprocess management, temporary files, hashing, error handling, and a small set of engine-specific utilities.

## CLI design

The CLI should expose one workflow with two entry points:

```sh
calepin preprocess INPUT.typ [OPTIONS]
calepin compile INPUT.typ [OUTPUT] [OPTIONS] [-- TYPST_ARGS...]
```

`preprocess`:

- Writes `.calepin/calepin.typ` if needed.
- Runs `typst query` to discover chunks.
- Executes chunks.
- Writes results JSON and figure artifacts.
- Does not call `typst compile`.

`compile`:

- Runs `preprocess`.
- Invokes `typst compile`.
- Passes `--input calepin-mode=render`.
- Passes `--input calepin-results=<relative-results-path>`.
- Forwards user-provided Typst arguments after `--`.

Common options:

- `--root DIR`: Typst project root. Defaults to the input file directory.
- `--cwd DIR`: execution working directory. Defaults to the input file directory.
- `--results PATH`: override results JSON path.
- `--cache` and `--no-cache`: override document defaults.
- `--execute` and `--no-execute`: global execution toggle. `--no-execute` is equivalent to setting `eval: false` for all chunks.
- `--clean`: remove generated results and figures before preprocessing.
- `--quiet`: suppress progress messages.
- `--typst PATH`: path to the Typst executable.
- `--rscript PATH`, `--python PATH`, `--shell PATH`: engine executable overrides.
- `--timeout SECONDS`: per-chunk timeout.

Removed commands:

- `render`
- `preview`
- `init`
- `man`
- `extra`
- `templates`

If shell completions are retained, they should be generated by the binary as part of the CLI framework but not exposed as a large separate feature.

## Typst compile integration

`calepin compile paper.typ paper.pdf` is a convenience wrapper. It does not make Calepin a renderer.

Calepin forwards Typst options without interpreting them:

```sh
calepin compile paper.typ paper.pdf -- --font-path fonts --input theme=dark
```

Calepin reserves these Typst input keys:

- `calepin-mode`
- `calepin-results`

User-provided forwarded arguments must not override those keys. If they do, Calepin exits with an error.

## Security model

Calepin executes arbitrary user code. There is no sandbox in v1.

The security posture is:

- Code execution only happens when the user invokes Calepin.
- Plain `typst compile` never runs system commands.
- Shell chunks are explicit through `engine: "sh"` or `engine: "bash"`.
- The CLI prints which engines will run before execution unless `--quiet` is set.
- Timeouts are available and should default to a conservative value.
- The results file is ordinary data that Typst reads during render mode.

## Error handling

Typst query errors:

- Forward Typst diagnostics to stderr.
- Exit nonzero.
- Common causes include missing `.calepin/calepin.typ`, syntax errors, and invalid imports.

Chunk validation errors:

- Report the chunk label when available.
- Exit nonzero.
- Duplicate labels are reported together.
- Missing labels are reported with the chunk ordinal.

Execution errors:

- With `error: false`, stop preprocessing and exit nonzero.
- With `error: true`, store an `error` result item and continue.

Render-mode missing results:

- If no results path is provided and `placeholder` is enabled, the Typst runtime shows source and a minimal placeholder.
- If a results path is provided but a label is missing, the Typst runtime emits an error naming the label and results path.

Cache errors:

- Cache read failures are treated as misses.
- Cache write failures warn and continue.
- Results JSON write failures are fatal.

## Testing strategy

Unit tests:

- Parse Typst query JSON into `ChunkSpec`.
- Validate bare raw element rules.
- Reject missing labels, duplicate labels, unsupported engines, and language-tagged raw blocks.
- Merge setup defaults and chunk overrides.
- Compute stable cache keys.
- Exclude display-only options from cache keys.
- Include execution-affecting options in cache keys.
- Serialize and deserialize results JSON.
- Select rich output formats by MIME preference.
- Apply missing-results placeholder behavior.

Engine tests:

- R stdout, warning, message, error, and plot capture.
- Python stdout, warning, error, and matplotlib capture.
- Shell stdout and error capture.
- Timeout behavior.
- State persistence across chunks of the same engine.

Integration tests:

- `calepin preprocess` writes results JSON for a minimal Typst document.
- `calepin compile` produces a PDF through Typst for a document with a source block and stdout.
- Figure chunk produces an artifact and a renderable Typst figure.
- Table chunk with `results: "asis"` and `tbl-cap` produces a Typst table figure.
- Rich output with both image and text picks the image by default.
- Direct `typst compile` without a results file shows a placeholder when placeholders are enabled.
- Cache hit avoids re-execution and restores figures.
- Changing an upstream chunk invalidates downstream chunks.

Tests requiring R, Python plotting packages, or Typst should be gated so normal unit tests remain fast and reliable.

## Migration notes

This rewrite is not a backward-compatible release. It is a product reset.

Expected migration:

- Old `.qmd` documents are no longer supported.
- Users write native `.typ` documents.
- Code chunks move into `#calepin.chunk(...)` calls.
- Users rely on Typst for layout, cross-references, bibliography, styling, and final output.
- Calepin owns only code execution, caching, generated artifacts, and optional `typst compile` orchestration.

The old Quarto-like Calepin behavior is preserved by the `last-quarto-substitute` tag and should not constrain the rewrite.

## Future work outside v1

- Optional unlabeled chunks using a Typst runtime counter.
- A published Typst Universe package instead of `.calepin/calepin.typ`.
- Watch mode that reruns preprocessing and Typst compilation.
- Optional Jupyter kernel backend for structured execution.
- Optional Ark provider for R through the Jupyter backend.
- More engines.
- A small `calepin doctor` command for checking Typst, R, Python, and shell availability.
- Richer table adapters for common R and Python table libraries.
- A machine-readable dependency file for build systems.
