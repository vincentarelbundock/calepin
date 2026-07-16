#import "/.calepin/calepin.typ": *



#let _raw-chunk-langs = ("python", "r", "mermaid", "dot", "tikz", "d2", "powershell", "sh")
#show raw.where(block: true, lang: "typ", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "typst", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "python", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("python", it) }
#show raw.where(block: true, lang: "r", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("r", it) }
#show raw.where(block: true, lang: "mermaid", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("mermaid", it) }
#show raw.where(block: true, lang: "dot", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("dot", it) }
#show raw.where(block: true, lang: "tikz", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("tikz", it) }
#show raw.where(block: true, lang: "d2", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("d2", it) }
#show raw.where(block: true, lang: "powershell", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("powershell", it) }
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

#show heading: it => {
  if _is-html() and "label" in it.fields() {
    std.html.elem("calepin-heading-anchor", attrs: (data-id: str(it.label)))
  }
  it
}

// Notebook theme
#import "/.calepin/calepin.typ": _html-themed-raw-block, chunk_from_raw_plain

// Body text size, captured below at document-body level. Code blocks are sized
// relative to this rather than to `1em`, which would compound: a literal
// ```typ block is rendered by replacing its source `raw` element, so it renders
// inside Typst's already-reduced raw text context, whereas executed chunks are
// emitted as ordinary calls at body size. Anchoring to the captured body size
// gives both paths a single, matching reduction instead of shrinking twice.
#let _calepin-body-size = std.state("calepin-body-size", 11pt)

#show raw.where(block: true): it => {
  if it.theme != auto {
    context {
      set text(size: _calepin-body-size.get() * 0.8)
      it
    }
  } else if it.lang != none and _raw-chunk-langs.contains(it.lang) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}

#context _calepin-body-size.update(text.size)

#import "/.calepin/calepin.typ" as calepin
#import "/.calepin/calepin.typ" as calepin_runtime
#set document(title: [Compile, watch, serve])
#metadata((tags: ("getting started", "CLI"))) <website-metadata>

#title() <compile-watch-serve>

= Compile
<compile>

Use `calepin compile` when you want to execute code chunks and render a
document. The command line shape is intentionally the same as
`typst compile`: same output-first/format-driven arguments, with `--`
pass-through for Typst flags, plus Calepin preprocessing.

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin compile paper.typ --format pdf\ncalepin compile paper.typ --format html\ncalepin compile paper.typ {p}.svg --format svg\n\n# explicit output path\ncalepin compile paper.typ path/to/paper.pdf --format pdf\n", block: true, lang: "sh"))

Compile a website by pointing `calepin compile` at a source directory
that contains `calepin.toml`:

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin compile docs docs\ncalepin compile my_site public\n", block: true, lang: "sh"))

Arguments after `--` are forwarded to Typst, so project-specific Typst
flags can stay in the same command.

#calepin_runtime.chunk_from_raw_plain("sh", raw("# open PDF in system viewer\ncalepin compile paper.typ -- --open\n\n# set path to font directory\ncalepin compile paper.typ -- --font-path fonts\n", block: true, lang: "sh"))

== Progress output
<progress-output>

Calepin shows animated progress when the terminal supports it, using the
same terminal detection as the underlying progress renderer. When output
is redirected, the terminal is not interactive, or the terminal reports
limited capabilities, Calepin falls back to plain status lines.

Some terminal panes and command runners report themselves as interactive
but render spinner redraws as separate lines. Set `CALEPIN_PROGRESS` to
`plain` to disable animated progress while keeping status messages:

#calepin_runtime.chunk_from_raw_plain("sh", raw("CALEPIN_PROGRESS=plain calepin compile paper.typ\n", block: true, lang: "sh"))

In PowerShell:

```powershell
$env:CALEPIN_PROGRESS = "plain"
calepin compile paper.typ
```

= Watch
<watch>

Use `calepin watch` while editing. It watches your source for changes,
re-runs preprocessing, and delegates recompilation and previewing to
`typst watch`. The command form is the same as `typst watch`: same
positional arguments and pass-through flags, with Calepin running its
preprocessing step first.

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin watch paper.typ\ncalepin watch paper.typ --format html\ncalepin watch paper.typ {p}.svg --format svg\n", block: true, lang: "sh"))

The default output format is PDF. Choose another format with `--format`,
or let the output extension select the format.

Arguments after `--` are passed through to `typst watch`. Typst's
`--open` flag opens the rendered output in the operating system's
default viewer, and Typst's `--port` flag chooses the HTML preview port.

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin watch example.typ -- --open\ncalepin watch paper.typ paper.html --format html -- --port 3001 --open\n", block: true, lang: "sh"))

Watch a website directory by passing the source and output directories:

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin watch docs docs\ncalepin watch my_site public\n", block: true, lang: "sh"))

Add `--serve` to run the local server while watching a website. It uses
the same `--host`, `--port`, and `--open` options as `calepin serve`.

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin watch docs docs --serve --open\ncalepin watch my_site public --serve --host 127.0.0.1 --port 8001\n", block: true, lang: "sh"))

== PDF viewer auto-refresh
<pdf-viewer-auto-refresh>

Some PDF viewers do not automatically refresh a document when it is
regenerated on disk. For example, macOS Preview may keep showing an
older PDF until the window is focused, the file is reopened, or the
application is restarted.

For smoother live preview, use a PDF viewer that reloads the file when
it changes. On macOS, Skim is a good option. Other platforms have
similar auto-reloading viewers, which are useful when working with tools
that repeatedly rebuild PDFs.

= Serve
<serve>

Use `calepin serve` to preview a compiled website directory locally.
This is useful for checking routing, assets, Pagefind search, and links
after a static build.

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin serve docs\ncalepin serve public\n", block: true, lang: "sh"))

By default, Calepin uses `127.0.0.1` and the first available port from
8000. Set the host and port explicitly when you need a stable preview
URL:

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin serve docs --host 127.0.0.1 --port 8001\n", block: true, lang: "sh"))

Open the served website in your default browser:

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin serve docs --open\n", block: true, lang: "sh"))
