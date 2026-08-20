#import "chunk.typ": _fenced-chunk
#import "chunk-support.typ": _disable-raw-chunk-transforms
#import "code.typ": _html-themed-raw-block, _output-chrome, code-block

// Default paged styling for the labeled chunk carriers emitted by `code.typ`.
//
// Internal: the generated wrapper applies this, and only when a notebook theme
// is in play (`staging.rs`). It is not part of the public `calepin.*` surface —
// documents restyle chunks through the labels, and themes through their own
// show rules, so nobody outside Calepin needs to name this transform.
//
// A document compiled with `theme = "typst"` never has it applied, so the
// carriers stay bare and packages such as codly own the chrome around code
// completely. Syntax colors are not part of that: chunk source keeps Calepin's
// palette under every theme (see `code.typ`), so a chunk and a plain fenced
// block can differ in color even where all other chrome is gone.
// Chunks still execute either way — execution is driven by the fenced-chunk
// rules in the wrapper, not by this transform.
//
// Every rule below reconstructs from `it.body` and never re-displays `it`. That
// is a correctness condition, not a style choice: a rule that re-displays `it`
// would chain under a document author's later rule instead of being replaced by
// it, so overrides would nest rather than win.

// Code is sized relative to the captured document body size rather than to
// `1em`, which would compound: a literal ```typ block is rendered by replacing
// its source `raw` element, so it renders inside Typst's already-reduced raw
// text context, whereas executed chunks are emitted as ordinary calls at body
// size. Anchoring to the captured body size gives both paths a single, matching
// reduction instead of shrinking twice.
#let _calepin-body-size = std.state("calepin-body-size", 11pt)

#let _sized(body) = context {
  set text(size: _calepin-body-size.get() * 0.8)
  body
}

#let _default-chunk-chrome(body) = {
  show <calepin-input>: it => _sized(code-block(it.body))
  show <calepin-output>: it => _sized(_output-chrome(it.body, kind: "stdout"))
  show <calepin-result>: it => _sized(_output-chrome(it.body, kind: "result"))
  show <calepin-warning>: it => _sized(_output-chrome(it.body, kind: "warning"))
  show <calepin-error>: it => _sized(_output-chrome(it.body, kind: "error"))

  // Fenced blocks in a themed document. The wrapper installs the same detection
  // for registered languages earlier; this rule is defined last, so it is the
  // outermost one.
  //
  // The condition must not distinguish the query pass from the render pass.
  // It used to admit every language while querying and only the registered
  // ones while rendering, so the two passes stepped the automatic `chunk-N`
  // counter at different rates and every chunk after an unregistered fence
  // rendered its predecessor's source (issue #108). `_fenced-chunk` decides
  // from the document's own `fenced-chunks` setting, which is identical in
  // both passes.
  show raw.where(block: true, theme: auto): it => {
    if _disable-raw-chunk-transforms.get() {
      _html-themed-raw-block(it)
    } else if it.lang != none {
      _fenced-chunk(none, it.lang, it)
    } else {
      _html-themed-raw-block(it)
    }
  }

  context _calepin-body-size.update(text.size)
  body
}
