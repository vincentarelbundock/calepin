#import "/.calepin/calepin.typ": *



#let _raw-chunk-langs = ("python", "r", "mermaid", "dot", "tikz", "d2", "text")
#show raw.where(block: true, lang: "typ", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "typst", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "python", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("python", it) }
#show raw.where(block: true, lang: "r", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("r", it) }
#show raw.where(block: true, lang: "mermaid", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("mermaid", it) }
#show raw.where(block: true, lang: "dot", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("dot", it) }
#show raw.where(block: true, lang: "tikz", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("tikz", it) }
#show raw.where(block: true, lang: "d2", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("d2", it) }
#show raw.where(block: true, lang: "text", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("text", it) }

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
#set document(title: [CLI reference])
#metadata((tags: ("reference", "CLI"))) <website-metadata>

#title() <cli-reference>

= `calepin`
<calepin>

```text
Preprocess Typst documents with executable code chunks

Usage: calepin <COMMAND>

Commands:
  new      Create a notebook file, website scaffold, or ejected theme
  health   Check Calepin's local runtime environment and local links
  compile  Preprocess, then invoke typst compile
  watch    Watch, preprocess, and delegate recompiles to typst watch
  serve    Serve static files locally
  update   Update Calepin using the official installer updater
  clean    Remove `.calepin` directories and generated artifacts
  help     Print this message or the help of the given subcommand(s)

Options:
  -v, --version  Print version
  -h, --help     Print help
```

= `calepin new`
<calepin-new>

```text
Create a notebook file, website scaffold, or ejected theme

Usage: calepin new [OPTIONS] <PATH|website|theme> [DIR]

Arguments:
  <PATH|website|theme>  What to create: a .typ notebook path, `website`, or `theme`
  [DIR]                 Destination directory when creating a website scaffold or ejected theme

Options:
      --theme <THEME>  Built-in theme to use when creating a website scaffold or ejected theme [possible values: calepin, academic]
  -f, --force          Overwrite the file if it already exists
  -h, --help           Print help

Examples:
  calepin new paper.typ
  calepin new website
  calepin new website --theme academic
  calepin new theme
  calepin new theme --theme academic
  calepin new theme themes/my-theme
```

= `calepin health`
<calepin-health>

```text
Check Calepin's local runtime environment and local links

Usage: calepin health [OPTIONS]

Options:
      --config <CONFIG>       Path to project config TOML
  -d, --depth <DEPTH>         Maximum recursion depth when searching for links
      --json                  Print machine-readable JSON
      --strict                Exit with an error when warnings are present
      --check-external-links  Also check external links
  -h, --help                  Print help
```

= `calepin compile`
<calepin-compile>

```text
Preprocess, then invoke typst compile

Usage: calepin compile [OPTIONS] <INPUT> [OUTPUT] [TYPST_ARGS]...

Arguments:
  <INPUT>
          Input .typ file, or a website source directory containing calepin.toml

  [OUTPUT]
          Output file path, or website output directory when INPUT is a directory

  [TYPST_ARGS]...
          Arguments forwarded to typst compile after `--`

Options:
      --format <FORMAT>
          Output format passed to typst compile
          
          [possible values: pdf, png, svg, html]

      --minify
          Minify HTML output after theming and asset processing

      --config <CONFIG>
          Path to project config TOML

  -q, --quiet
          Quiet mode

      --timeout <TIMEOUT>
          Per-chunk timeout in seconds

      --set <KEY=VALUE>
          Override a Calepin config value as `key=value` (repeatable).
          
          Uses dotted paths for nested config, such as `theme=./theme`, `vars.region=CA`, or `toc.enabled=false`.

  -h, --help
          Print help (see a summary with '-h')
```

= `calepin watch`
<calepin-watch>

```text
Watch, preprocess, and delegate recompiles to typst watch

Usage: calepin watch [OPTIONS] <INPUT> [OUTPUT] [TYPST_ARGS]...

Arguments:
  <INPUT>
          Input .typ file, or a website source directory containing calepin.toml

  [OUTPUT]
          Output file path, or website output directory when INPUT is a directory

  [TYPST_ARGS]...
          Arguments forwarded to typst watch after `--`

Options:
      --format <FORMAT>
          Output format passed to typst watch
          
          [possible values: pdf, png, svg, html]

      --serve
          Serve the website while watching a directory

      --open
          Open the served website in the default browser

      --host <HOST>
          Interface to bind when serving a watched website
          
          [default: 127.0.0.1]

      --port <PORT>
          Port to bind when serving a watched website (default: first free port from 8000)

      --config <CONFIG>
          Path to project config TOML

  -q, --quiet
          Quiet mode

      --timeout <TIMEOUT>
          Per-chunk timeout in seconds

      --set <KEY=VALUE>
          Override a Calepin config value as `key=value` (repeatable).
          
          Uses dotted paths for nested config, such as `theme=./theme`, `vars.region=CA`, or `toc.enabled=false`.

  -h, --help
          Print help (see a summary with '-h')
```

= `calepin serve`
<calepin-serve>

```text
Serve static files locally

Usage: calepin serve [OPTIONS] <DIR>

Arguments:
  <DIR>  Directory containing static files to serve

Options:
      --host <HOST>  Interface to bind [default: 127.0.0.1]
  -p, --port <PORT>  Port to bind (default: first free port from 8000)
      --open         Open the website in the default browser
  -h, --help         Print help
```

= `calepin update`
<calepin-update>

```text
Update Calepin using the official installer updater

Usage: calepin update

Options:
  -h, --help  Print help
```

= `calepin clean`
<calepin-clean>

```text
Remove `.calepin` directories and generated artifacts

Usage: calepin clean [OPTIONS]

Options:
  -d, --depth <DEPTH>  Maximum recursion depth when searching for `.calepin` directories
  -y, --yes            Skip interactive confirmation and delete immediately
  -h, --help           Print help
```
