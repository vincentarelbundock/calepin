#set document(title: [Compile, watch, serve])
#metadata((tags: ("getting started", "CLI"))) <website-metadata>

#title() <compile-watch-serve>

= Compile
<compile>

Use `calepin compile` when you want to execute code chunks and render a
document. The command line shape is intentionally the same as
`typst compile`: same output-first/format-driven arguments, with `--`
pass-through for Typst flags, plus Calepin preprocessing.

```sh
calepin compile paper.typ --format pdf
calepin compile paper.typ --format html
calepin compile paper.typ {p}.svg --format svg

# explicit output path
calepin compile paper.typ path/to/paper.pdf --format pdf
```

Compile a website by pointing `calepin compile` at a source directory
that contains `calepin.toml`:

```sh
calepin compile docs docs
calepin compile my_site public
```

Arguments after `--` are forwarded to Typst, so project-specific Typst
flags can stay in the same command.

```sh
# open PDF in system viewer
calepin compile paper.typ -- --open

# set path to font directory
calepin compile paper.typ -- --font-path fonts
```

== Progress output
<progress-output>

Calepin shows animated progress when the terminal supports it, using the
same terminal detection as the underlying progress renderer. When output
is redirected, the terminal is not interactive, or the terminal reports
limited capabilities, Calepin falls back to plain status lines.

Some terminal panes and command runners report themselves as interactive
but render spinner redraws as separate lines. Set `CALEPIN_PROGRESS` to
`plain` to disable animated progress while keeping status messages:

```sh
CALEPIN_PROGRESS=plain calepin compile paper.typ
```

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

```sh
calepin watch paper.typ
calepin watch paper.typ --format html
calepin watch paper.typ {p}.svg --format svg
```

The default output format is PDF. Choose another format with `--format`,
or let the output extension select the format.

Arguments after `--` are passed through to `typst watch`. Typst's
`--open` flag opens the rendered output in the operating system's
default viewer, and Typst's `--port` flag chooses the HTML preview port.

```sh
calepin watch example.typ -- --open
calepin watch paper.typ paper.html --format html -- --port 3001 --open
```

Watch a website directory by passing the source and output directories:

```sh
calepin watch docs docs
calepin watch my_site public
```

Add `--serve` to run the local server while watching a website. It uses
the same `--host`, `--port`, and `--open` options as `calepin serve`.

```sh
calepin watch docs docs --serve --open
calepin watch my_site public --serve --host 127.0.0.1 --port 8001
```

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

```sh
calepin serve docs
calepin serve public
```

By default, Calepin uses `127.0.0.1` and the first available port from
8000. Set the host and port explicitly when you need a stable preview
URL:

```sh
calepin serve docs --host 127.0.0.1 --port 8001
```

Open the served website in your default browser:

```sh
calepin serve docs --open
```
