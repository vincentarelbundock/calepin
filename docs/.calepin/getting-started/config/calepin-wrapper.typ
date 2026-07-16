#import "/.calepin/calepin.typ": *



#let _raw-chunk-langs = ("python", "r", "mermaid", "dot", "tikz", "d2", "bash", "toml")
#show raw.where(block: true, lang: "typ", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "typst", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "python", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("python", it) }
#show raw.where(block: true, lang: "r", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("r", it) }
#show raw.where(block: true, lang: "mermaid", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("mermaid", it) }
#show raw.where(block: true, lang: "dot", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("dot", it) }
#show raw.where(block: true, lang: "tikz", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("tikz", it) }
#show raw.where(block: true, lang: "d2", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("d2", it) }
#show raw.where(block: true, lang: "bash", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("bash", it) }
#show raw.where(block: true, lang: "toml", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("toml", it) }

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
#set document(title: [Configuration])

#title()

= `config.toml`

`Calepin` does not auto-discover configuration. For `compile` and `watch`, pass the config path explicitly with `--config=PATH`:

#calepin_runtime.chunk_from_raw_plain("bash", raw("calepin compile --config config.toml paper.typ\n", block: true, lang: "bash"))

If `--config` is omitted, `Calepin` uses defaults.

Relative paths written inside the config resolve relative to the config file's own directory, not the document being compiled. Absolute paths are used as is.

`asset-dir` controls where Calepin writes generated runtime files and browser-facing assets, including theme CSS, JavaScript, and fonts. The default is `.calepin`, which works in local preview. Some hosts like Netlify do not serve files from dot-directories, so deployed pages can sometimes load without styling, search, dark-mode toggles, or other theme JavaScript even though they looked correct locally. For sites published to those hosts, set `asset-dir = "_calepin"` before building.

```toml
# config.toml

# Directory for generated Calepin runtime and website assets.
# Default: ".calepin". Use "_calepin" for Netlify or GitHub Pages.
asset-dir = ".calepin"

[executables]
typst = "typst"                    # Typst binary (can be name or full path)
python = ".venv/bin/python"         # Python interpreter
rscript = "Rscript"                # R executable
mmdc = "mmdc"                      # Mermaid
#dot = "/opt/homebrew/bin/dot"     # optional, if needed
#d2 = "d2"                          # optional
#tectonic = "tectonic"              # optional
#dvisvgm = "dvisvgm"                # optional
#pdf2svg = "pdf2svg"                # optional
```

If `python` is not configured, set `CALEPIN_PYTHON` in the environment to
pick it. If that is absent, Calepin uses a local `.venv` interpreter when
available.
