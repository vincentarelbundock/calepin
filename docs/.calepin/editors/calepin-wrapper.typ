#import "/.calepin/calepin.typ": *



#let _raw-chunk-langs = ("python", "r", "mermaid", "dot", "tikz", "d2", "sh")
#show raw.where(block: true, lang: "typ", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "typst", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "python", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("python", it) }
#show raw.where(block: true, lang: "r", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("r", it) }
#show raw.where(block: true, lang: "mermaid", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("mermaid", it) }
#show raw.where(block: true, lang: "dot", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("dot", it) }
#show raw.where(block: true, lang: "tikz", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("tikz", it) }
#show raw.where(block: true, lang: "d2", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("d2", it) }
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

#import "/.calepin/calepin.typ" as calepin_runtime
#set document(title: [Editor integration])
#metadata((tags: ("getting started", "editors", "VS Code", "Tinymist"))) <website-metadata>

#title()

Calepin documents are ordinary Typst source files, so they work with any editor that supports Typst. The editor can provide language tooling and preview, while Calepin remains responsible for executing computational chunks.

= VS Code, Cursor, and Positron

Install _Calepin for Typst_ from the VS Code Marketplace. Cursor, Positron, and other VSX-compatible editors can install the same extension from Open VSX. It provides these command-palette actions:

- `Calepin Typst New`
- `Calepin Typst Compile`
- `Calepin Typst Watch`
- `Calepin Typst Stop`

The extension uses its bundled Calepin binary when available, then falls back to `calepin` on `PATH`. Set `calepin.binaryPath` to select another executable. The watch command runs Calepin continuously, with integrated preview for PDF and HTML; use it when executable chunks should rerun as the notebook changes.

= Tinymist preview

Tinymist can preview the authored `.typ` file after one successful Calepin compile, with no notebook-specific `typstExtraArgs`:

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin compile paper.typ\n", block: true, lang: "sh"))

Use the canonical facade and document adapter:

````typ
#import "/.calepin/calepin.typ" as calepin
#show: calepin.document
````

Tinymist refreshes prose, layout, and other Typst changes, but it does not execute computational chunks. After changing executable code, run Calepin again or use `Calepin Typst Watch`. Until then, the preview shows the last stored computational snapshot.

The generic facade follows the most recently compiled or watched single notebook when Calepin is not supplying its internal Typst inputs. For independent simultaneous previews, import the generated notebook-specific facade instead:

````typ
#import "/.calepin/paper/calepin.typ" as calepin
#show: calepin.document
````

Existing aliased imports and Calepin-managed workflows remain compatible. Do not use `#import "/.calepin/calepin.typ": *`: the exported `document` adapter shadows Typst's built-in `document` element, including `#set document(...)`.

= Other editors

The same compile-once contract works with other Typst language servers, preview tools, and plain `typst compile`, provided they use the same project root as Calepin. Run `calepin compile` to refresh stored results, or run `calepin watch` for automatic execution and rendering. No editor-specific Calepin settings or lock file are required.
