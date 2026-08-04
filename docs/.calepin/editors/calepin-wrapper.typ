#let _calepin-document-element = document
#import "/.calepin/calepin.typ": *
#let document = _calepin-document-element

#let _calepin-expected-generation = "e8168650674ca08c-1349cde127705c16"
#let _calepin-verify-generation() = {
  let path = sys.inputs.at("calepin-results", default: none)
  if path != none and path != "" {
    let actual = json(path).at("generation", default: "")
    if actual != _calepin-expected-generation {
      panic("Calepin results changed while this render was starting; Typst will retry with the completed build")
    }
  }
}
#_calepin-verify-generation()



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
#import "/.calepin/calepin.typ": _html-themed-raw-block, _is-query, chunk_from_raw_plain

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
  } else if it.lang != none and (_is-query() or _raw-chunk-langs.contains(it.lang)) and _fenced-chunks-runs(
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
#import "/.calepin/calepin.typ" as calepin
#show: calepin.document

#set document(title: [Editor integration])
#metadata((tags: ("getting started", "editors", "VS Code", "Tinymist"))) <website-metadata>

#title()

Calepin documents are ordinary Typst files, so you can keep your editor's language tooling and preview while Calepin evaluates computational chunks.

The #link("https://marketplace.visualstudio.com/items?itemName=myriad-dreamin.tinymist")[Tinymist extension] provides Typst language tooling and live document preview in VS Code.

The _Calepin for Typst_ extension adds start and stop commands for Calepin in VS Code, Cursor, Positron, and other VSX-compatible editors. Install it from the #link("https://marketplace.visualstudio.com/items?itemName=VincentArel-Bundock.calepin")[Visual Studio Marketplace] for VS Code, or from #link("https://open-vsx.org/extension/VincentArel-Bundock/calepin")[Open VSX] for Cursor, Positron, VSCodium, and other editors that use the Open VSX registry. This short screencast shows the workflow alongside Tinymist preview.

#calepin.elements.lightbox-video(
  "vscode-calepin-screencast",
  "/assets/calepin_vscode.mp4",
  poster: "/assets/calepin_vscode-thumb.png",
  width: 48em,
)

= Workflow

Tinymist and Calepin do different jobs, and both must be running. Tinymist ships its own Typst binary and renders the preview; it does not know what a code chunk is and never executes one. Calepin executes the chunks and stores their results on disk. The preview then picks up those stored results like any other file the document reads.

+ Open the notebook in VS Code, Cursor, or Positron.
+ Run *Typst: Calepin* from the command palette. This starts `calepin watch <file> --eval-only` in the background, which evaluates chunks but leaves rendering to your editor.
+ Run *Typst Preview: Preview Opened File* from Tinymist.
+ Edit and save. Calepin re-evaluates the chunks whose code changed and rewrites the results; Tinymist re-renders and the preview updates.

Run *Typst: Stop Calepin* to stop the background watcher.

= The document show rule

A notebook previewed this way must apply the document show rule:

```typ
#import "/.calepin/calepin.typ" as calepin
#show: calepin.document
```

This rule is what replaces executable code fences with their stored results. When Calepin drives Typst itself, it supplies the rule internally, so `calepin compile` produces a correct PDF or HTML file whether or not the line is present. Tinymist compiles the notebook directly, with no such wrapper, so without the line nothing rewrites the fences.

#calepin.elements.callout(kind: "note")[
  The symptom is specific: the preview shows your prose and your code, but no chunk output, while the file produced by `calepin compile` looks correct. Inline results such as `#py[...]` still appear, because those are ordinary function calls that read the stored results directly and do not depend on the show rule. If block chunks are blank and inline results are not, the show rule is missing.
]

`calepin new paper.typ` writes the line into the notebook it creates, so new documents have it already.

= Configuration

The extension runs `calepin watch <file> --eval-only` without a `--config` argument, and Calepin does not auto-discover configuration files. A `calepin.toml` that sets interpreter paths under `[executables]` therefore has no effect on chunks evaluated through the extension, and an engine that is not on your `PATH` will not run.

Until the extension grows a setting for this, either make the interpreters discoverable on `PATH`, or skip the extension command and start the watcher yourself in a terminal:

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin watch --config calepin.toml paper.typ --eval-only\n", block: true, lang: "sh"))

Tinymist preview works the same way against a watcher started in a terminal. The only setting the extension exposes today is `calepin.binaryPath`, which selects the `calepin` binary to run.
